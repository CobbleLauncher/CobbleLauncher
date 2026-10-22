// SPDX-License-Identifier: MIT
use std::{path::PathBuf, process::ExitCode};

const USAGE: &str = "Usage: cobble play <version> --dir <folder> [--name <offline-name>]";

pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(version) = args.next().filter(|value| !value.starts_with("--")) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let mut directory = None;
    let mut name = None;
    while let Some(option) = args.next() {
        let target = match option.as_str() {
            "--dir" if directory.is_none() => &mut directory,
            "--name" if name.is_none() => &mut name,
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        };
        *target = args.next().filter(|value| !value.starts_with("--"));
        if target.is_none() {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    }
    let Some(directory) = directory.map(PathBuf::from) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match play(&version, &directory, name.as_deref()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn play(
    version: &str,
    directory: &std::path::Path,
    name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let identity = match name {
            Some(name) => {
                cobble_auth::LaunchIdentity::offline(&cobble_auth::OfflineProfile::new(name)?)?
            }
            None => cobble_auth::launch_identity(None).await?,
        };
        let mut game = cobble_core::start_game_with_identity(
            version,
            directory,
            &identity,
            &cobble_minecraft::LaunchOverrides::default(),
        )
        .await?;
        println!(
            "Minecraft started with {} profile {}.",
            if identity.user_type == "msa" {
                "Microsoft"
            } else {
                "offline"
            },
            identity.name
        );
        println!("Game log: {}", game.log_path.display());
        let status = game.child.wait().await?;
        if !status.success() {
            return Err(format!(
                "Minecraft exited with {status}; see {}",
                game.log_path.display()
            )
            .into());
        }
        println!("Minecraft exited successfully.");
        Ok(())
    })
}
