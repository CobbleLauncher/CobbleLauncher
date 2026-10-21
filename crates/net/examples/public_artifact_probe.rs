// SPDX-License-Identifier: MIT
//! Explicit network probe of the production libcurl adapter. No provider token,
//! fixture routing or additional certificate trust is involved.
use cobble_net::{
    ArtifactTransferRequest, Broker, BrokerConfig, TransferCancellation, TransferMode,
};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "cobble-transfer-probe-{}-{nonce:x}.part",
        std::process::id()
    ));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result: Result<(), Box<dyn std::error::Error>> = runtime.block_on(async move {
        let version = curl::Version::get();
        println!(
            "libcurl={} tls={}",
            version.version(),
            version.ssl_version().unwrap_or("unavailable")
        );
        let broker = Broker::new(BrokerConfig::default())?;
        let url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
        let reference = broker.get(url, None, 4 * 1024 * 1024).await?;
        if reference.metadata.status != 200 || reference.body.is_empty() {
            return Err("reference metadata probe failed".into());
        }
        let expected = Sha256::digest(&reference.body);
        let mut receipt = broker
            .artifact_transfer(
                ArtifactTransferRequest {
                    url: url.to_owned(),
                    credential: None,
                    expected_size: reference.body.len() as u64,
                    max_bytes: 4 * 1024 * 1024,
                    mode: TransferMode::Full,
                },
                file,
                TransferCancellation::default(),
            )
            .await?;
        receipt.file.seek(SeekFrom::Start(0))?;
        let mut body = Vec::new();
        receipt.file.read_to_end(&mut body)?;
        let actual = Sha256::digest(&body);
        if actual != expected || body.len() != reference.body.len() {
            return Err("artifact transport differs from the reference metadata bytes".into());
        }
        println!(
            "origin={} status={} bytes={} redirects={} sha256={actual:x} reference_match=true",
            receipt.metadata.final_origin,
            receipt.metadata.status,
            receipt.metadata.body_bytes,
            receipt.metadata.redirects
        );
        Ok(())
    });
    // A timed-out caller does not terminate spawn_blocking. Runtime shutdown
    // waits for those workers before this development probe removes its file.
    drop(runtime);
    let cleanup = std::fs::remove_file(path);
    result?;
    cleanup?;
    Ok(())
}
