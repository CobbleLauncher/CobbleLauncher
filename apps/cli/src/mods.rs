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
        eprintln!("Usage: cobble mods-search <minecraft> <query>");
        return ExitCode::from(2);
    };
    let query = args.collect::<Vec<_>>().join(" ");
    if query.is_empty() {
        eprintln!("Usage: cobble mods-search <minecraft> <query>");
        return ExitCode::from(2);
    }
    result((|| {
        let matches =
            runtime()?.block_on(cobble_content::search_modrinth(&query, &mc, "fabric", 0))?;
        println!(
            "{} matching Fabric mods (first {}):",
            matches.total_hits,
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
        validate_target(&mc, directory)?;
        runtime()?.block_on(async {
            let plan = cobble_content::resolve_modrinth(&project, &mc, "fabric").await?;
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

fn validate_target(mc: &str, directory: &Path) -> Result<(), Error> {
    let base: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(directory.join(".cobble-install.json"))?)?;
    let fabric: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(directory.join(".cobble-fabric.json"))?)?;
    if base["version"] != mc
        || base["complete"] != true
        || base["client_only"] != false
        || fabric["mc_version"] != mc
        || fabric["complete"] != true
        || fabric["profile"]["inheritsFrom"] != mc
        || fabric["profile"]["mainClass"] != "net.fabricmc.loader.impl.launch.knot.KnotClient"
    {
        return Err(
            "select a complete matching Fabric installation; stop the game before installing mods"
                .into(),
        );
    }
    Ok(())
}
