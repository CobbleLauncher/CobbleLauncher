// SPDX-License-Identifier: MIT
use super::*;
#[path = "recent_maintenance.rs"]
mod maintenance;
pub use maintenance::MaintenanceOverview;

#[tauri::command]
pub async fn list_game_maintenance(
    app: AppHandle,
    id: String,
) -> Result<MaintenanceOverview, String> {
    maintenance::list(app, id).await
}

#[tauri::command]
pub async fn get_game_mod_update_policy(
    app: AppHandle,
    id: String,
) -> Result<cobble_core::ModUpdatePolicy, String> {
    let directory = game_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        cobble_core::read_mod_update_policy(&directory).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn set_game_mod_update_policy(
    app: AppHandle,
    id: String,
    policy: cobble_core::ModUpdatePolicy,
) -> Result<(), String> {
    let (game, operation) = management::begin(&app, &id, true)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_core::set_mod_update_policy(&game.directory, &policy)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?;
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|error| error.to_string())?;
    library.update_plans.remove(&id);
    library.curseforge_plans.remove(&id);
    library.curseforge_preview_ids.remove(&id);
    result
}

#[tauri::command]
pub async fn preview_game_mod_operation_undo(
    app: AppHandle,
    id: String,
    operation_id: String,
) -> Result<Vec<String>, String> {
    let directory = game_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        match cobble_core::preview_mod_operation_undo(&directory, &operation_id) {
            Ok(()) => Vec::new(),
            Err(error) => vec![error.to_string()],
        }
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_game_mod_history(
    app: AppHandle,
    id: String,
) -> Result<Vec<cobble_core::ModOperationHistory>, String> {
    let directory = game_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        cobble_core::list_mod_operation_history(&directory).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn undo_game_mod_operation(
    app: AppHandle,
    id: String,
    operation_id: String,
) -> Result<(), String> {
    let (game, operation) = management::begin(&app, &id, true)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_core::undo_mod_operation(&game.directory, &operation_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?;
    // Invalidate plans even after failure: a partial rollback must never reuse a preview.
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|error| error.to_string())?;
    library.update_plans.remove(&id);
    library.mod_plans.remove(&id);
    result
}

#[tauri::command]
pub async fn list_game_mod_versions(
    app: AppHandle,
    id: String,
    project_id: String,
) -> Result<Vec<cobble_content::ModVersion>, String> {
    let (version, loader) = modrinth_target(&app, &id)?;
    cobble_content::list_modrinth_versions(&project_id, &version, &loader)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_game_mod_version_details(
    app: AppHandle,
    id: String,
    project_id: String,
    version_id: String,
) -> Result<cobble_content::ModVersionDetails, String> {
    let (version, loader) = modrinth_target(&app, &id)?;
    cobble_content::modrinth_version_details(&project_id, &version_id, &version, &loader)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_managed_game_mods(
    app: AppHandle,
    id: String,
) -> Result<Vec<ManagedGameMod>, String> {
    modrinth_target(&app, &id)?;
    let directory = game_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let managed =
            cobble_core::list_managed_modrinth_mods(&directory).map_err(|e| e.to_string())?;
        let local = cobble_content::list_mods(&directory).map_err(|e| e.to_string())?;
        Ok(managed
            .into_iter()
            .map(|item| {
                let name = local
                    .iter()
                    .find(|entry| {
                        entry
                            .file_name
                            .strip_suffix(".disabled")
                            .unwrap_or(&entry.file_name)
                            == item.filename
                    })
                    .map(|entry| entry.name.clone())
                    .unwrap_or_else(|| item.filename.clone());
                ManagedGameMod { item, name }
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct ManagedGameMod {
    #[serde(flatten)]
    item: cobble_core::ManagedModSummary,
    name: String,
}

#[tauri::command]
pub async fn set_game_mod_pinned(
    app: AppHandle,
    id: String,
    project_id: String,
    pinned: bool,
    source: Option<String>,
) -> Result<(), String> {
    let (game, operation) = management::begin(&app, &id, true)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        match source.as_deref().unwrap_or("modrinth") {
            "modrinth" => {
                cobble_core::set_modrinth_mod_pinned(&game.directory, &project_id, pinned)
            }
            "curseforge" => {
                cobble_core::set_curseforge_mod_pinned(&game.directory, &project_id, pinned)
            }
            _ => return Err("Unknown Mod provider".into()),
        }
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?;
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|error| error.to_string())?;
    library.update_plans.remove(&id);
    library.curseforge_plans.remove(&id);
    library.curseforge_preview_ids.remove(&id);
    result
}

#[tauri::command]
pub async fn preview_game_mod_version_changes(
    app: AppHandle,
    id: String,
    selections: Vec<cobble_content::ModVersionSelection>,
) -> Result<UpdatePreview, String> {
    let (version, loader) = modrinth_target(&app, &id)?;
    let directory = game_directory(&app, &id)?;
    let plan =
        cobble_core::prepare_modrinth_version_changes(&directory, &version, &loader, &selections)
            .await
            .map_err(|e| e.to_string())?;
    let plan_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos()
        .to_string();
    let preview = UpdatePreview {
        plan_id: plan_id.clone(),
        changes: plan.changes.clone(),
    };
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|e| e.to_string())?;
    if !library.games.iter().any(|game| game.id == id) {
        return Err("找不到这个游戏".into());
    }
    library.update_plans.insert(id, (plan_id, plan));
    Ok(preview)
}

#[tauri::command]
pub async fn recognize_game_mods(
    app: AppHandle,
    id: String,
) -> Result<cobble_core::ModRecognitionOutcome, String> {
    let (version, loader) = modrinth_target(&app, &id)?;
    let (game, _operation) = management::begin(&app, &id, true)?;
    update_game(&app, &id, |_| {}, false);
    let outcome = cobble_core::recognize_modrinth_mods(&game.directory, &version, &loader)
        .await
        .map_err(|e| e.to_string())?;
    if outcome.recognized > 0 {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|e| e.to_string())?;
        library.update_plans.remove(&id);
        library.mod_plans.remove(&id);
    }
    Ok(outcome)
}

#[derive(Serialize)]
pub struct UpdateCandidate {
    #[serde(flatten)]
    item: cobble_core::ModUpdateCandidate,
    pack_owned: bool,
}

#[derive(Serialize)]
pub struct UpdateCheck {
    updates: Vec<UpdateCandidate>,
    managed_count: usize,
}

#[derive(Serialize)]
pub struct UpdatePreview {
    plan_id: String,
    changes: Vec<cobble_core::ModUpdateChange>,
}

#[tauri::command]
pub async fn check_game_mod_updates(app: AppHandle, id: String) -> Result<UpdateCheck, String> {
    let (version, loader) = modrinth_target(&app, &id)?;
    let directory = game_directory(&app, &id)?;
    let local_app = app.clone();
    let local_id = id.clone();
    let local_directory = directory.clone();
    let (managed_count, installed) = tauri::async_runtime::spawn_blocking(move || {
        let count = cobble_core::managed_mod_count(&local_directory).map_err(|e| e.to_string())?;
        let installed = with_game_mods(&local_app, &local_id, false, cobble_content::list_mods)?;
        Ok::<_, String>((count, installed))
    })
    .await
    .map_err(|e| e.to_string())??;
    let updates = cobble_core::check_modrinth_updates(&directory, &version, &loader)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|mut item| {
            if let Some(local) = installed.iter().find(|local| {
                local
                    .item
                    .file_name
                    .strip_suffix(".disabled")
                    .unwrap_or(&local.item.file_name)
                    == item.filename
            }) {
                item.name = local.item.name.clone();
            }
            let pack_owned = installed.iter().any(|local| {
                local.pack_owned
                    && local
                        .item
                        .file_name
                        .strip_suffix(".disabled")
                        .unwrap_or(&local.item.file_name)
                        == item.filename
            });
            UpdateCandidate { item, pack_owned }
        })
        .collect();
    Ok(UpdateCheck {
        updates,
        managed_count,
    })
}

#[tauri::command]
pub async fn preview_game_mod_updates(
    app: AppHandle,
    id: String,
    project_ids: Vec<String>,
) -> Result<UpdatePreview, String> {
    let (version, loader) = modrinth_target(&app, &id)?;
    let directory = game_directory(&app, &id)?;
    let plan = cobble_core::prepare_modrinth_updates(&directory, &version, &loader, &project_ids)
        .await
        .map_err(|e| e.to_string())?;
    let plan_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos()
        .to_string();
    let preview = UpdatePreview {
        plan_id: plan_id.clone(),
        changes: plan.changes.clone(),
    };
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|e| e.to_string())?;
    if !library.games.iter().any(|game| game.id == id) {
        return Err("找不到这个游戏".into());
    }
    library.update_plans.insert(id, (plan_id, plan));
    Ok(preview)
}

#[tauri::command]
pub async fn apply_game_mod_updates(
    app: AppHandle,
    id: String,
    plan_id: String,
) -> Result<cobble_core::ModUpdateOutcome, String> {
    let plan = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| game.id == id)
            .ok_or("找不到这个游戏")?;
        if game.loader_version.is_none() {
            return Err("请先完成模组加载器安装".into());
        }
        let stored = library
            .update_plans
            .get(&id)
            .ok_or("请先预览要更新的 Mod")?;
        if stored.0 != plan_id {
            return Err("更新方案已变更，请重新预览".into());
        }
        stored.1.clone()
    };
    let result = match tasks::run(
        app.clone(),
        tasks::TaskRequest::ModUpdate {
            game_id: id.clone(),
            plan,
        },
        "更新 Mod",
    )
    .await?
    {
        tasks::TaskResult::ModUpdate(outcome) => outcome,
        _ => return Err("任务类型不匹配".into()),
    };
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|e| e.to_string())?;
    if library
        .update_plans
        .get(&id)
        .is_some_and(|stored| stored.0 == plan_id)
    {
        library.update_plans.remove(&id);
    }
    Ok(result)
}
