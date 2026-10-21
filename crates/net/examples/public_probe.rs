// SPDX-License-Identifier: MIT
//! Explicit public metadata probe; never run as part of the offline test suite.
//! Uses the production broker without test routing or additional trust roots.
use cobble_net::{Broker, BrokerConfig};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let broker = Broker::new(BrokerConfig::default())?;
        for url in [
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
            "https://meta.fabricmc.net/v2/versions/game",
        ] {
            let response = broker.get(url, None, 4 * 1024 * 1024).await?;
            println!(
                "origin={} status={} bytes={} redirects={} sha256={:x}",
                response.metadata.final_origin,
                response.metadata.status,
                response.metadata.body_bytes,
                response.metadata.redirects,
                Sha256::digest(&response.body)
            );
            if response.metadata.status != 200 || response.body.is_empty() {
                return Err("public metadata probe did not return a nonempty HTTP 200 body".into());
            }
        }
        Ok(())
    })
}
