// SPDX-License-Identifier: MIT
use cobble_minecraft::DownloadSource;
use std::{path::PathBuf, process::ExitCode, sync::Arc};

pub fn info(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next().filter(|_| args.next().is_none()) else {
        eprintln!("Usage: cobble pack-info <file.mrpack>");
        return ExitCode::from(2);
    };
    match cobble_content::read_mrpack(std::path::Path::new(&path)) {
        Ok(pack) => {
            println!("{} · {}", pack.name, pack.version_id);
            println!("Minecraft {}", pack.minecraft_version);
            if let Some(loader) = pack.loader_version {
                println!("Fabric {loader}");
            }
            if let Some(summary) = pack.summary {
                println!("{summary}");
            }
            println!(
                "{} client files, {} overrides",
                pack.files.len(),
                pack.overrides.len()
            );
            for file in pack.files.iter().filter(|file| file.optional) {
                println!("Optional (off): {}", file.path);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

pub fn install(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(archive) = args.next() else {
        return usage();
    };
    let mut target = None;
    let mut source = None;
    let mut optional = Vec::new();
    while let Some(option) = args.next() {
        match option.as_str() {
            "--dir" if target.is_none() => {
                target = args
                    .next()
                    .filter(|value| !value.starts_with("--"))
                    .map(PathBuf::from);
                if target.is_none() {
                    return usage();
                }
            }
            "--source" if source.is_none() => {
                source = match args.next().as_deref() {
                    Some("official") => Some(DownloadSource::Official),
                    Some("bmclapi") => Some(DownloadSource::Bmclapi),
                    _ => return usage(),
                };
            }
            "--optional" => {
                let Some(path) = args.next().filter(|value| !value.starts_with("--")) else {
                    return usage();
                };
                optional.push(path);
            }
            _ => return usage(),
        }
    }
    let Some(target) = target else {
        return usage();
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let source = match source {
            Some(source) => source,
            None => cobble_minecraft::read_download_settings()?.download_source()?,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let outcome = runtime.block_on(cobble_core::install_mrpack(
            std::path::Path::new(&archive),
            &target,
            source,
            &optional,
            Arc::new(|event| {
                let every = if event.total > 256 { 256 } else { 8 };
                if event.completed == 0
                    || event.completed == event.total
                    || event.completed.is_multiple_of(every)
                {
                    eprintln!("{} {}/{}", event.phase, event.completed, event.total);
                }
            }),
        ))?;
        println!(
            "Installed {} · {} into {}",
            outcome.name,
            outcome.version_id,
            target.display()
        );
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "Usage: cobble pack-install <file.mrpack> --dir <folder> [--source official|bmclapi] [--optional <path> ...]"
    );
    ExitCode::from(2)
}
