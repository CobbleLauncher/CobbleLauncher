// SPDX-License-Identifier: MIT
use super::*;

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
        .map(|item| {
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
    let (game, _operation) = management::begin(&app, &id, true)?;
    if game.loader_version.is_none() {
        return Err("请先完成模组加载器安装".into());
    }
    let plan = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|e| e.to_string())?;
        let stored = library
            .update_plans
            .get(&id)
            .ok_or("请先预览要更新的 Mod")?;
        if stored.0 != plan_id {
            return Err("更新方案已变更，请重新预览".into());
        }
        stored.1.clone()
    };
    let installed = with_game_mods(&app, &id, false, cobble_content::list_mods)?;
    let pack_files: Vec<_> = plan
        .changes
        .iter()
        .filter_map(|change| {
            let previous = plan.previous_filename(&change.project_id)?;
            installed
                .iter()
                .any(|local| {
                    local.pack_owned
                        && local
                            .item
                            .file_name
                            .strip_suffix(".disabled")
                            .unwrap_or(&local.item.file_name)
                            == previous
                })
                .then(|| format!("mods/{}", change.filename))
        })
        .collect();
    update_game(&app, &id, |_| {}, false);
    let result = cobble_core::apply_modrinth_updates(&game.directory, &plan, Arc::new(|_| {}))
        .await
        .map_err(|e| e.to_string());
    if result.is_ok() {
        if !pack_files.is_empty() {
            update_game(
                &app,
                &id,
                |game| {
                    for file in pack_files {
                        if !game.pack_mod_files.contains(&file) {
                            game.pack_mod_files.push(file);
                        }
                    }
                },
                true,
            );
        }
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|e| e.to_string())?;
        if library
            .update_plans
            .get(&id)
            .is_some_and(|stored| stored.0 == plan_id)
        {
            library.update_plans.remove(&id);
        }
    }
    result
}
