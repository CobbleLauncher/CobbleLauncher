// SPDX-License-Identifier: MIT
//! Thin libcurl artifact transport, not an installer or a verified CAS handle.
//! Core owns the pre-opened file; renderer/plugins receive no paths or options.
//! Every hop uses a fresh, non-reusable libcurl handle, pinned DNS addresses,
//! verified TLS, no proxy and no automatic redirects. The safe binding cannot
//! inspect the socket peer from a header callback without reentrant FFI: pinning
//! fixes the destination structurally; primary IP/port is checked after perform.
//! No claim is made that the peer was read from the socket before body writes.
//!
//! Filesystem writes run on bounded blocking workers. Dropping the future marks
//! cancellation; it never releases worker leases or pretends to stop a live
//! blocking task. libcurl cancellation is cooperative (idle callbacks can be
//! about one second apart), with a hard libcurl transfer deadline as backstop.
//! Arbitrary blocking filesystem implementations cannot be forcibly interrupted.
//! Failed/cancelled transfers may leave a partial; no outcome commits to CAS.

use crate::broker::{ConnectionLeases, ResponseMetadata};
use crate::policy::{Endpoint, Origin};
use crate::{Broker, CredentialId, NetError, TimeoutPhase};
use curl::easy::{Easy2, Handler, HttpVersion, List, NetRc, SslVersion, WriteError};
use http::{HeaderMap, HeaderName, HeaderValue};
use std::{
    fmt,
    io::{Read, Seek, SeekFrom, Write},
    net::{IpAddr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::OwnedSemaphorePermit;

const HARD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const HEADER_BYTES: usize = 16 * 1024;
const HEADER_COUNT: usize = 64;

/// Core-only pre-opened regular file. Implementations must provide real file
/// length/truncation, not accept an untrusted path or run arbitrary host code.
pub trait ArtifactFile: Read + Write + Seek + Send + 'static {
    fn file_len(&self) -> std::io::Result<u64>;
    fn truncate(&mut self, len: u64) -> std::io::Result<()>;
    /// Runs only inside the bounded blocking transfer worker, after the entire
    /// final header gate passes and before truncation, seeking or any body write.
    fn accepted_headers(&mut self, _headers: AcceptedArtifactHeaders) -> std::io::Result<()> {
        Ok(())
    }
    /// Explicit blocking-worker finalization on both transport success/failure.
    /// Drop must remain free of SQLite/fsync I/O: queued/cancelled sinks may be
    /// destroyed on an async executor without ever entering the worker.
    fn finish_transfer(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub enum AcceptedArtifactHeaders {
    Full { etag: Option<StrongEtag> },
    Resume { etag: StrongEtag },
}
impl ArtifactFile for std::fs::File {
    fn file_len(&self) -> std::io::Result<u64> {
        let metadata = self.metadata()?;
        if !metadata.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "regular file required",
            ));
        }
        Ok(metadata.len())
    }
    fn truncate(&mut self, len: u64) -> std::io::Result<()> {
        self.set_len(len)
    }
}

/// Strong opaque HTTP entity tag; deliberately no Debug/Display.
#[derive(Clone, PartialEq, Eq)]
pub struct StrongEtag(String);
impl StrongEtag {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
    pub fn new(value: &str) -> Result<Self, TransferError> {
        if value.len() < 2
            || value.len() > 1024
            || !value.starts_with('"')
            || !value.ends_with('"')
            || value.as_bytes()[1..value.len() - 1]
                .iter()
                .any(|b| !matches!(b, 0x21 | 0x23..=0x7e))
        {
            return Err(TransferError::InvalidRequest);
        }
        Ok(Self(value.to_owned()))
    }
}
#[derive(Clone)]
pub enum TransferMode {
    Full,
    Resume { offset: u64, etag: StrongEtag },
}
/// This slice requires a known artifact size. Digests/provenance remain the
/// installer's T006 verification responsibility, including the existing prefix.
pub struct ArtifactTransferRequest {
    pub url: String,
    pub credential: Option<CredentialId>,
    pub expected_size: u64,
    pub max_bytes: u64,
    pub mode: TransferMode,
}
#[derive(Default)]
struct CancellationState {
    cancelled: Arc<AtomicBool>,
    changed: tokio::sync::Notify,
    received: AtomicU64,
    active: AtomicUsize,
}
#[derive(Clone, Default)]
pub struct TransferCancellation(Arc<CancellationState>);
impl TransferCancellation {
    pub(crate) fn observed_bytes(&self) -> u64 {
        self.0.received.load(Ordering::Relaxed)
    }
    pub(crate) fn active_transfers(&self) -> usize {
        self.0.active.load(Ordering::Relaxed)
    }
    pub(crate) fn stop_flag(&self) -> Arc<AtomicBool> {
        self.0.cancelled.clone()
    }
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        self.0.changed.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    pub(crate) async fn cancelled(&self) {
        let notified = self.0.changed.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if !self.is_cancelled() {
            notified.await;
        }
    }
}
pub(crate) struct CancelOnDrop {
    pub(crate) cancellation: TransferCancellation,
    pub(crate) armed: bool,
}
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.cancellation.cancel();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferError {
    Network(NetError),
    InvalidRequest,
    Sink,
    Cancelled,
    RestartRequired,
    InvalidRange,
    SizeMismatch,
    HttpStatus(u16),
    HttpRetry {
        status: u16,
        retry_after_seconds: Option<u64>,
    },
    UnsupportedRetryAfter,
    Worker,
    SourceNotAuthorized,
}
impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "artifact transfer: {self:?}")
    }
}
impl std::error::Error for TransferError {}
impl From<NetError> for TransferError {
    fn from(value: NetError) -> Self {
        Self::Network(value)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferOutcome {
    Transferred,
    /// Transport did not write anything or verify the whole file. The caller
    /// must re-read size + every trusted digest before considering completion.
    NeedsWholeFileVerification,
}
pub struct TransferReceipt<S> {
    pub file: S,
    pub outcome: TransferOutcome,
    pub metadata: ResponseMetadata,
}
/// Internal typed grant, constructed from the same validated request endpoint.
/// Public-IP safety and provider/source authority are separate checks.
pub(crate) struct SameOriginGrant {
    origin: Origin,
    mirror_redirects: bool,
    official_origins: Vec<Origin>,
    official_files_path: bool,
}
pub(crate) fn curseforge_endpoint(endpoint: &Endpoint) -> bool {
    endpoint.credential_path_safe
        && endpoint.url.scheme() == "https"
        && matches!(
            endpoint.url.host_str(),
            Some("edge.forgecdn.net" | "mediafilez.forgecdn.net" | "media.forgecdn.net")
        )
        && endpoint.url.path().starts_with("/files/")
        && endpoint.url.query().is_none()
        && endpoint.url.fragment().is_none()
        && endpoint.url.port().is_none()
}
impl SameOriginGrant {
    pub(crate) fn new(endpoint: &Endpoint) -> Self {
        Self {
            origin: endpoint.origin(),
            mirror_redirects: false,
            official_origins: Vec::new(),
            official_files_path: false,
        }
    }
    pub(crate) fn with_mirror(mut self, mirror_redirects: bool) -> Self {
        self.mirror_redirects = mirror_redirects;
        self
    }
    pub(crate) fn with_curseforge(mut self, enabled: bool) -> Self {
        if enabled {
            self.official_origins = [
                "https://edge.forgecdn.net",
                "https://mediafilez.forgecdn.net",
                "https://media.forgecdn.net",
            ]
            .into_iter()
            .map(|origin| Origin(origin.into()))
            .collect();
            self.official_files_path = true;
        }
        self
    }
    fn permits(&self, endpoint: &Endpoint) -> bool {
        if self.official_files_path {
            return endpoint.credential_path_safe
                && self.official_origins.contains(&endpoint.origin())
                && endpoint.url.path().starts_with("/files/")
                && endpoint.url.query().is_none()
                && endpoint.url.fragment().is_none();
        }
        // The selected anonymous mirror delegates CDN routing. Every hop still
        // passes the broker's HTTPS/public-IP/TLS limits and final digest checks.
        self.mirror_redirects || self.origin == endpoint.origin()
    }
}

#[cfg(test)]
#[test]
fn mirror_opt_in_does_not_change_default_same_origin_grant() {
    let start = Endpoint::parse("https://mirror.fixture.invalid/maven/file.jar").unwrap();
    let other = Endpoint::parse("https://storage.fixture.invalid/bmclapi/ab/fixture").unwrap();
    let grant = SameOriginGrant::new(&start);
    assert!(grant.permits(&start));
    assert!(!grant.permits(&other));
    assert!(grant.with_mirror(true).permits(&other));
}

impl Broker {
    /// Streaming artifact transfer through the same broker policy/credentials.
    /// Success means transport completion only, never upstream/hash verification.
    pub async fn artifact_transfer<S: ArtifactFile>(
        &self,
        request: ArtifactTransferRequest,
        file: S,
        cancel: TransferCancellation,
    ) -> Result<TransferReceipt<S>, TransferError> {
        self.artifact_transfer_inner(request, file, cancel, None)
            .await
    }
    pub(crate) async fn artifact_transfer_granted<S: ArtifactFile>(
        &self,
        request: ArtifactTransferRequest,
        file: S,
        cancel: TransferCancellation,
        grant: SameOriginGrant,
    ) -> Result<TransferReceipt<S>, TransferError> {
        self.artifact_transfer_inner(request, file, cancel, Some(grant))
            .await
    }
    async fn artifact_transfer_inner<S: ArtifactFile>(
        &self,
        request: ArtifactTransferRequest,
        file: S,
        cancel: TransferCancellation,
        grant: Option<SameOriginGrant>,
    ) -> Result<TransferReceipt<S>, TransferError> {
        let mut cancel_on_drop = CancelOnDrop {
            cancellation: cancel.clone(),
            armed: true,
        };
        let config = self.artifact_config();
        let deadline = Instant::now() + config.total_timeout;
        let operation = async {
            if request.max_bytes > HARD_BYTES
                || request.expected_size > request.max_bytes
                || matches!(request.mode, TransferMode::Resume { offset, .. } if offset == 0 || offset > request.expected_size)
            {
                return Err(TransferError::InvalidRequest);
            }
            let mut endpoint = Endpoint::parse(&request.url)?;
            let request_lease = Arc::new(self.artifact_request_lease().await?);
            let mut file = file;
            let mut stripped = false;
            for hop in 0..=config.max_redirects {
                if cancel.is_cancelled() {
                    return Err(TransferError::Cancelled);
                }
                if grant
                    .as_ref()
                    .is_some_and(|grant| !grant.permits(&endpoint))
                {
                    return Err(TransferError::SourceNotAuthorized);
                }
                let (addresses, headers, leases) = self
                    .artifact_hop(&endpoint, request.credential.as_ref().filter(|_| !stripped))
                    .await?;
                let worker = Worker {
                    file,
                    mode: request.mode.clone(),
                    expected: request.expected_size,
                    max_bytes: request.max_bytes,
                    received: 0,
                    status: 0,
                    headers: HeaderMap::new(),
                    header_bytes: 0,
                    header_count: 0,
                    gate: Gate::Pending,
                    error: None,
                    cancel: cancel.clone(),
                    deadline,
                    last_progress: Instant::now(),
                    no_progress: config.no_progress_timeout,
                    _request: request_lease.clone(),
                    _connections: leases,
                };
                let endpoint_for_worker = endpoint.clone();
                let remaining = deadline.saturating_duration_since(Instant::now());
                // cfg(test) routing is private and never enabled by features.
                #[cfg(test)]
                let fixture = self.artifact_fixture();
                let result = tokio::task::spawn_blocking(move || {
                    run_hop(
                        worker,
                        endpoint_for_worker,
                        addresses,
                        headers,
                        config.connect_timeout.min(remaining),
                        remaining,
                        #[cfg(test)]
                        fixture,
                    )
                })
                .await
                .map_err(|_| TransferError::Worker)??;
                file = result.file;
                match result.gate {
                    Gate::Redirect(location) => {
                        if hop == config.max_redirects {
                            return Err(NetError::RedirectLimit.into());
                        }
                        let next = endpoint.redirect(&location)?;
                        stripped |= next.origin() != endpoint.origin();
                        endpoint = next;
                    }
                    Gate::Accept | Gate::Verify => {
                        return Ok(TransferReceipt {
                            file,
                            outcome: if matches!(result.gate, Gate::Verify) {
                                TransferOutcome::NeedsWholeFileVerification
                            } else {
                                TransferOutcome::Transferred
                            },
                            metadata: ResponseMetadata {
                                status: result.status,
                                body_bytes: result.received,
                                redirects: hop,
                                final_origin: endpoint.origin().0,
                                content_length: number(&result.headers, "content-length")?,
                                etag: text(&result.headers, "etag")?,
                                last_modified: text(&result.headers, "last-modified")?,
                                content_range: text(&result.headers, "content-range")?,
                            },
                        });
                    }
                    Gate::Pending => return Err(NetError::Protocol.into()),
                }
            }
            Err(NetError::RedirectLimit.into())
        };
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TransferError::Cancelled),
            result = tokio::time::timeout(config.total_timeout, operation) => result,
        };
        match result {
            Ok(result) => {
                cancel_on_drop.armed = false;
                result
            }
            Err(_) => Err(TransferError::Network(NetError::Timeout(
                TimeoutPhase::Total,
            ))),
        }
    }
}

enum Gate {
    Pending,
    Accept,
    Redirect(String),
    Verify,
}
struct CurlHandler<S>(Option<Worker<S>>);
impl<S> CurlHandler<S> {
    fn state(&self) -> &Worker<S> {
        self.0.as_ref().expect("owned worker")
    }
    fn state_mut(&mut self) -> &mut Worker<S> {
        self.0.as_mut().expect("owned worker")
    }
}
impl<S: ArtifactFile> Handler for CurlHandler<S> {
    fn header(&mut self, data: &[u8]) -> bool {
        self.0.as_mut().is_some_and(|s| s.header(data))
    }
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.0.as_mut().map_or(Ok(0), |s| s.write(data))
    }
    fn progress(&mut self, a: f64, b: f64, c: f64, d: f64) -> bool {
        self.0.as_mut().is_some_and(|s| s.progress(a, b, c, d))
    }
}
struct Worker<S> {
    file: S,
    mode: TransferMode,
    expected: u64,
    max_bytes: u64,
    received: u64,
    status: u16,
    headers: HeaderMap,
    header_bytes: usize,
    header_count: usize,
    gate: Gate,
    error: Option<TransferError>,
    cancel: TransferCancellation,
    deadline: Instant,
    last_progress: Instant,
    no_progress: Duration,
    _request: Arc<OwnedSemaphorePermit>,
    _connections: ConnectionLeases,
}
impl<S: ArtifactFile> Worker<S> {
    fn check_job_budget(&self) -> Result<(), TransferError> {
        if self.cancel.is_cancelled() {
            return Err(TransferError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(NetError::Timeout(TimeoutPhase::Total).into());
        }
        Ok(())
    }
    fn check_live(&self) -> Result<(), TransferError> {
        self.check_job_budget()?;
        if self.last_progress.elapsed() >= self.no_progress {
            return Err(NetError::Timeout(TimeoutPhase::Body).into());
        }
        Ok(())
    }
    fn fail(&mut self, error: TransferError) -> bool {
        self.error = Some(error);
        false
    }
    fn finish_headers(&mut self) -> Result<(), TransferError> {
        // Validate all singular metadata before changing the destination file.
        let length = number(&self.headers, "content-length")?;
        let etag = text(&self.headers, "etag")?;
        let _ = text(&self.headers, "last-modified")?;
        let range = text(&self.headers, "content-range")?;
        let content_type = text(&self.headers, "content-type")?;
        if self.status == 206
            && content_type.as_deref().is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("multipart/byteranges"))
            })
        {
            return Err(TransferError::InvalidRange);
        }
        let encoding = text(&self.headers, "content-encoding")?;
        if encoding
            .as_deref()
            .is_some_and(|s| !s.eq_ignore_ascii_case("identity"))
        {
            return Err(NetError::UnsupportedEncoding.into());
        }
        if matches!(self.status, 301 | 302 | 303 | 307 | 308) {
            // A redirected partial has no authority to inherit its validator,
            // source permissions or prefix. The installer must restart Full.
            if matches!(self.mode, TransferMode::Resume { .. }) {
                return Err(TransferError::RestartRequired);
            }
            self.gate = Gate::Redirect(text(&self.headers, "location")?.ok_or(NetError::Protocol)?);
            return Ok(());
        }
        if self.status == 416 {
            if !matches!(self.mode, TransferMode::Resume { offset, .. } if offset == self.expected)
                || range.as_deref() != Some(format!("bytes */{}", self.expected).as_str())
            {
                return Err(TransferError::InvalidRange);
            }
            self.gate = Gate::Verify;
            return Ok(());
        }
        if matches!(self.status, 429 | 503) {
            let retry_after_seconds = text(&self.headers, "retry-after")?
                .map(|value| {
                    // This slice supports delta-seconds only; never retry early on
                    // an HTTP-date which we have not interpreted.
                    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(TransferError::UnsupportedRetryAfter);
                    }
                    value
                        .parse::<u64>()
                        .map_err(|_| TransferError::UnsupportedRetryAfter)
                })
                .transpose()?;
            return Err(TransferError::HttpRetry {
                status: self.status,
                retry_after_seconds,
            });
        }
        let start = match (&self.mode, self.status) {
            (_, 200) => {
                if range.is_some() {
                    return Err(TransferError::InvalidRange);
                }
                0
            }
            (
                TransferMode::Resume {
                    offset,
                    etag: expected_etag,
                },
                206,
            ) => {
                if etag.as_deref() != Some(expected_etag.0.as_str()) {
                    return Err(TransferError::RestartRequired);
                }
                let wanted = format!("bytes {}-{}/{}", offset, self.expected - 1, self.expected);
                if range.as_deref() != Some(wanted.as_str()) {
                    return Err(TransferError::InvalidRange);
                }
                *offset
            }
            (_, 206) => return Err(TransferError::InvalidRange),
            (_, code) => return Err(TransferError::HttpStatus(code)),
        };
        if length.is_some_and(|n| n != self.expected - start) {
            return Err(TransferError::SizeMismatch);
        }
        let accepted = if self.status == 206 {
            AcceptedArtifactHeaders::Resume {
                etag: StrongEtag::new(etag.as_deref().ok_or(TransferError::InvalidRange)?)?,
            }
        } else {
            AcceptedArtifactHeaders::Full {
                etag: etag
                    .as_deref()
                    .and_then(|value| StrongEtag::new(value).ok()),
            }
        };
        self.file
            .accepted_headers(accepted)
            .map_err(|_| TransferError::Sink)?;
        if start == 0 {
            self.file.truncate(0).map_err(|_| TransferError::Sink)?;
        }
        self.file
            .seek(SeekFrom::Start(start))
            .map_err(|_| TransferError::Sink)?;
        self.gate = Gate::Accept;
        Ok(())
    }
}
impl<S: ArtifactFile> Handler for Worker<S> {
    fn header(&mut self, data: &[u8]) -> bool {
        // No trailers or second final response may reclassify/reset the file.
        // This check precedes status-line and empty-line handling deliberately.
        if !matches!(self.gate, Gate::Pending) {
            return self.fail(NetError::Protocol.into());
        }
        if let Err(error) = self.check_live() {
            return self.fail(error);
        }
        self.last_progress = Instant::now();
        self.header_bytes = self.header_bytes.saturating_add(data.len());
        self.header_count += 1;
        if self.header_bytes > HEADER_BYTES || self.header_count > HEADER_COUNT {
            return self.fail(NetError::HeaderLimit.into());
        }
        if data.starts_with(b"HTTP/") {
            let Some(status) = std::str::from_utf8(data)
                .ok()
                .and_then(|s| s.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u16>().ok())
            else {
                return self.fail(NetError::Protocol.into());
            };
            self.status = status;
            self.headers.clear();
            return true;
        }
        if data == b"\r\n" {
            if (100..200).contains(&self.status) {
                return true;
            }
            if let Err(error) = self.finish_headers() {
                return self.fail(error);
            }
            if let Err(error) = self.check_job_budget() {
                return self.fail(error);
            }
            // Validated headers and completed durable sink preparation are
            // progress, just like a completed blocking write callback. Their
            // execution time remains charged to the unchanged total deadline.
            self.last_progress = Instant::now();
            return matches!(self.gate, Gate::Accept);
        }
        let Some(colon) = data.iter().position(|b| *b == b':') else {
            return self.fail(NetError::Protocol.into());
        };
        let Ok(name) = HeaderName::from_bytes(&data[..colon]) else {
            return self.fail(NetError::Protocol.into());
        };
        let value = data[colon + 1..].trim_ascii();
        let Ok(value) = HeaderValue::from_bytes(value) else {
            return self.fail(NetError::Protocol.into());
        };
        self.headers.append(name, value);
        true
    }
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        let result = (|| {
            self.check_live()?;
            if !matches!(self.gate, Gate::Accept) {
                return Err(NetError::Protocol.into());
            }
            let next = self
                .received
                .checked_add(data.len() as u64)
                .ok_or(NetError::BodyLimit)?;
            let start = if self.status == 206 {
                match self.mode {
                    TransferMode::Resume { offset, .. } => offset,
                    _ => 0,
                }
            } else {
                0
            };
            if next > self.max_bytes || next > self.expected - start {
                return Err(NetError::BodyLimit.into());
            }
            self.file.write_all(data).map_err(|_| TransferError::Sink)?;
            self.received = next;
            self.cancel
                .0
                .received
                .fetch_add(data.len() as u64, Ordering::Relaxed);
            self.last_progress = Instant::now();
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error);
            return Ok(0);
        }
        Ok(data.len())
    }
    fn progress(&mut self, _: f64, _: f64, _: f64, _: f64) -> bool {
        match self.check_live() {
            Ok(()) => true,
            Err(error) => self.fail(error),
        }
    }
}

fn run_hop<S: ArtifactFile>(
    worker: Worker<S>,
    endpoint: Endpoint,
    addresses: Vec<SocketAddr>,
    headers: HeaderMap,
    connect_timeout: Duration,
    remaining: Duration,
    #[cfg(test)] fixture: (bool, bool),
) -> Result<Worker<S>, TransferError> {
    struct Active(TransferCancellation);
    impl Drop for Active {
        fn drop(&mut self) {
            self.0.0.active.fetch_sub(1, Ordering::Relaxed);
        }
    }
    worker.cancel.0.active.fetch_add(1, Ordering::Relaxed);
    let _active = Active(worker.cancel.clone());
    if remaining.is_zero() {
        return Err(NetError::Timeout(TimeoutPhase::Total).into());
    }
    let offset = match worker.mode {
        TransferMode::Resume { offset, .. } => offset,
        TransferMode::Full => 0,
    };
    if worker.file.file_len().map_err(|_| TransferError::Sink)? != offset {
        return Err(TransferError::InvalidRequest);
    }
    let mut easy = Easy2::new(CurlHandler(Some(worker)));
    let operation = (|| -> Result<(), TransferError> {
        let set = |_: curl::Error| TransferError::Network(NetError::Protocol);
        easy.url(endpoint.url.as_str()).map_err(set)?;
        easy.proxy("").map_err(set)?;
        easy.noproxy("*").map_err(set)?;
        easy.netrc(NetRc::Ignored).map_err(set)?;
        easy.follow_location(false).map_err(set)?;
        easy.fresh_connect(true).map_err(set)?;
        easy.forbid_reuse(true).map_err(set)?;
        easy.ssl_verify_peer(true).map_err(set)?;
        easy.ssl_verify_host(true).map_err(set)?;
        easy.ssl_version(SslVersion::Tlsv12).map_err(set)?;
        easy.http_version(HttpVersion::V11).map_err(set)?;
        easy.http_content_decoding(false).map_err(set)?;
        // Fixed per-transfer receive buffer amortizes synchronous streaming
        // writes; active workers remain bounded by the broker/job budgets.
        easy.buffer_size(64 * 1024).map_err(set)?;
        easy.connect_timeout(connect_timeout).map_err(set)?;
        easy.progress(true).map_err(set)?;
        easy.useragent("CobbleLauncher/0.1").map_err(set)?;
        let mut resolve = List::new();
        let ips = addresses
            .iter()
            .map(|a| match a.ip() {
                IpAddr::V4(ip) => ip.to_string(),
                IpAddr::V6(ip) => format!("[{ip}]"),
            })
            .collect::<Vec<_>>()
            .join(",");
        resolve
            .append(&format!("{}:443:{ips}", endpoint.host_header()))
            .map_err(set)?;
        easy.resolve(resolve).map_err(set)?;
        #[cfg(test)]
        if fixture.0 {
            let mut connect = List::new();
            connect
                .append(&format!(
                    "{}:443:{}:{}",
                    endpoint.host_header(),
                    addresses[0].ip(),
                    addresses[0].port()
                ))
                .map_err(set)?;
            easy.connect_to(connect).map_err(set)?;
            if fixture.1 {
                easy.ssl_cainfo_blob(include_bytes!("../tests/fixtures/synthetic-root.pem"))
                    .map_err(set)?;
                // The original offline test CA has no CRL/OCSP responder. Only
                // this private synthetic trust path omits Schannel revocation;
                // CA signatures, validity and hostname checks remain enabled.
                #[cfg(windows)]
                {
                    let mut options = curl::easy::SslOpt::new();
                    options.no_revoke(true);
                    easy.ssl_options(&options).map_err(set)?;
                }
            }
        }
        let mut request_headers = List::new();
        request_headers
            .append("Accept-Encoding: identity")
            .map_err(set)?;
        request_headers.append("Connection: close").map_err(set)?;
        for (name, value) in headers.iter() {
            request_headers
                .append(&format!(
                    "{}: {}",
                    name.as_str(),
                    value.to_str().map_err(|_| NetError::CredentialScope)?
                ))
                .map_err(set)?;
        }
        if let TransferMode::Resume { offset, etag } = &easy.get_ref().state().mode {
            let range = format!("{offset}-");
            request_headers
                .append(&format!("If-Range: {}", etag.0))
                .map_err(set)?;
            easy.range(&range).map_err(set)?;
        }
        easy.http_headers(request_headers).map_err(set)?;
        // Blocking-pool queueing, file checks and native handle setup consume
        // the total budget, not an idle-body interval before HTTP even starts.
        let left = easy
            .get_ref()
            .state()
            .deadline
            .saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(NetError::Timeout(TimeoutPhase::Total).into());
        }
        easy.timeout(left).map_err(set)?;
        easy.get_mut().state_mut().last_progress = Instant::now();
        let result = easy.perform();
        if let Some(error) = easy.get_ref().state().error {
            return Err(error);
        }
        if matches!(easy.get_ref().state().gate, Gate::Pending) {
            return Err(match result {
                Err(error) if error.is_operation_timedout() => {
                    NetError::Timeout(TimeoutPhase::Total)
                }
                Err(error)
                    if error.is_peer_failed_verification() || error.is_ssl_connect_error() =>
                {
                    NetError::Tls
                }
                _ => NetError::Protocol,
            }
            .into());
        }
        // Additional post-transfer observation, never advertised as a pre-body check.
        let peer = easy
            .primary_ip()
            .map_err(set)?
            .and_then(|s| s.parse::<IpAddr>().ok());
        let port = easy.primary_port().map_err(set)?;
        if !addresses
            .iter()
            .any(|a| Some(a.ip()) == peer && a.port() == port)
        {
            return Err(NetError::PeerMismatch.into());
        }
        if matches!(easy.get_ref().state().gate, Gate::Accept) {
            if let Err(error) = result {
                return Err(if error.is_operation_timedout() {
                    NetError::Timeout(TimeoutPhase::Total).into()
                } else {
                    NetError::Protocol.into()
                });
            }
            let state = easy.get_mut().state_mut();
            let wanted = state.expected - if state.status == 206 { offset } else { 0 };
            if state.received != wanted {
                return Err(TransferError::SizeMismatch);
            }
            state.file.flush().map_err(|_| TransferError::Sink)?;
        } else if matches!(easy.get_ref().state().gate, Gate::Pending) {
            return Err(NetError::Protocol.into());
        }
        Ok(())
    })();
    // No Drop checkpoint: explicit finalization always happens on this actual
    // blocking worker, even if its async caller has already been cancelled.
    let finalized = easy.get_mut().state_mut().file.finish_transfer();
    let state = easy.get_mut().0.take().expect("owned worker");
    drop(easy);
    finalized.map_err(|_| TransferError::Sink)?;
    operation?;
    state.check_job_budget()?;
    Ok(state)
}

fn text(headers: &HeaderMap, name: &str) -> Result<Option<String>, TransferError> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .map(|v| {
            v.to_str()
                .map(str::to_owned)
                .map_err(|_| TransferError::Network(NetError::Protocol))
        })
        .transpose()?;
    if values.next().is_some() {
        return Err(NetError::Protocol.into());
    }
    Ok(value)
}
fn number(headers: &HeaderMap, name: &str) -> Result<Option<u64>, TransferError> {
    text(headers, name)?
        .map(|s| {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(NetError::Protocol.into());
            }
            s.parse().map_err(|_| NetError::Protocol.into())
        })
        .transpose()
}

#[cfg(test)]
mod curseforge_redirect_tests {
    use super::*;
    use crate::tests::{Server, config, raw};
    #[test]
    fn curseforge_grant_is_finite_and_has_no_signed_url_or_path_escape() {
        let edge = Endpoint::parse("https://edge.forgecdn.net/files/1/a.jar").unwrap();
        let grant = SameOriginGrant::new(&edge).with_curseforge(true);
        assert!(
            grant.permits(
                &Endpoint::parse("https://mediafilez.forgecdn.net/files/1/a.jar").unwrap()
            )
        );
        for url in [
            "https://mediafilez.forgecdn.net/other/a.jar",
            "https://evil.invalid/files/1/a.jar",
            "https://mediafilez.forgecdn.net/files/1/a.jar?token=synthetic",
            "https://edge.forgecdn.net.evil.invalid/files/1/a.jar",
            "https://mediafilez.forgecdn.net/files/%2e%2e/other/a.jar",
        ] {
            assert!(!grant.permits(&Endpoint::parse(url).unwrap()));
        }
    }
    #[tokio::test]
    async fn approved_redirect_strips_key_and_unapproved_origin_or_path_is_never_contacted() {
        for (target, allowed) in [
            ("https://two.fixture.invalid/files/a.jar", true),
            ("https://wrong.fixture.invalid/files/a.jar", false),
            ("https://two.fixture.invalid/other/a.jar", false),
        ] {
            let server=Server::new(move |request| {
                if request.to_ascii_lowercase().contains("host: one.fixture.invalid") {raw(format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"))} else {raw(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef".to_vec())}
            }).await;
            let broker = server.broker(config());
            let credential = broker
                .register_credentials(
                    crate::CredentialScope::new("https://one.fixture.invalid", "/files/").unwrap(),
                    crate::ProviderCredentials {
                        bearer: None,
                        api_key: Some("synthetic-private-key".into()),
                        cookie: None,
                    },
                )
                .unwrap();
            let start = Endpoint::parse("https://one.fixture.invalid/files/a.jar").unwrap();
            let mut grant = SameOriginGrant::new(&start).with_curseforge(true);
            // The production constructor above is tested against real official
            // origins; only the local TLS transport fixture uses synthetic names.
            grant.official_origins = vec![
                start.origin(),
                Endpoint::parse("https://two.fixture.invalid/files/a.jar")
                    .unwrap()
                    .origin(),
            ];
            let path = std::env::temp_dir().join(format!(
                "cobble-cf-redirect-{}-{}.part",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            struct Cleanup(std::path::PathBuf);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    let _ = std::fs::remove_file(&self.0);
                }
            }
            let _cleanup = Cleanup(path.clone());
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            let result = broker
                .artifact_transfer_granted(
                    ArtifactTransferRequest {
                        url: start.url.to_string(),
                        credential: Some(credential),
                        expected_size: 6,
                        max_bytes: 6,
                        mode: TransferMode::Full,
                    },
                    file,
                    TransferCancellation::default(),
                    grant,
                )
                .await;
            let captured = server.captured();
            assert!(captured[0].contains("synthetic-private-key"));
            if allowed {
                assert!(result.is_ok());
                assert_eq!(captured.len(), 2);
                assert!(!captured[1].to_ascii_lowercase().contains("x-api-key"));
                assert!(!captured[1].contains("synthetic-private-key"));
            } else {
                assert!(matches!(result, Err(TransferError::SourceNotAuthorized)));
                assert_eq!(captured.len(), 1);
            }
        }
    }
}
