// SPDX-License-Identifier: MIT
//! Explicit live Modrinth diagnostic; not part of the offline test suite.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let broker = cobble_net::Broker::new(cobble_net::BrokerConfig::default())?;
    match broker
        .get(
            "https://api.modrinth.com/v2/search?limit=1",
            None,
            1024 * 1024,
        )
        .await
    {
        Ok(response) => println!("direct_status={}", response.metadata.status),
        Err(error) => println!("direct_error={error}"),
    }
    let page = cobble_content::search_modrinth_packs("", None, None, 0).await?;
    println!(
        "pack_search_hits={} total={}",
        page.hits.len(),
        page.total_hits
    );
    assert!(!page.hits.is_empty());
    Ok(())
}
