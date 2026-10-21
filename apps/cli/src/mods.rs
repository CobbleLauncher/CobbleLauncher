// SPDX-License-Identifier: MIT
use std::{path::Path, process::ExitCode, sync::Arc};
type Error = Box<dyn std::error::Error + Send + Sync>;

fn runtime() -> Result<tokio::runtime::Runtime, Error> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}
fn result(result: Result<(), Error>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

pub fn search(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(mc) = args.next() else {
        eprintln!("Usage: cobble mods-search <minecraft> <query> [--loader fabric|forge|neoforge]");
        return ExitCode::from(2);
    };
    let (query, loader) = match search_options(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    result((|| {
        let matches =
            runtime()?.block_on(cobble_content::search_modrinth(&query, &mc, &loader, 0))?;
        println!(
            "{} matching {} mods (first {}):",
            matches.total_hits,
            loader,
            matches.hits.len()
        );
        for project in matches.hits {
            println!(
                "{}\t{}\t{}",
                project.project_id, project.slug, project.title
            );
        }
        Ok(())
    })())
}

pub fn install(mut args: impl Iterator<Item = String>) -> ExitCode {
    let (Some(mc), Some(project), Some(option), Some(directory), None) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    ) else {
        eprintln!("Usage: cobble mod-install <minecraft> <project-id> --dir <folder>");
        return ExitCode::from(2);
    };
    if option != "--dir" {
        eprintln!("Usage: cobble mod-install <minecraft> <project-id> --dir <folder>");
        return ExitCode::from(2);
    }
    result((|| {
        let directory = Path::new(&directory);
        let loader = validate_target(&mc, directory)?;
        runtime()?.block_on(async {
            let plan = cobble_content::resolve_modrinth(&project, &mc, loader).await?;
            for selected in &plan.mods {
                println!(
                    "{} {} ({})",
                    selected.name, selected.version_number, selected.file.filename
                );
            }
            let outcome = cobble_core::install_modrinth_plan(
                &plan,
                directory,
                Arc::new(|event| {
                    eprintln!(
                        "{}: {}/{} ({} existing)",
                        event.phase, event.completed, event.total, event.reused
                    );
                }),
            )
            .await?;
            println!(
                "Installed {} mods; reused {} existing files.",
                outcome.installed, outcome.reused
            );
            Ok(())
        })
    })())
}

fn search_options(mut args: impl Iterator<Item = String>) -> Result<(String, String), Error> {
    let mut loader = None;
    let mut query = Vec::new();
    while let Some(argument) = args.next() {
        if argument == "--loader" {
            if loader.is_some() {
                return Err("specify --loader once".into());
            }
            let value = args
                .next()
                .ok_or("--loader requires fabric, forge or neoforge")?;
            if !matches!(value.as_str(), "fabric" | "forge" | "neoforge") {
                return Err("--loader requires fabric, forge or neoforge".into());
            }
            loader = Some(value);
        } else if argument.starts_with("--") {
            return Err(format!("unknown mods-search option: {argument}").into());
        } else {
            query.push(argument);
        }
    }
    let query = query.join(" ");
    if query.trim().is_empty() {
        return Err(
            "Usage: cobble mods-search <minecraft> <query> [--loader fabric|forge|neoforge]".into(),
        );
    }
    Ok((query, loader.unwrap_or_else(|| "fabric".into())))
}

fn validate_target(mc: &str, directory: &Path) -> Result<&'static str, Error> {
    let base: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(directory.join(".cobble-install.json"))?)?;
    if base["version"] != mc || base["complete"] != true || base["client_only"] != false {
        return Err(
            "select a complete matching installation; stop the game before installing mods".into(),
        );
    }
    let read_marker = |name: &str| -> Result<Option<serde_json::Value>, Error> {
        match std::fs::File::open(directory.join(name)) {
            Ok(file) => Ok(Some(serde_json::from_reader(file)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    };
    let fabric = read_marker(".cobble-fabric.json")?;
    let forge = read_marker(".cobble-forge.json")?;
    let (state, loader, main_class) = match (fabric, forge) {
        (Some(state), None) => (
            state,
            "fabric",
            "net.fabricmc.loader.impl.launch.knot.KnotClient",
        ),
        (None, Some(state)) => {
            let loader = match state["kind"].as_str() {
                Some("forge") => "forge",
                Some("neoforge") => "neoforge",
                _ => return Err("unsupported installed mod loader".into()),
            };
            (
                state,
                loader,
                "cpw.mods.bootstraplauncher.BootstrapLauncher",
            )
        }
        (Some(_), Some(_)) => return Err("installation contains conflicting loader markers".into()),
        (None, None) => return Err("select a Fabric, Forge or NeoForge installation".into()),
    };
    if state["mc_version"] != mc
        || state["complete"] != true
        || state["profile"]["inheritsFrom"] != mc
        || state["profile"]["mainClass"] != main_class
    {
        return Err("select a complete matching loader installation".into());
    }
    Ok(loader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn search_loader_defaults_and_options() {
        let parse = |args: &[&str]| search_options(args.iter().map(|value| value.to_string()));
        assert_eq!(
            parse(&["mod", "menu"]).unwrap(),
            ("mod menu".into(), "fabric".into())
        );
        for loader in ["fabric", "forge", "neoforge"] {
            assert_eq!(
                parse(&["test", "--loader", loader]).unwrap(),
                ("test".into(), loader.into())
            );
        }
        for args in [
            vec!["test", "--loader"],
            vec!["test", "--loader", "quilt"],
            vec!["--loader", "forge"],
            vec!["test", "--loader", "forge", "--loader", "fabric"],
        ] {
            assert!(parse(&args).is_err());
        }
    }

    #[test]
    fn install_loader_inference_requires_matching_complete_markers() {
        let directory = std::env::temp_dir().join(format!(
            "cobble-cli-loader-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(
            directory.join(".cobble-install.json"),
            serde_json::to_vec(&json!({"version":"1.21.1","complete":true,"client_only":false}))
                .unwrap(),
        )
        .unwrap();
        for loader in ["fabric", "forge", "neoforge"] {
            let (marker, main_class) = if loader == "fabric" {
                (
                    ".cobble-fabric.json",
                    "net.fabricmc.loader.impl.launch.knot.KnotClient",
                )
            } else {
                (
                    ".cobble-forge.json",
                    "cpw.mods.bootstraplauncher.BootstrapLauncher",
                )
            };
            let mut state = json!({"kind":loader,"mc_version":"1.21.1","complete":true,
                "profile":{"inheritsFrom":"1.21.1","mainClass":main_class}});
            let path = directory.join(marker);
            std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
            assert_eq!(validate_target("1.21.1", &directory).unwrap(), loader);
            assert!(validate_target("1.20.1", &directory).is_err());
            state["complete"] = json!(false);
            std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
            assert!(validate_target("1.21.1", &directory).is_err());
            std::fs::remove_file(path).unwrap();
        }
        assert!(validate_target("1.21.1", &directory).is_err());
        for marker in [".cobble-fabric.json", ".cobble-forge.json"] {
            std::fs::write(directory.join(marker), b"{}").unwrap();
        }
        assert!(
            validate_target("1.21.1", &directory)
                .unwrap_err()
                .to_string()
                .contains("conflicting")
        );
        for marker in [
            ".cobble-fabric.json",
            ".cobble-forge.json",
            ".cobble-install.json",
        ] {
            std::fs::remove_file(directory.join(marker)).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }
}
