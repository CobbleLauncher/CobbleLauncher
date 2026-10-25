// SPDX-License-Identifier: MIT
//! Read-only projection of retained receipts. No maintenance database or new history writes.
use super::*;
use serde_json::{Value, json};

#[derive(Serialize)]
pub struct MaintenanceOverview {
    items: Vec<Value>,
    unavailable: Vec<&'static str>,
    limited: bool,
}

pub(super) async fn list(app: AppHandle, id: String) -> Result<MaintenanceOverview, String> {
    let directory = game_directory(&app, &id)?;
    let selected = id.clone();
    let mut overview = tauri::async_runtime::spawn_blocking(move || {
        let mut result = MaintenanceOverview { items: Vec::new(), unavailable: Vec::new(), limited: false };
        match cobble_core::list_mod_operation_history(&directory) {
            Ok(operations) => {
                result.limited |= operations.len() > 64;
                result.items.extend(operations.into_iter().take(64).map(|operation| json!({
                    "kind":"mod", "id":operation.operation_id, "createdAt":operation.created_at,
                    "title":operation.files.iter().take(3).map(|file| file.name.as_str()).collect::<Vec<_>>().join(", "), "status":operation.status
                })));
            }
            Err(_) => result.unavailable.push("mods"),
        }
        match game_config::recovery_metadata(&directory).and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string())) {
            Ok(configs) => {
                result.limited |= configs["truncated"].as_bool().unwrap_or_default();
                if let Some(files) = configs["files"].as_array() {
                    for file in files {
                        if let Some(backups) = file["backups"].as_array() {
                            result.limited |= backups.len() > 1;
                            if let Some(backup) = backups.first() {
                                result.items.push(json!({"kind":"config", "id":format!("{}:{}",file["id"].as_str().unwrap_or_default(),backup["id"].as_str().unwrap_or_default()),"createdAt":backup["createdAt"],"title":file["name"],"status":"retained"}));
                            }
                        }
                    }
                }
            }
            Err(_) => result.unavailable.push("configs"),
        }
        match crate::plugins::plugin_recovery_workspace::last_clean_timing(&selected) {
            Ok(Some((started, ended))) => result.items.push(json!({"kind":"clean_exit","id":format!("clean-{ended}"),"createdAt":ended,"startedAt":started,"title":"","status":"exit_zero"})),
            Ok(None) => {}
            Err(_) => result.unavailable.push("launch"),
        }
        result
    }).await.map_err(|_| "MAINTENANCE_UNAVAILABLE")?;
    match worlds::list_game_worlds(app.clone(), id.clone()).await {
        Ok(worlds) => {
            overview.limited |= worlds.len() > 8;
            for world in worlds.into_iter().take(8) {
                match world_snapshots::list_world_snapshots(
                    app.clone(),
                    id.clone(),
                    world.folder_name.clone(),
                )
                .await
                {
                    Ok(snapshots) => {
                        overview.limited |= snapshots.len() > 8;
                        overview.items.extend(snapshots.into_iter().take(8).map(|snapshot| json!({
                            "kind":"world", "id":snapshot.id,"createdAt":snapshot.created_at,
                            "title":if snapshot.name.is_empty() {world.name.clone()} else {format!("{} · {}",world.name,snapshot.name)},"status":if snapshot.automatic {"automatic"} else {"retained"}
                        })));
                    }
                    Err(_) => {
                        if !overview.unavailable.contains(&"worlds") {
                            overview.unavailable.push("worlds");
                        }
                    }
                }
            }
        }
        Err(_) => overview.unavailable.push("worlds"),
    }
    Ok(overview)
}
