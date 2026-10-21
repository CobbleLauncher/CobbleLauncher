// SPDX-License-Identifier: MIT
//! Command-line entry point for explicit launcher operations.

use cobble_minecraft::{DownloadSource, VersionCatalog, VersionKind};
use cobble_net::{Broker, BrokerConfig};
use std::io::{self, Write};
use std::process::ExitCode;
mod install;
mod mods;
mod play;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("--help" | "-h") => {
            println!(
                "Cobble Launcher\n\nUsage:\n  cobble versions          List the latest 20 releases\n  cobble versions --all    List all versions, including snapshots\n  cobble fabric-versions <version>  List compatible Fabric loaders\n  cobble info <version>    Show Java requirements and download size\n  cobble install <version> --dir <folder> [--client-only | --fabric <loader-version>] [--source official|bmclapi | --mirror <https-base>]\n  cobble mods-search <minecraft> <query>  Search compatible Fabric mods\n  cobble mod-install <minecraft> <project-id> --dir <folder>  Install mods into a stopped Fabric instance\n  cobble java <major>      Find a matching installed Java runtime\n  cobble launch-info <version> --dir <folder>  Prepare an installed game without launching\n  cobble play <version> --dir <folder> [--name <offline-name>]  Launch with an explicit offline profile\n  cobble --version\n\nOffline profiles do not authenticate a Microsoft account."
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
        Some("fabric-versions") => {
            let Some(version) = args.next().filter(|_| args.next().is_none()) else {
                eprintln!("Usage: cobble fabric-versions <minecraft-version>");
                return ExitCode::from(2);
            };
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())
                .and_then(|runtime| {
                    runtime
                        .block_on(cobble_core::list_fabric_versions(&version))
                        .map_err(|error| error.to_string())
                });
            match result {
                Ok(versions) => {
                    for loader in versions {
                        println!(
                            "{}{}",
                            loader.version,
                            if loader.stable { " (stable)" } else { "" }
                        );
                    }
                    ExitCode::SUCCESS
                }
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
        Some("play") => play::run(args),
        Some("mods-search") => mods::search(args),
        Some("mod-install") => mods::install(args),
        Some("launch-info") => {
            let version = args.next();
            let option = args.next();
            let directory = args.next();
            let (Some(version), Some("--dir"), Some(directory), None) =
                (version, option.as_deref(), directory, args.next())
            else {
                eprintln!("Usage: cobble launch-info <version> --dir <folder>");
                return ExitCode::from(2);
            };
            match launch_info(&version, std::path::Path::new(&directory)) {
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
                    "Usage: cobble install <version> --dir <folder> [--client-only | --fabric <loader-version>] [--source official|bmclapi | --mirror <https-base>]"
                );
                return ExitCode::from(2);
            };
            let mut target = None;
            let mut client_only = false;
            let mut fabric = None;
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
                    "--fabric" if fabric.is_none() => {
                        fabric = args.next().filter(|value| !value.starts_with("--"));
                        if fabric.is_none() {
                            eprintln!("--fabric requires a loader version");
                            return ExitCode::from(2);
                        }
                    }
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
                            "Usage: cobble install <version> --dir <folder> [--client-only | --fabric <loader-version>] [--source official|bmclapi | --mirror <https-base>]"
                        );
                        return ExitCode::from(2);
                    }
                }
            }
            let Some(target) = target else {
                eprintln!(
                    "Usage: cobble install <version> --dir <folder> [--client-only | --fabric <loader-version>] [--source official|bmclapi | --mirror <https-base>]"
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
            if client_only && fabric.is_some() {
                eprintln!("--fabric cannot be combined with --client-only");
                return ExitCode::from(2);
            }
            match install::run(&version, &target, client_only, source, fabric.as_deref()) {
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

fn launch_info(
    version: &str,
    directory: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let plan = cobble_core::prepare_launch(version, directory).await?;
        let java = cobble_minecraft::resolve_java(plan.required_java).await?;
        println!("Java {} · {} · {}", java.version, java.vendor, java.arch);
        println!("Executable: {}", java.executable.display());
        println!("Main class: {}", plan.main_class);
        println!("Classpath: {} entries", plan.classpath_entries);
        println!(
            "Arguments: {} JVM, {} game",
            plan.jvm_arguments.len(),
            plan.game_arguments.len()
        );
        println!(
            "Natives: {} files in {}",
            plan.native_files,
            plan.natives_directory.display()
        );
        println!("Launch preparation complete. Use play for an explicit offline-profile launch.");
        Ok(())
    })
}
