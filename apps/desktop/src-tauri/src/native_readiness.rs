// SPDX-License-Identifier: MIT
//! Host-owned readiness plans authorize registered games, never renderer paths.
use super::*;
use cobble_core::readiness::{LocalReadiness, SelectedContent};
use serde::{Deserialize, Serialize};
#[path = "native_readiness_checks.rs"]
mod checks;

#[derive(Clone, Serialize, Deserialize)]
pub struct RepairPlan {
    version: String,
    loader_kind: Option<String>,
    loader_version: Option<String>,
    java: cobble_minecraft::JavaOverride,
    source: DownloadSettings,
    content_revision: String,
    pub(super) environment: Option<cobble_content::EnvironmentPlan>,
}
#[derive(Serialize)]
pub struct Condition {
    pub state: String,
    pub code: String,
}
#[derive(Serialize)]
pub struct ReadinessView {
    pub files: LocalReadiness,
    pub java: Condition,
    pub account: Condition,
    pub server: Condition,
}
fn condition(state: &str, code: &str) -> Condition {
    Condition {
        state: state.into(),
        code: code.into(),
    }
}

async fn inspect(
    game: &Game,
    control: cobble_core::OperationControl,
) -> Result<ReadinessView, String> {
    let started = Instant::now();
    let version = game.version.clone();
    let directory = game.directory.clone();
    let loader = game.loader_kind.clone();
    let loader_version = game.loader_version.clone();
    let file_control = control.clone();
    let files = tauri::async_runtime::spawn_blocking(move || {
        cobble_core::readiness::inspect_local_readiness(
            &version,
            &directory,
            loader.as_deref(),
            loader_version.as_deref(),
            file_control,
        )
    })
    .await
    .map_err(|_| "READINESS_CHECK_FAILED")?
    .map_err(|e| e.to_string())?;
    let java = if let Some(major) = files.required_java {
        let captured = cobble_core::capture_java_choice(&game.runtime_overrides.java).ok();
        let choice = captured.as_ref().unwrap_or(&game.runtime_overrides.java);
        let resolved = tokio::select! {value=cobble_minecraft::resolve_java_override(major,choice)=>value,
        _=control.cancelled()=>return Err("READINESS_CANCELLED".into())};
        match resolved {
            Ok(_) => condition("ready", "local_runtime_available"),
            Err(_) => match captured {
                Some(cobble_minecraft::JavaOverride::Automatic)
                    if cfg!(all(target_os = "windows", target_arch = "x86_64")) =>
                {
                    condition("missing", "java_unavailable")
                }
                Some(cobble_minecraft::JavaOverride::Automatic) => {
                    condition("missing", "managed_java_unsupported")
                }
                _ => condition("missing", "custom_java_unavailable"),
            },
        }
    } else {
        condition("unknown", "java_requirement_unavailable")
    };
    control
        .check_cancelled()
        .map_err(|_| "READINESS_CANCELLED")?;
    let account = match cobble_auth::get_player_profile(game.profile_id.as_deref()) {
        Ok(profile) if profile.kind == "offline" => condition("ready", "offline_profile"),
        Ok(_) => condition("unknown", "online_auth_not_checked"),
        Err(_) => condition("missing", "profile_unavailable"),
    };
    tracing::info!(event="local_readiness",elapsed_ms=started.elapsed().as_millis()as u64,
        files_ready=files.ready,checked=files.checked_files,missing=files.missing_files,changed=files.changed_files,
        unknown=files.unknown,java_state=%java.state,account_state=%account.state,server_state="unknown");
    Ok(ReadinessView {
        files,
        java,
        account,
        server: condition("unknown", "server_not_checked"),
    })
}

#[tauri::command]
pub async fn inspect_game_readiness(
    app: AppHandle,
    id: String,
    request_id: Option<String>,
) -> Result<ReadinessView, String> {
    let (control, _check) = checks::begin(request_id)?;
    let (game, _lease) = management::begin(&app, &id, true)?;
    inspect(&game, control).await
}

#[tauri::command]
pub fn cancel_game_readiness_check(request_id: String) -> Result<(), String> {
    checks::cancel(request_id)
}

fn validate_exact_content(
    selected: &[SelectedContent],
    environment: &cobble_content::EnvironmentPlan,
) -> Result<(), String> {
    let mut actual = Vec::new();
    for (source, graph) in [
        (
            cobble_content::EnvironmentSource::Modrinth,
            environment.modrinth.as_ref(),
        ),
        (
            cobble_content::EnvironmentSource::Curseforge,
            environment.curseforge.as_ref().map(|p| &p.graph),
        ),
    ] {
        for item in graph.into_iter().flat_map(|g| &g.mods) {
            actual.push((
                source,
                item.project_id.as_str(),
                item.version_id.as_str(),
                item.file.filename.as_str(),
                item.file.sha1.to_ascii_lowercase(),
            ));
        }
    }
    let mut expected: Vec<_> = selected
        .iter()
        .map(|s| {
            (
                s.selection.source,
                s.selection.project_id.as_str(),
                s.selection.version_id.as_str(),
                s.filename.as_str(),
                s.sha1.to_ascii_lowercase(),
            )
        })
        .collect();
    actual.sort();
    expected.sort();
    if actual != expected {
        return Err("READINESS_CONTENT_METADATA_CHANGED".into());
    }
    Ok(())
}

async fn plan(
    app: &AppHandle,
    id: &str,
    control: cobble_core::OperationControl,
) -> Result<(Game, RepairPlan, management::Operation), String> {
    let (game, lease) = management::begin(app, id, true)?;
    let check = inspect(&game, control.clone()).await?;
    if check.files.changed_files > 0 {
        return Err("READINESS_CHANGED_FILES_REQUIRE_REVIEW".into());
    }
    let selected = check.files.selected_content;
    let retained = cobble_core::environment_install::read_environment_plan(&game.directory)
        .map_err(|e| e.to_string())?
        .filter(|environment| validate_exact_content(&selected, environment).is_ok());
    let environment = if selected.is_empty() {
        None
    } else if retained.is_some() {
        retained
    } else {
        if selected.len() > 128 {
            return Err("READINESS_CONTENT_GRAPH_UNAVAILABLE".into());
        }
        let roots: Vec<_> = selected.iter().map(|s| s.selection.clone()).collect();
        let cf = if roots
            .iter()
            .any(|r| r.source == cobble_content::EnvironmentSource::Curseforge)
        {
            Some(crate::curseforge::client()?)
        } else {
            None
        };
        let resolving = cobble_content::resolve_environment_selected(
            &roots,
            &game.version,
            game.loader_kind
                .as_deref()
                .ok_or("READINESS_LOADER_UNAVAILABLE")?,
            cf.as_ref(),
        );
        let environment = tokio::select! {value=resolving=>value.map_err(|e|e.to_string())?,_ = control.cancelled()=>return Err("READINESS_CANCELLED".into())};
        validate_exact_content(&selected, &environment)?;
        Some(environment)
    };
    let source = game
        .download_source
        .clone()
        .map(Ok)
        .unwrap_or_else(cobble_minecraft::read_download_settings)
        .map_err(|e| e.to_string())?;
    let captured = cobble_core::capture_java_choice(&game.runtime_overrides.java)
        .map_err(|e| e.to_string())?;
    let plan = RepairPlan {
        version: game.version.clone(),
        loader_kind: game.loader_kind.clone(),
        loader_version: game.loader_version.clone(),
        java: captured,
        source,
        content_revision: check.files.content_revision,
        environment,
    };
    Ok((game, plan, lease))
}

#[derive(Serialize)]
pub struct ReadinessPreparation {
    pub space: cobble_core::space::SpaceEstimate,
    pub requirements: native_reverse_install::Requirements,
}
#[tauri::command]
pub async fn preview_game_readiness(
    app: AppHandle,
    id: String,
    request_id: Option<String>,
) -> Result<ReadinessPreparation, String> {
    let (control, _check) = checks::begin(request_id)?;
    let (game, plan, _lease) = plan(&app, &id, control.clone()).await?;
    let space = native_readiness_space::estimate(
        &plan.version,
        plan.loader_kind.as_deref(),
        plan.environment.as_ref(),
        &game.directory,
        plan.source,
        &plan.java,
        control,
    )
    .await?;
    let requirements = if let Some(environment) = plan.environment {
        native_reverse_install::requirements(environment).await?
    } else {
        native_reverse_install::Requirements {
            missing: vec![],
            ready: true,
        }
    };
    Ok(ReadinessPreparation {
        space,
        requirements,
    })
}

#[tauri::command]
pub async fn prepare_game_readiness(
    app: AppHandle,
    id: String,
) -> Result<tasks::TaskSnapshot, String> {
    let (_game, plan, lease) = plan(
        &app,
        &id,
        cobble_core::OperationControl::new(Arc::new(|_| {})),
    )
    .await?;
    drop(lease);
    let task_id = tasks::register(
        &app,
        tasks::TaskRequest::GameReadiness { game_id: id, plan },
        "准备离线文件",
    )?;
    let snapshot = tasks::list_tasks(app.clone())?
        .into_iter()
        .find(|t| t.id == task_id)
        .ok_or("TASK_NOT_FOUND")?;
    tauri::async_runtime::spawn(async move {
        let _ = tasks::execute(app, task_id).await;
    });
    Ok(snapshot)
}

pub(super) async fn perform(
    app: &AppHandle,
    id: &str,
    plan: &RepairPlan,
    control: cobble_core::OperationControl,
) -> Result<(), String> {
    let (game, _lease) = management::begin(app, id, true)?;
    if game.version != plan.version
        || game.loader_kind != plan.loader_kind
        || game.loader_version != plan.loader_version
    {
        return Err("READINESS_GAME_CHANGED".into());
    }
    if cobble_core::capture_java_choice(&game.runtime_overrides.java).map_err(|e| e.to_string())?
        != plan.java
    {
        return Err("READINESS_JAVA_CHANGED".into());
    }
    let selected =
        cobble_core::readiness::selected_content(&game.directory).map_err(|e| e.to_string())?;
    if cobble_core::readiness::content_revision(&selected).map_err(|e| e.to_string())?
        != plan.content_revision
    {
        return Err("READINESS_CONTENT_CHANGED".into());
    }
    let before = inspect(&game, control.clone()).await?;
    if before.files.changed_files > 0 {
        return Err("READINESS_CHANGED_FILES_REQUIRE_REVIEW".into());
    }
    let space = native_readiness_space::estimate(
        &plan.version,
        plan.loader_kind.as_deref(),
        plan.environment.as_ref(),
        &game.directory,
        plan.source.clone(),
        &plan.java,
        control.clone(),
    )
    .await?;
    if !space.can_proceed {
        return Err("SPACE_INSUFFICIENT".into());
    }
    let source = plan.source.download_source().map_err(|e| e.to_string())?;
    let outcome = match plan.loader_version.as_deref() {
        Some(loader) => match plan.loader_kind.as_deref() {
            Some("fabric") => {
                cobble_core::install_fabric_controlled(
                    &plan.version,
                    loader,
                    &game.directory,
                    source,
                    control.clone(),
                )
                .await
            }
            Some("forge" | "neoforge") => {
                cobble_core::install_forge_controlled(
                    &plan.version,
                    loader,
                    if plan.loader_kind.as_deref() == Some("neoforge") {
                        cobble_core::ForgeKind::NeoForge
                    } else {
                        cobble_core::ForgeKind::Forge
                    },
                    &game.directory,
                    source,
                    control.clone(),
                )
                .await
            }
            _ => return Err("READINESS_LOADER_UNAVAILABLE".into()),
        },
        None => {
            cobble_core::install_game_controlled(
                &plan.version,
                &game.directory,
                source,
                control.clone(),
            )
            .await
        }
    }
    .map_err(|e| e.to_string())?;
    cobble_core::ensure_launch_java_controlled(outcome.java_major, &plan.java, control.clone())
        .await
        .map_err(|e| e.to_string())?;
    if let Some(environment) = &plan.environment {
        validate_exact_content(&selected, environment)?;
        // Exact selected versions and digests are preserved. The initial creation
        // receipt may describe an older deliberate update, so use provider repair
        // rather than replacing that receipt with a different initial plan.
        if let Some(mr) = &environment.modrinth {
            cobble_core::install_modrinth_plan_controlled(mr, &game.directory, control.clone())
                .await
                .map_err(|e| e.to_string())?;
        }
        if let Some(cf) = &environment.curseforge {
            cobble_core::install_curseforge_plan_controlled(
                cf,
                &game.directory,
                crate::curseforge::credential().ok_or("ENVIRONMENT_CURSEFORGE_UNAVAILABLE")?,
                control.clone(),
            )
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    let after = inspect(&game, control).await?;
    if !after.files.ready || after.java.state != "ready" {
        return Err("READINESS_FILES_STILL_INCOMPLETE".into());
    }
    Ok(())
}
