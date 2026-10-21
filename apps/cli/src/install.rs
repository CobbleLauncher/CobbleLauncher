// SPDX-License-Identifier: MIT
//! CLI presentation around shared Core installation.
use cobble_minecraft::{DownloadSource, PlatformContext, VersionCatalog};
use cobble_net::{Broker, BrokerConfig};
use std::{io, path::Path, sync::Arc};
type Error = Box<dyn std::error::Error + Send + Sync>;
pub fn info(version: &str) -> Result<(), Error> {
    let runtime = runtime()?;
    runtime.block_on(async {
        let broker = Broker::new(BrokerConfig::default())?;
        let source = cobble_minecraft::read_download_settings()?.download_source()?;
        let metadata = VersionCatalog::fetch_from(&broker, &source)
            .await?
            .fetch_version_from(&broker, version, &source)
            .await?;
        let plan = metadata.download_plan(&PlatformContext::current()?)?;
        println!("Minecraft {}", plan.version_id);
        println!("Java: {}", plan.java_major);
        println!("Main class: {}", plan.main_class);
        println!(
            "Client/libraries/index/logging: {} files, {:.1} MiB",
            plan.files.len(),
            plan.files.iter().map(|f| f.size).sum::<u64>() as f64 / 1_048_576.0
        );
        println!("Assets index: {}", plan.asset_index.id);
        Ok(())
    })
}

pub fn run(
    version: &str,
    target: &Path,
    client_only: bool,
    source: DownloadSource,
    fabric: Option<&str>,
) -> Result<(), Error> {
    let runtime = runtime()?;
    println!("Download source: {}", source.attribution());
    let progress = Arc::new(|progress: cobble_core::InstallProgress| {
        let report_every = if progress.total > 256 { 256 } else { 32 };
        if matches!(progress.phase.as_str(), "files" | "assets" | "fabric")
            && progress.completed > 0
            && (progress.completed.is_multiple_of(report_every)
                || progress.completed == progress.total)
        {
            eprintln!(
                "Files ready: {}/{} (reused: {})",
                progress.completed, progress.total, progress.reused
            );
        }
    });
    let result = runtime.block_on(async {
        match fabric {
            Some(loader) => {
                cobble_core::install_fabric(version, loader, target, source, progress).await
            }
            None => {
                cobble_core::install_resources(version, target, client_only, source, progress).await
            }
        }
    });
    // Blocking transfer/copy workers must finish before process exit.
    drop(runtime);
    let outcome = result?;
    println!(
        "Minecraft {} · Java {}",
        outcome.version, outcome.java_major
    );
    if client_only {
        println!(
            "Client downloaded to {} (libraries, assets and Java not installed)",
            target.display()
        );
    } else {
        println!(
            "Game files downloaded to {}. Java {} is required; use play to launch with an offline profile.",
            target.display(),
            outcome.java_major
        );
        if let Ok(java) = self::runtime()?.block_on(cobble_minecraft::find_java(outcome.java_major))
        {
            println!("Java {} found: {}", java.version, java.executable.display());
        }
    }
    Ok(())
}

fn runtime() -> io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

pub fn show_java(major: u32) -> Result<(), Error> {
    let java = runtime()?.block_on(cobble_minecraft::resolve_java(major))?;
    println!("Java {} · {} · {}", java.version, java.vendor, java.arch);
    println!("{}", java.executable.display());
    Ok(())
}
