// SPDX-License-Identifier: MIT
//! Explicit read-only inspection tool. Never starts a launcher, copies resources,
//! accesses account stores, changes configuration or performs network requests.
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let roots = std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Err("provide one or more explicit read-only search roots".into());
    }
    let candidates = cobble_core::discover_launcher_imports(&roots)?;
    for candidate in &candidates {
        println!(
            "{}",
            serde_json::json!({"name":candidate.name,"minecraft":candidate.minecraft_version,"loader":candidate.loader_kind,"loaderVersion":candidate.loader_version,"unsupported":candidate.unsupported_reason,"contentChoices":candidate.content_roots.iter().map(|root|&root.id).collect::<Vec<_>>()})
        );
    }
    println!("candidates={}", candidates.len());
    Ok(())
}
