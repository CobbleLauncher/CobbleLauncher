// SPDX-License-Identifier: MIT
//! Command-line entry point for explicit launcher operations.

use cobble_minecraft::{DownloadSource, VersionCatalog, VersionKind};
use cobble_net::{Broker, BrokerConfig};
use std::io::{self, Write};
use std::process::ExitCode;
mod install;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("--help" | "-h") => {
            println!(
                "Cobble Launcher\n\nUsage:\n  cobble versions          List the latest 20 releases\n  cobble versions --all    List all versions, including snapshots\n  cobble info <version>    Show Java requirements and download size\n  cobble install <version> --dir <folder> [--client-only] [--source official|bmclapi | --mirror <https-base>]\n  cobble java <major>      Find a matching installed Java runtime\n  cobble --version\n\nInstall downloads game files. Sign-in and launching follow separately."
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
        Some("info") => {
            let Some(version) = args.next().filter(|_| args.next().is_none()) else {
                eprintln!("Usage: cobble info <version>");
                return ExitCode::from(2);
            };
            match install::info(&version) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("java") => {
            let major = args.next().and_then(|value| value.parse::<u32>().ok());
            let Some(major) = major.filter(|major| *major > 0 && args.next().is_none()) else {
                eprintln!("Usage: cobble java <major>");
                return ExitCode::from(2);
            };
            match install::show_java(major) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("install") => {
            let Some(version) = args.next() else {
                eprintln!(
                    "Usage: cobble install <version> --dir <folder> [--client-only] [--source official|bmclapi | --mirror <https-base>]"
                );
                return ExitCode::from(2);
            };
            let mut target = None;
            let mut client_only = false;
            let mut source = None;
            let mut source_selected = false;
            while let Some(option) = args.next() {
                match option.as_str() {
                    "--dir" if target.is_none() => {
                        target = args
                            .next()
                            .filter(|value| !value.starts_with("--"))
                            .map(std::path::PathBuf::from);
                        if target.is_none() {
                            eprintln!("--dir requires a folder path");
                            return ExitCode::from(2);
                        }
                    }
                    "--client-only" if !client_only => client_only = true,
                    "--source" if !source_selected => {
                        source = Some(match args.next().as_deref() {
                            Some("official") => DownloadSource::Official,
                            Some("bmclapi") => DownloadSource::Bmclapi,
                            _ => {
                                eprintln!("--source requires official or bmclapi");
                                return ExitCode::from(2);
                            }
                        });
                        source_selected = true;
                    }
                    "--mirror" if !source_selected => {
                        source = Some(match args.next().as_deref().map(DownloadSource::custom) {
                            Some(Ok(source)) => source,
                            _ => {
                                eprintln!("--mirror requires a BMCLAPI-compatible HTTPS base URL");
                                return ExitCode::from(2);
                            }
                        });
                        source_selected = true;
                    }
                    _ => {
                        eprintln!(
                            "Usage: cobble install <version> --dir <folder> [--client-only] [--source official|bmclapi | --mirror <https-base>]"
                        );
                        return ExitCode::from(2);
                    }
                }
            }
            let Some(target) = target else {
                eprintln!(
                    "Usage: cobble install <version> --dir <folder> [--client-only] [--source official|bmclapi | --mirror <https-base>]"
                );
                return ExitCode::from(2);
            };
            let source = match source.map(Ok).unwrap_or_else(|| {
                cobble_minecraft::read_download_settings()
                    .map_err(|error| error.to_string())?
                    .download_source()
                    .map_err(|error| error.to_string())
            }) {
                Ok(source) => source,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };
            match install::run(&version, &target, client_only, source) {
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
        let source = cobble_minecraft::read_download_settings()?.download_source()?;
        VersionCatalog::fetch_from(&broker, &source)
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
