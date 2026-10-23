// SPDX-License-Identifier: MIT
//! Single durable download job composed from libcurl transfer + store staging
//! and CAS. No provider resolver, subscriber single-flight, retry engine, install
//! transaction or GC is implemented here. A trusted Core authority assertion is
//! required; labels/hash keys do not authenticate a provider or grant ownership.
//!
//! All sync recovery/transfer/finalization/verification runs on bounded blocking
//! workers. A caller timeout/cancellation may precede actual worker exit: leases
//! stay with that worker. No Drop method syncs files or writes SQLite. Cancellation
//! around CAS publication may leave a verified CAS object; it never deletes it.

use crate::artifact::{CancelOnDrop, SameOriginGrant};
use crate::policy::Endpoint;
use crate::{
    AcceptedArtifactHeaders, ArtifactFile, ArtifactTransferRequest, Broker, CredentialId,
    CredentialPartition, NetError, StrongEtag, TransferCancellation, TransferError, TransferMode,
};
use cobble_store::{
    ArtifactExpectation, CasError, ContentStore, DownloadStage, DownloadStaging, ExpectedDigests,
    IngestResult, IntegrityBasis, PublicationDurability, StageError, StageIdentity, StageRecovery,
    StageRecoveryBudget, VerifiedArtifact,
};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    io::{self, Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const HARD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const CHECKPOINT_BYTES: u64 = 16 * 1024 * 1024;
const CHECKPOINT_TIME: Duration = Duration::from_secs(5);

/// Trusted Core assertions, not proof of provider permission. Redirects default
/// to same-origin; anonymous mirrors need the explicit request opt-in below.
/// Labels are bounded nonsecret record IDs;
/// never place credentials, signed URLs or user-facing personal data in them.
pub struct DownloadAuthority {
    provider: String,
    purpose: String,
    policy: String,
    partition: Option<CredentialPartition>,
}
impl DownloadAuthority {
    pub fn same_origin(
        provider: &str,
        purpose: &str,
        policy: &str,
        partition: Option<CredentialPartition>,
    ) -> Result<Self, DownloadError> {
        if [provider, purpose, policy].iter().any(|label| {
            label.is_empty() || label.len() > 128 || !label.bytes().all(|b| b.is_ascii_graphic())
        }) {
            return Err(DownloadError::InvalidRequest);
        }
        Ok(Self {
            provider: provider.into(),
            purpose: purpose.into(),
            policy: policy.into(),
            partition,
        })
    }
}
/// Fields are private so source/grant/identity cannot drift after validation.
/// External artifact jobs require known size and at least one trusted expected
/// digest. UserConfirmedLocal imports remain a separate T006 API.
#[derive(Clone)]
pub struct DownloadRequest {
    source_label: String,
    endpoint: Endpoint,
    url: String,
    credential: Option<CredentialId>,
    expectation: ArtifactExpectation,
    identity: StageIdentity,
    partition: Option<CredentialPartition>,
    mirror_redirects: bool,
    curseforge_redirects: bool,
}
impl DownloadRequest {
    pub(crate) fn progress_labels(&self) -> (String, String, u64) {
        let item = self
            .endpoint
            .url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .unwrap_or("download");
        (
            self.source_label.clone(),
            item.chars()
                .filter(|character| !character.is_control())
                .take(128)
                .collect(),
            self.identity.expected_size,
        )
    }
    /// Opt in for a selected anonymous BMCL-compatible source's HTTPS CDN routes.
    /// The provider may delegate through redirects subject to the broker's
    /// per-hop destination/TLS checks and existing redirect/body/time limits.
    /// Redirects cannot change the expected digest supplied by the caller.
    pub fn with_mirror_redirects(
        mut self,
        original_official_url: &str,
    ) -> Result<Self, DownloadError> {
        if self.credential.is_some()
            || self.partition.is_some()
            || self.mirror_redirects
            || self.curseforge_redirects
        {
            return Err(DownloadError::InvalidRequest);
        }
        self.expectation
            .digests
            .sha1
            .ok_or(DownloadError::InvalidRequest)?;
        let official = Endpoint::parse(original_official_url).map_err(DownloadError::Policy)?;
        let path = official.url.path();
        let supported = match official.url.host_str() {
            Some("resources.download.minecraft.net" | "libraries.minecraft.net") => true,
            Some("piston-meta.mojang.com" | "launchermeta.mojang.com") => {
                path.starts_with("/v1/packages/") || path.starts_with("/mc/game/")
            }
            Some("piston-data.mojang.com" | "launcher.mojang.com") => {
                path.starts_with("/v1/objects/") || path.starts_with("/mc/game/")
            }
            _ => false,
        };
        if !supported || official.url.query().is_some() {
            return Err(DownloadError::InvalidRequest);
        }
        let official_url = official.url.to_string();
        let mut hash = Sha256::new();
        hash.update(b"CobbleLauncher anonymous provider-delegated mirror redirects v2\0");
        hash.update(self.identity.request_key);
        hash_field(&mut hash, official_url.as_bytes());
        self.identity.request_key = hash.finalize().into();
        self.mirror_redirects = true;
        Ok(self)
    }
    /// Follow anonymous HTTPS redirects for an explicitly selected modpack's
    /// documented download hosts. Existing per-hop network checks still apply.
    pub fn with_modpack_redirects(mut self) -> Result<Self, DownloadError> {
        if self.credential.is_some()
            || self.partition.is_some()
            || self.mirror_redirects
            || self.curseforge_redirects
            || !matches!(
                self.endpoint.url.host_str(),
                Some(
                    "cdn.modrinth.com" | "github.com" | "raw.githubusercontent.com" | "gitlab.com"
                )
            )
        {
            return Err(DownloadError::InvalidRequest);
        }
        let mut hash = Sha256::new();
        hash.update(b"CobbleLauncher selected modpack redirects v1\0");
        hash.update(self.identity.request_key);
        self.identity.request_key = hash.finalize().into();
        self.mirror_redirects = true;
        Ok(self)
    }

    pub(crate) fn key(&self) -> [u8; 32] {
        self.identity.request_key
    }
    pub fn new(
        url: &str,
        expectation: ArtifactExpectation,
        authority: DownloadAuthority,
        credential: Option<CredentialId>,
    ) -> Result<Self, DownloadError> {
        let endpoint = Endpoint::parse(url).map_err(DownloadError::Policy)?;
        let size = expectation
            .expected_size
            .ok_or(DownloadError::InvalidRequest)?;
        if expectation.basis != IntegrityBasis::TrustedMetadata
            || expectation.digests == ExpectedDigests::default()
            || expectation.max_bytes > HARD_BYTES
            || size > expectation.max_bytes
            || credential.is_some() != authority.partition.is_some()
        {
            return Err(DownloadError::InvalidRequest);
        }
        let mut hash = Sha256::new();
        hash.update(b"CobbleLauncher durable artifact job v1\0");
        for field in [
            size.to_be_bytes().as_slice(),
            expectation.max_bytes.to_be_bytes().as_slice(),
            &[0],
            authority.provider.as_bytes(),
            authority.purpose.as_bytes(),
            authority.policy.as_bytes(),
        ] {
            hash_field(&mut hash, field);
        }
        hash_option(
            &mut hash,
            expectation.digests.sha1.as_ref().map(|v| v.as_slice()),
        );
        hash_option(
            &mut hash,
            expectation.digests.sha256.as_ref().map(|v| v.as_slice()),
        );
        hash_option(
            &mut hash,
            expectation.digests.sha512.as_ref().map(|v| v.as_slice()),
        );
        hash_option(
            &mut hash,
            authority.partition.as_ref().map(|v| v.0.as_slice()),
        );
        hash_field(&mut hash, &Sha256::digest(url.as_bytes()));
        let identity = StageIdentity {
            request_key: hash.finalize().into(),
            expected_size: size,
            max_bytes: expectation.max_bytes,
        };
        Ok(Self {
            source_label: authority.provider,
            endpoint,
            url: url.into(),
            credential,
            expectation,
            identity,
            partition: authority.partition,
            mirror_redirects: false,
            curseforge_redirects: false,
        })
    }
    /// Exact API-provided CurseForge files may redirect between the finite
    /// official CDN origins. Credentials are still stripped on origin change.
    pub fn with_curseforge_redirects(mut self) -> Result<Self, DownloadError> {
        if self.mirror_redirects
            || self.curseforge_redirects
            || self.source_label != "curseforge"
            || self.expectation.digests.sha1.is_none()
            || !crate::artifact::curseforge_endpoint(&self.endpoint)
        {
            return Err(DownloadError::InvalidRequest);
        }
        let mut hash = Sha256::new();
        hash.update(b"CobbleLauncher finite official CurseForge CDN redirects v1\0");
        hash.update(self.identity.request_key);
        self.identity.request_key = hash.finalize().into();
        self.curseforge_redirects = true;
        Ok(self)
    }
}
fn hash_field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u32).to_be_bytes());
    hash.update(bytes);
}
fn hash_option(hash: &mut Sha256, bytes: Option<&[u8]>) {
    hash.update([u8::from(bytes.is_some())]);
    if let Some(bytes) = bytes {
        hash_field(hash, bytes);
    }
}

#[derive(Debug)]
pub enum DownloadError {
    InvalidRequest,
    Policy(NetError),
    Transfer(TransferError),
    Staging(StageError),
    Integrity(CasError),
    Cancelled,
    Deadline,
    Worker,
}
impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRequest => "invalid trusted download request",
            Self::Policy(_) => "download policy denied",
            Self::Transfer(_) => "artifact transfer failed",
            Self::Staging(_) => "download staging failed",
            Self::Integrity(_) => "artifact verification failed",
            Self::Cancelled => "download cancelled; worker may still be finalizing",
            Self::Deadline => "download job deadline exceeded; worker may still be finalizing",
            Self::Worker => "download worker failed",
        })
    }
}
impl std::error::Error for DownloadError {}
pub struct DownloadResult {
    pub artifact: VerifiedArtifact,
    pub reused: bool,
    pub publication_durability: PublicationDurability,
    /// CAS is verified even when private staging cleanup failed. A cleanup
    /// failure must not be presented as absent/unverified CAS publication.
    pub staging_retained: bool,
}
#[derive(Clone)]
pub struct ArtifactDownloader {
    broker: Broker,
    staging: Arc<DownloadStaging>,
    content: Arc<ContentStore>,
    jobs: Arc<Semaphore>,
    disk: Arc<Semaphore>,
}
impl ArtifactDownloader {
    pub(crate) fn concurrency(&self) -> usize {
        self.broker.artifact_config().max_concurrent
    }
    pub fn new(broker: Broker, staging: DownloadStaging, content: ContentStore) -> Self {
        let concurrent = broker.artifact_config().max_concurrent;
        Self {
            broker,
            staging: Arc::new(staging),
            content: Arc::new(content),
            jobs: Arc::new(Semaphore::new(concurrent)),
            disk: Arc::new(Semaphore::new(2)),
        }
    }
    pub(crate) fn total_timeout(&self) -> Duration {
        self.broker.artifact_config().total_timeout
    }
    pub(crate) fn validate_request(&self, request: &DownloadRequest) -> Result<(), DownloadError> {
        self.broker
            .check_download_credentials(
                &request.endpoint,
                request.credential.as_ref(),
                request.partition,
            )
            .map_err(DownloadError::Policy)
    }
    pub async fn download(
        &self,
        request: DownloadRequest,
        cancel: TransferCancellation,
    ) -> Result<DownloadResult, DownloadError> {
        self.download_until(request, cancel, Instant::now() + self.total_timeout(), None)
            .await
    }
    // Wait for the last preparation/transfer/CAS owner, including workers which
    // outlive a timed-out async attempt, before allowing a retry/new writer.
    pub(crate) async fn download_shared_before(
        &self,
        request: DownloadRequest,
        cancel: TransferCancellation,
        deadline: Instant,
    ) -> Result<DownloadResult, DownloadError> {
        let (send, receive) = tokio::sync::oneshot::channel();
        let completion = Arc::new(AttemptCompletion(Some(send)));
        let result = self
            .download_until(request, cancel, deadline, Some(completion))
            .await;
        let _ = receive.await;
        result
    }
    async fn download_until(
        &self,
        request: DownloadRequest,
        cancel: TransferCancellation,
        deadline: Instant,
        completion: Option<Arc<AttemptCompletion>>,
    ) -> Result<DownloadResult, DownloadError> {
        let total = deadline.saturating_duration_since(Instant::now());
        let mut guard = CancelOnDrop {
            cancellation: cancel.clone(),
            armed: true,
        };
        let operation = self.download_inner(request, cancel.clone(), deadline, completion);
        let result = tokio::select! { biased;
            _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
            result = tokio::time::timeout(total, operation) => result,
        };
        match result {
            Ok(result) => {
                guard.armed = false;
                result
            }
            Err(_) => Err(DownloadError::Deadline),
        }
    }
    async fn download_inner(
        &self,
        request: DownloadRequest,
        cancel: TransferCancellation,
        deadline: Instant,
        completion: Option<Arc<AttemptCompletion>>,
    ) -> Result<DownloadResult, DownloadError> {
        self.broker
            .check_download_credentials(
                &request.endpoint,
                request.credential.as_ref(),
                request.partition,
            )
            .map_err(DownloadError::Policy)?;
        let job = Arc::new(
            self.jobs
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| DownloadError::Worker)?,
        );
        let mut restart = false;
        let file = loop {
            let disk = Arc::new(
                self.disk
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(|_| DownloadError::Worker)?,
            );
            let staging = self.staging.clone();
            let identity = request.identity;
            let worker_job = job.clone();
            let status = Arc::new(Mutex::new(None));
            let worker_status = status.clone();
            let worker_completion = completion.clone();
            let recovery_budget = StageRecoveryBudget::new(deadline, cancel.stop_flag());
            let (file, mode) = tokio::task::spawn_blocking(move || {
                // Recovery/hashing has a separate two-worker disk budget. Do
                // not keep that lease while the socket waits for upstream.
                let _disk = disk;
                let completion = worker_completion;
                let mut stage = staging
                    .open_job_with_budget(identity, recovery_budget)
                    .map_err(DownloadError::Staging)?;
                let mode = match stage.recovery() {
                    StageRecovery::Resume(checkpoint) if !restart => checkpoint
                        .validator
                        .as_deref()
                        .and_then(|value| std::str::from_utf8(value).ok())
                        .and_then(|value| StrongEtag::new(value).ok())
                        .map(|etag| TransferMode::Resume {
                            offset: checkpoint.durable_offset,
                            etag,
                        }),
                    _ => None,
                };
                let mode = if let Some(mode) = mode {
                    mode
                } else {
                    // Store recovery has already durably prepared an empty
                    // Full stage. Only a resumable prefix being abandoned
                    // needs another reset before requesting a fresh response.
                    if matches!(stage.recovery(), StageRecovery::Resume(_)) {
                        stage.reset_for_full(None).map_err(DownloadError::Staging)?;
                    }
                    TransferMode::Full
                };
                Ok::<_, DownloadError>((
                    StageFile {
                        stage: Some(stage),
                        validator: None,
                        accepted: false,
                        since_checkpoint: 0,
                        checkpoint_at: Instant::now(),
                        status: worker_status,
                        _job: worker_job,
                        _completion: completion,
                    },
                    mode,
                ))
            })
            .await
            .map_err(|_| DownloadError::Worker)??;
            check_live(&cancel, deadline)?;
            let transfer = ArtifactTransferRequest {
                url: request.url.clone(),
                credential: request.credential.clone(),
                expected_size: identity.expected_size,
                max_bytes: identity.max_bytes,
                mode,
            };
            let result = self
                .broker
                .artifact_transfer_granted(
                    transfer,
                    file,
                    cancel.clone(),
                    SameOriginGrant::new(&request.endpoint)
                        .with_mirror(request.mirror_redirects)
                        .with_curseforge(request.curseforge_redirects),
                )
                .await;
            if let Some(error) = status.lock().map_err(|_| DownloadError::Worker)?.take() {
                return Err(DownloadError::Staging(error));
            }
            match result {
                Ok(receipt) => break receipt.file,
                Err(TransferError::RestartRequired) if !restart => {
                    restart = true;
                }
                Err(error) => return Err(DownloadError::Transfer(error)),
            }
        };
        check_live(&cancel, deadline)?;
        let disk = self
            .disk
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| DownloadError::Worker)?;
        let content = self.content.clone();
        tokio::task::spawn_blocking(move || {
            let _disk = disk;
            let _completion = file._completion;
            let _job = job;
            let mut stage = file.stage.expect("owned stage");
            check_live(&cancel, deadline)?;
            let computed = match stage.recovery() {
                StageRecovery::Resume(checkpoint) => checkpoint.prefix_sha256,
                StageRecovery::Full if request.identity.expected_size == 0 => {
                    Sha256::digest([]).into()
                }
                _ => return Err(DownloadError::InvalidRequest),
            };
            stage
                .seek(SeekFrom::Start(0))
                .map_err(|_| DownloadError::Worker)?;
            #[cfg(test)]
            fault_point("before_cas");
            let result = content.ingest(
                CancellableRead {
                    stage: &mut stage,
                    cancel: &cancel,
                    deadline,
                },
                request.expectation,
            );
            let ingested = confirm_publication(
                &content,
                result,
                computed,
                request.expectation,
                &StageRecoveryBudget::new(deadline, cancel.stop_flag()),
            )?;
            #[cfg(test)]
            fault_point("after_cas");
            // Publication is already verified. A late cancellation may leave
            // it in CAS, and must not trigger deletion or an unchecked retry.
            check_live(&cancel, deadline)?;
            let staging_retained = stage.discard().is_err();
            Ok(DownloadResult {
                artifact: ingested.artifact,
                reused: ingested.reused,
                publication_durability: ingested.durability,
                staging_retained,
            })
        })
        .await
        .map_err(|_| DownloadError::Worker)?
    }
}
fn check_live(cancel: &TransferCancellation, deadline: Instant) -> Result<(), DownloadError> {
    if cancel.is_cancelled() {
        Err(DownloadError::Cancelled)
    } else if Instant::now() >= deadline {
        Err(DownloadError::Deadline)
    } else {
        Ok(())
    }
}
fn confirm_publication(
    content: &ContentStore,
    result: Result<IngestResult, CasError>,
    computed: [u8; 32],
    expected: ArtifactExpectation,
    budget: &StageRecoveryBudget,
) -> Result<IngestResult, DownloadError> {
    match result {
        Ok(result) => Ok(result),
        Err(error @ CasError::PublicationMayExist(_)) => {
            let artifact = content
                .open_verified_with_budget(computed, expected, budget)
                .map_err(|_| DownloadError::Integrity(error))?;
            Ok(IngestResult {
                artifact,
                reused: true,
                durability: PublicationDurability::FileSyncedDirectoryDurabilityUnverified,
            })
        }
        Err(error) => Err(DownloadError::Integrity(error)),
    }
}
struct CancellableRead<'a> {
    stage: &'a mut DownloadStage,
    cancel: &'a TransferCancellation,
    deadline: Instant,
}
impl Read for CancellableRead<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        check_live(self.cancel, self.deadline)
            .map_err(|_| io::Error::other("job no longer active"))?;
        self.stage.read(out)
    }
}

struct AttemptCompletion(Option<tokio::sync::oneshot::Sender<()>>);
impl Drop for AttemptCompletion {
    fn drop(&mut self) {
        if let Some(send) = self.0.take() {
            let _ = send.send(());
        }
    }
}

struct StageFile {
    stage: Option<DownloadStage>,
    validator: Option<StrongEtag>,
    accepted: bool,
    since_checkpoint: u64,
    checkpoint_at: Instant,
    status: Arc<Mutex<Option<StageError>>>,
    _job: Arc<OwnedSemaphorePermit>,
    // Completion stays with preparation, HTTP/checkpoints and CAS even if the
    // caller is cancelled; retries wait for the final blocking owner to exit.
    _completion: Option<Arc<AttemptCompletion>>,
}
impl StageFile {
    fn checkpoint(&mut self) -> io::Result<()> {
        let result = self
            .stage
            .as_mut()
            .expect("owned stage")
            .checkpoint(self.validator.as_ref().map(StrongEtag::as_bytes));
        self.record(result)?;
        self.since_checkpoint = 0;
        self.checkpoint_at = Instant::now();
        Ok(())
    }
    fn record<T>(&self, result: Result<T, StageError>) -> io::Result<T> {
        result.map_err(|error| {
            if let Ok(mut status) = self.status.lock()
                && status.is_none()
            {
                *status = Some(error);
            }
            io::Error::other("staging checkpoint failed")
        })
    }
}
impl ArtifactFile for StageFile {
    fn file_len(&self) -> io::Result<u64> {
        self.record(self.stage.as_ref().expect("owned stage").byte_len())
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        // The typed accepted-header hook already durably reset this inode.
        if len != 0 || self.file_len()? != 0 {
            return Err(io::Error::other("unexpected stage truncation"));
        }
        Ok(())
    }
    fn accepted_headers(&mut self, headers: AcceptedArtifactHeaders) -> io::Result<()> {
        match headers {
            AcceptedArtifactHeaders::Full { etag } => {
                let reset = self
                    .stage
                    .as_mut()
                    .expect("owned stage")
                    .reset_for_full(etag.as_ref().map(StrongEtag::as_bytes));
                self.record(reset)?;
                self.validator = etag;
                #[cfg(test)]
                fault_point("after_full_reset");
            }
            AcceptedArtifactHeaders::Resume { etag } => {
                self.validator = Some(etag);
            }
        }
        self.accepted = true;
        self.checkpoint_at = Instant::now();
        Ok(())
    }
    fn finish_transfer(&mut self) -> io::Result<()> {
        if self.accepted {
            self.checkpoint()?;
        }
        Ok(())
    }
}
impl Write for StageFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.stage.as_mut().expect("owned stage").write(bytes)?;
        self.since_checkpoint += count as u64;
        if self.since_checkpoint >= CHECKPOINT_BYTES
            || self.checkpoint_at.elapsed() >= CHECKPOINT_TIME
        {
            self.checkpoint()?;
        }
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stage.as_mut().expect("owned stage").flush()
    }
}
impl Read for StageFile {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.stage.as_mut().expect("owned stage").read(out)
    }
}
impl Seek for StageFile {
    fn seek(&mut self, at: SeekFrom) -> io::Result<u64> {
        self.stage.as_mut().expect("owned stage").seek(at)
    }
}

#[cfg(test)]
fn fault_point(phase: &str) {
    if std::env::var("COBBLE_DOWNLOAD_CHILD_PHASE").ok().as_deref() == Some(phase) {
        std::fs::write(
            std::env::var_os("COBBLE_DOWNLOAD_CHILD_SIGNAL").expect("test child signal"),
            b"ready",
        )
        .unwrap();
        loop {
            std::thread::park_timeout(Duration::from_secs(1));
        }
    }
}

#[cfg(test)]
#[path = "download_tests.rs"]
mod tests;
