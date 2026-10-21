// SPDX-License-Identifier: MIT
//! Explicit production-network smoke probe: reference bytes -> durable download
//! -> verified CAS -> verified reopen. The reference is observed over HTTPS;
//! this does not independently authenticate provider metadata or game ownership.
use cap_std::fs::Dir;
use cobble_net::{
    ArtifactDownloader, Broker, BrokerConfig, DownloadAuthority, DownloadRequest,
    TransferCancellation,
};
use cobble_store::{
    ArtifactExpectation, ContentStore, DownloadStaging, ExpectedDigests, IntegrityBasis,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

struct ProbeRoot(PathBuf);
impl ProbeRoot {
    fn create() -> std::io::Result<Self> {
        for _ in 0..16 {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(std::io::Error::other)?
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "cobble-download-probe-{}-{nonce:x}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::other(
            "exclusive probe directory unavailable",
        ))
    }
}
impl Drop for ProbeRoot {
    fn drop(&mut self) {
        // Only this process's successfully create_dir-owned probe tree.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn register(path: &Path) -> std::io::Result<Dir> {
    Dir::open_ambient_dir(path, cap_std::ambient_authority())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = ProbeRoot::create()?;
    let staging = root.0.join("staging");
    let content = root.0.join("content");
    std::fs::create_dir(&staging)?;
    std::fs::create_dir(&content)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result: Result<(), Box<dyn std::error::Error>> = runtime.block_on(async {
        let broker = Broker::new(BrokerConfig::default())?;
        let url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
        let reference = broker.get(url, None, 4 * 1024 * 1024).await?;
        if reference.metadata.status != 200 || reference.body.is_empty() {
            return Err("reference HTTPS response failed".into());
        }
        let expected: [u8; 32] = Sha256::digest(&reference.body).into();
        let expectation = ArtifactExpectation {
            expected_size: Some(reference.body.len() as u64),
            digests: ExpectedDigests {
                sha256: Some(expected),
                ..ExpectedDigests::default()
            },
            max_bytes: 4 * 1024 * 1024,
            basis: IntegrityBasis::TrustedMetadata,
        };
        let downloader = ArtifactDownloader::new(
            broker,
            DownloadStaging::open(register(&staging)?)?,
            ContentStore::open(register(&content)?)?,
        );
        let request = DownloadRequest::new(
            url,
            expectation,
            DownloadAuthority::same_origin("mojang", "manifest-probe", "explicit-public-probe", None)?,
            None,
        )?;
        let result = downloader
            .download(request, TransferCancellation::default())
            .await?;
        if result.artifact.sha256() != expected
            || result.artifact.size_bytes() != reference.body.len() as u64
        {
            return Err("verified artifact differs from reference bytes".into());
        }
        drop(downloader);
        let content_path = content.clone();
        let reopened = tokio::task::spawn_blocking(move || {
            let store = ContentStore::open(register(&content_path).map_err(cobble_store::CasError::Io)?)?;
            store.open_verified(expected, expectation)
        })
        .await??;
        if reopened != result.artifact {
            return Err("reopened CAS artifact differs from the verified download".into());
        }
        println!(
            "bytes={} sha256={} reference_match=true cas_reopen_match={} staging_retained={} durability={:?}",
            result.artifact.size_bytes(),
            result.artifact.sha256_hex(),
            reopened == result.artifact,
            result.staging_retained,
            result.publication_durability,
        );
        Ok(())
    });
    // Caller cancellation does not terminate spawn_blocking. Wait for owned
    // workers before deleting this probe's temporary capability roots.
    drop(runtime);
    result
}
