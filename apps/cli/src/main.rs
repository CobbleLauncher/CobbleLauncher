// SPDX-License-Identifier: MIT
//! Command-line entry point for explicit launcher operations.

use cobble_minecraft::{VersionCatalog, VersionKind};
use cobble_net::{Broker, BrokerConfig};
use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("--help" | "-h") => {
            println!(
                "Cobble Launcher\n\nUsage:\n  cobble versions          List the latest 20 releases\n  cobble versions --all    List all versions, including snapshots\n  cobble --version\n\nGame installation and launch are not implemented yet."
            );
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            let info = cobble_core::app_info();
            println!("{} {} ({})", info.name, info.version, info.stage);
            ExitCode::SUCCESS
        }
        Some("versions") => {
            let all = match args.next().as_deref() {
                None => false,
                Some("--all") if args.next().is_none() => true,
                _ => {
                    eprintln!("Usage: cobble versions [--all]");
                    return ExitCode::from(2);
                }
            };
            match list_versions(all) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        Some(_) => {
            eprintln!("Unknown command. Run cobble --help for available commands.");
            ExitCode::from(2)
        }
    }
}

fn list_versions(all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let catalog = runtime.block_on(async {
        let broker = Broker::new(BrokerConfig::default())?;
        VersionCatalog::fetch(&broker)
            .await
            .map_err(Box::<dyn std::error::Error>::from)
    })?;
    let mut out = io::BufWriter::new(io::stdout().lock());
    writeln!(out, "Latest release: {}", catalog.latest.release)?;
    writeln!(out, "Latest snapshot: {}\n", catalog.latest.snapshot)?;
    writeln!(out, "  {:<24} {:<12} RELEASED", "VERSION", "TYPE")?;
    for version in catalog
        .versions
        .iter()
        .filter(|version| all || version.kind == VersionKind::Release)
        .take(if all { usize::MAX } else { 20 })
    {
        let latest = version.id == catalog.latest.release || version.id == catalog.latest.snapshot;
        writeln!(
            out,
            "{} {:<24} {:<12} {}",
            if latest { '*' } else { ' ' },
            version.id,
            version.kind.as_str(),
            version
                .release_time
                .get(..10)
                .unwrap_or(&version.release_time),
        )?;
    }
    out.flush()?;
    Ok(())
}
