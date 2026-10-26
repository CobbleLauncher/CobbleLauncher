// SPDX-License-Identifier: MIT
//! Explicit local change sharing. Picker paths and prepared plans stay native.
use cobble_core::change_share::{self, Document};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

#[derive(Clone)]
struct Imported {
    document: Document,
    at: Instant,
}
#[derive(Clone)]
struct Prepared {
    game_id: String,
    document: Document,
    mod_plan: Option<cobble_core::ModUpdatePlan>,
    configs: Vec<(String, Option<String>)>,
    at: Instant,
}
#[derive(Default)]
struct Store {
    imports: HashMap<String, Imported>,
    plans: HashMap<String, Prepared>,
    created_configs: HashMap<String, (String, String, String)>,
}
fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}
fn nonce() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "CHANGE_SHARE_UNAVAILABLE")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn expire(state: &mut Store) {
    state
        .imports
        .retain(|_, value| value.at.elapsed() < Duration::from_secs(1800));
    state
        .plans
        .retain(|_, value| value.at.elapsed() < Duration::from_secs(1800));
}
fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
async fn inspect(
    game: &crate::games::workspace::GameSnapshot,
) -> Result<cobble_core::play_manifest::Preview, String> {
    tokio::time::timeout(
        Duration::from_secs(60),
        cobble_core::play_manifest::preview(
            &game.directory,
            &game.name,
            &game.version,
            game.loader_kind.as_deref(),
            game.loader_version.as_deref(),
        ),
    )
    .await
    .map_err(|_| "CHANGE_SHARE_PREVIEW_TIMEOUT")?
    .map_err(err)
}

#[tauri::command]
pub async fn preview_game_change_export(app: AppHandle, id: String) -> Result<Value, String> {
    let (game, _lease) = crate::games::workspace::acquire(&app, &id)?;
    let preview = inspect(&game).await?;
    Ok(json!({
        "revision":preview.revision,
        "mods":preview.mods,
        "configs":preview.configs.iter().map(|item| json!({"id":item.id,"name":item.name,"size":item.size,"text":item.text})).collect::<Vec<_>>(),
        "excludedMods":preview.excluded_mods,
    }))
}

#[tauri::command]
pub async fn export_game_change(
    app: AppHandle,
    id: String,
    revision: String,
    mod_ids: Vec<String>,
    config_ids: Vec<String>,
) -> Result<Value, String> {
    let (game, _lease) = crate::games::workspace::acquire(&app, &id)?;
    let preview = inspect(&game).await?;
    if preview.revision != revision {
        return Err("CHANGE_SHARE_STALE".into());
    }
    let document = change_share::from_preview(&preview, &mod_ids, &config_ids).map_err(err)?;
    let window = app
        .get_webview_window("main")
        .ok_or("CHANGE_SHARE_UNAVAILABLE")?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("分享一次改动")
        .set_file_name("cobble-changes.json")
        .add_filter("Cobble changes", &["json"])
        .save_file()
        .await
    else {
        return Ok(json!({"saved":false}));
    };
    let after = inspect(&game).await?;
    if after.revision != revision {
        return Err("CHANGE_SHARE_STALE".into());
    }
    let bytes = change_share::encode(&document).map_err(err)?;
    let destination = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let parent = destination.parent().ok_or("CHANGE_SHARE_SAVE_FAILED")?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|_| "CHANGE_SHARE_SAVE_FAILED")?;
        temporary
            .write_all(&bytes)
            .map_err(|_| "CHANGE_SHARE_SAVE_FAILED")?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| "CHANGE_SHARE_SAVE_FAILED")?;
        temporary
            .persist(destination)
            .map_err(|_| "CHANGE_SHARE_SAVE_FAILED")?;
        Ok(())
    })
    .await
    .map_err(|_| "CHANGE_SHARE_SAVE_FAILED")??;
    Ok(json!({"saved":true,"mods":document.mods.len(),"configs":document.configs.len()}))
}

#[tauri::command]
pub async fn import_game_change(app: AppHandle) -> Result<Value, String> {
    let window = app
        .get_webview_window("main")
        .ok_or("CHANGE_SHARE_UNAVAILABLE")?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择改动清单")
        .add_filter("Cobble changes", &["json"])
        .pick_file()
        .await
    else {
        return Ok(Value::Null);
    };
    let path = file.path().to_owned();
    let document = tauri::async_runtime::spawn_blocking(move || -> Result<Document, String> {
        let metadata = fs::symlink_metadata(&path).map_err(|_| "CHANGE_SHARE_DOCUMENT_INVALID")?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > change_share::MAX_DOCUMENT_BYTES
        {
            return Err("CHANGE_SHARE_DOCUMENT_INVALID".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("CHANGE_SHARE_DOCUMENT_INVALID".into());
            }
        }
        change_share::read(fs::File::open(path).map_err(|_| "CHANGE_SHARE_DOCUMENT_INVALID")?)
            .map_err(err)
    })
    .await
    .map_err(|_| "CHANGE_SHARE_DOCUMENT_INVALID")??;
    let id = nonce()?;
    let mut state = store().lock().map_err(|_| "CHANGE_SHARE_UNAVAILABLE")?;
    expire(&mut state);
    if state.imports.len() >= 8 {
        return Err("CHANGE_SHARE_BUSY".into());
    }
    state.imports.insert(
        id.clone(),
        Imported {
            document: document.clone(),
            at: Instant::now(),
        },
    );
    Ok(json!({"importId":id,"document":document}))
}

#[tauri::command]
pub async fn preview_game_change_apply(
    app: AppHandle,
    id: String,
    import_id: String,
) -> Result<Value, String> {
    let imported = {
        let mut state = store().lock().map_err(|_| "CHANGE_SHARE_UNAVAILABLE")?;
        expire(&mut state);
        state
            .imports
            .get(&import_id)
            .cloned()
            .ok_or("CHANGE_SHARE_IMPORT_EXPIRED")?
    };
    let (game, _lease) = crate::games::workspace::acquire(&app, &id)?;
    let document = imported.document;
    if game.version != document.game_version
        || game.loader_kind.as_deref() != Some(&document.loader)
        || game.loader_version.as_deref() != Some(&document.loader_version)
    {
        return Err("CHANGE_SHARE_TARGET_MISMATCH".into());
    }
    if document
        .mods
        .iter()
        .any(|item| item.source != cobble_content::EnvironmentSource::Modrinth)
    {
        return Err("CHANGE_SHARE_CURSEFORGE_APPLY_UNSUPPORTED".into());
    }
    let mod_plan = if document.mods.is_empty() {
        None
    } else {
        let roots = document
            .mods
            .iter()
            .map(|item| cobble_content::ModVersionSelection {
                project_id: item.project_id.clone(),
                version_id: item.version_id.clone(),
            })
            .collect::<Vec<_>>();
        let graph = tokio::time::timeout(
            Duration::from_secs(60),
            cobble_content::resolve_modrinth_selected(
                &roots,
                &document.game_version,
                &document.loader,
            ),
        )
        .await
        .map_err(|_| "CHANGE_SHARE_RESOLUTION_TIMEOUT")?
        .map_err(|_| "CHANGE_SHARE_RESOLUTION_FAILED")?;
        change_share::verify_modrinth_plan(&document, &graph).map_err(err)?;
        Some(
            cobble_core::prepare_shared_modrinth_changes(&game.directory, graph)
                .await
                .map_err(err)?,
        )
    };
    let mut configs = Vec::new();
    let mut config_revisions = Vec::new();
    for item in &document.configs {
        let current =
            crate::games::game_config::inspect_shared_config(&game.directory, &item.path)?;
        let (current_text, current_revision) = match current {
            Some((_, text, revision)) => (Some(text), Some(revision)),
            None => (None, None),
        };
        configs.push(
            json!({"path":item.path,"currentText":current_text,"incomingText":item.text,
            "currentRevision":current_revision,"choice":"keep"}),
        );
        config_revisions.push((item.path.clone(), current_revision));
    }
    let changes = mod_plan
        .as_ref()
        .map(|plan| plan.changes.clone())
        .unwrap_or_default();
    let selected: HashSet<_> = changes
        .iter()
        .map(|change| change.project_id.as_str())
        .collect();
    let skipped = document.mods.iter().filter(|item| !selected.contains(item.project_id.as_str()))
        .map(|item| json!({"projectId":item.project_id,"name":item.name,"reason":"alreadyInstalled"}))
        .collect::<Vec<_>>();
    let installed = cobble_core::list_managed_modrinth_mods(&game.directory).map_err(err)?;
    let shared_ids: HashSet<_> = document
        .mods
        .iter()
        .map(|item| item.project_id.as_str())
        .collect();
    let local_count = cobble_content::list_mods(&game.directory)
        .map_err(err)?
        .len();
    let shared_present = installed
        .iter()
        .filter(|item| shared_ids.contains(item.project_id.as_str()))
        .count();
    let additional_local_mods = local_count.saturating_sub(shared_present);
    let plan_id = nonce()?;
    let mut state = store().lock().map_err(|_| "CHANGE_SHARE_UNAVAILABLE")?;
    expire(&mut state);
    if state.plans.len() >= 8 {
        return Err("CHANGE_SHARE_BUSY".into());
    }
    state.plans.insert(
        plan_id.clone(),
        Prepared {
            game_id: id,
            document,
            mod_plan,
            configs: config_revisions,
            at: Instant::now(),
        },
    );
    let changes = changes
        .iter()
        .map(|item| {
            json!({
                "projectId":item.project_id,"name":item.name,"fromVersion":item.from_version,
                "toVersion":item.to_version,"filename":item.filename,"enabled":item.enabled,
            })
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"planId":plan_id,"changes":changes,"skipped":skipped,"configs":configs,
        "additionalLocalMods":additional_local_mods}),
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigChoice {
    path: String,
    choice: String,
}

#[tauri::command]
pub async fn apply_game_change(
    app: AppHandle,
    id: String,
    plan_id: String,
    config_choices: Vec<ConfigChoice>,
) -> Result<Value, String> {
    let prepared = {
        let mut state = store().lock().map_err(|_| "CHANGE_SHARE_UNAVAILABLE")?;
        expire(&mut state);
        state
            .plans
            .get(&plan_id)
            .cloned()
            .ok_or("CHANGE_SHARE_PLAN_EXPIRED")?
    };
    if prepared.game_id != id {
        return Err("CHANGE_SHARE_TARGET_MISMATCH".into());
    }
    let mut choices = HashMap::new();
    for item in config_choices {
        if !matches!(item.choice.as_str(), "keep" | "adopt")
            || choices.insert(item.path, item.choice).is_some()
        {
            return Err("CHANGE_SHARE_CONFIG_CHOICE_INVALID".into());
        }
    }
    if choices.len() != prepared.configs.len()
        || prepared
            .configs
            .iter()
            .any(|(path, _)| !choices.contains_key(path))
    {
        return Err("CHANGE_SHARE_CONFIG_CHOICE_INVALID".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let task_id = crate::games::tasks::register_external(
        &app,
        &id,
        "应用分享改动",
        "change_share",
        cancel.clone(),
    )?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let worker_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = async {
            crate::games::tasks::wait_external_turn(&worker_app, &task_id).await?;
            apply_prepared(
                &worker_app,
                &id,
                &plan_id,
                prepared,
                choices,
                cancel,
                &task_id,
            )
            .await
        }
        .await;
        let task_result = match &result {
            Ok(value) if value["complete"] == true => Ok(Some(id)),
            Ok(value) => Err(value["configError"]
                .as_str()
                .unwrap_or("CHANGE_SHARE_PARTIAL")
                .into()),
            Err(error) => Err(error.clone()),
        };
        crate::games::tasks::finish_external(&worker_app, &task_id, task_result);
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| "CHANGE_SHARE_TASK_INTERRUPTED".to_string())?
}

async fn apply_prepared(
    app: &AppHandle,
    id: &str,
    plan_id: &str,
    prepared: Prepared,
    choices: HashMap<String, String>,
    cancel: Arc<AtomicBool>,
    task_id: &str,
) -> Result<Value, String> {
    if prepared.at.elapsed() >= Duration::from_secs(1800) {
        return Err("CHANGE_SHARE_PLAN_EXPIRED".into());
    }
    if cancel.load(Ordering::Acquire) {
        return Err("CHANGE_SHARE_CANCELLED".into());
    }
    let (game, _lease) = crate::games::workspace::acquire(&app, &id)?;
    if game.version != prepared.document.game_version
        || game.loader_kind.as_deref() != Some(&prepared.document.loader)
        || game.loader_version.as_deref() != Some(&prepared.document.loader_version)
    {
        return Err("CHANGE_SHARE_TARGET_MISMATCH".into());
    }
    for (path, revision) in &prepared.configs {
        if cancel.load(Ordering::Acquire) {
            return Err("CHANGE_SHARE_CANCELLED".into());
        }
        let current = crate::games::game_config::inspect_shared_config(&game.directory, path)?;
        if current.as_ref().map(|(_, _, hash)| hash) != revision.as_ref() {
            return Err("CHANGE_SHARE_CONFIG_CHANGED".into());
        }
    }
    let before: HashSet<_> = cobble_core::list_mod_operation_history(&game.directory)
        .map_err(err)?
        .into_iter()
        .map(|item| item.operation_id)
        .collect();
    let progress_app = app.clone();
    let progress_id = task_id.to_owned();
    let control = cobble_core::OperationControl::new(Arc::new(move |p| {
        crate::games::tasks::external_progress(
            &progress_app,
            &progress_id,
            &p.phase,
            p.completed,
            p.total,
        );
    }));
    let watch = control.clone();
    let flag = cancel.clone();
    let watcher = tauri::async_runtime::spawn(async move {
        while !flag.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        watch.cancel();
    });
    let outcome = async {
        if cancel.load(Ordering::Acquire) {
            control.cancel();
        }
        control.check_cancelled().map_err(err)?;
        if let Some(plan) = &prepared.mod_plan {
            cobble_core::apply_modrinth_updates_controlled(&game.directory, plan, control)
                .await
                .map_err(err)
        } else {
            Ok(cobble_core::ModUpdateOutcome {
                updated: 0,
                added: 0,
            })
        }
    }
    .await;
    watcher.abort();
    let outcome = outcome?;
    let operation_id = cobble_core::list_mod_operation_history(&game.directory)
        .map_err(err)?
        .into_iter()
        .find(|item| !before.contains(&item.operation_id))
        .map(|item| item.operation_id);
    let mut applied = Vec::new();
    let mut config_error = None;
    let config_total = prepared
        .document
        .configs
        .iter()
        .filter(|item| choices[&item.path] == "adopt")
        .count();
    for config in &prepared.document.configs {
        if choices[&config.path] != "adopt" {
            continue;
        }
        if cancel.load(Ordering::Acquire) {
            config_error = Some("CHANGE_SHARE_CANCELLED".into());
            break;
        }
        let Some((_, expected)) = prepared
            .configs
            .iter()
            .find(|(path, _)| path == &config.path)
        else {
            continue;
        };
        let pending_undo = if expected.is_none() {
            let token = nonce()?;
            store()
                .lock()
                .map_err(|_| "CHANGE_SHARE_UNAVAILABLE")?
                .created_configs
                .insert(
                    token.clone(),
                    (id.to_owned(), config.path.clone(), config.sha256.clone()),
                );
            Some(token)
        } else {
            None
        };
        match crate::games::game_config::adopt_shared_config(
            &game.directory,
            &config.path,
            &config.text,
            expected.as_deref(),
        ) {
            Ok((config_id, backup_id, new_revision)) => {
                applied.push(
                    json!({"path":config.path,"configId":config_id,"backupId":backup_id,
                    "newRevision":new_revision,"undoId":pending_undo}),
                );
                crate::games::tasks::external_progress(
                    app,
                    task_id,
                    "shared-config",
                    applied.len(),
                    config_total,
                );
            }
            Err(error) => {
                if let Some(token) = pending_undo {
                    if let Ok(mut state) = store().lock() {
                        state.created_configs.remove(&token);
                    }
                }
                config_error = Some(error);
                break;
            }
        }
    }
    store()
        .lock()
        .map_err(|_| "CHANGE_SHARE_UNAVAILABLE")?
        .plans
        .remove(plan_id);
    Ok(
        json!({"updated":outcome.updated,"added":outcome.added,"configsWritten":applied.len(),
        "configBackups":applied,"operationId":operation_id,"configError":config_error,
        "complete":config_error.is_none(),"taskId":task_id}),
    )
}

#[tauri::command]
pub async fn undo_game_change_new_config(
    app: AppHandle,
    id: String,
    undo_id: String,
) -> Result<(), String> {
    let (game_id, path, revision) = store()
        .lock()
        .map_err(|_| "CHANGE_SHARE_UNAVAILABLE")?
        .created_configs
        .get(&undo_id)
        .cloned()
        .ok_or("CHANGE_SHARE_UNDO_EXPIRED")?;
    if game_id != id {
        return Err("CHANGE_SHARE_TARGET_MISMATCH".into());
    }
    let (game, _lease) = crate::games::workspace::acquire(&app, &id)?;
    crate::games::game_config::remove_shared_created_config(&game.directory, &path, &revision)?;
    store()
        .lock()
        .map_err(|_| "CHANGE_SHARE_UNAVAILABLE")?
        .created_configs
        .remove(&undo_id);
    Ok(())
}
