// SPDX-License-Identifier: MIT
use super::*;
#[tauri::command]
pub async fn list_world_datapacks(
    app: AppHandle,
    id: String,
    folder_name: String,
) -> Result<cobble_content::WorldDatapacks, String> {
    ensure_registered_game(&app, &id)?;
    let directory = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|e| e.to_string())?;
        library
            .games
            .iter()
            .find(|g| g.id == id)
            .ok_or("Game not found")?
            .directory
            .clone()
    };
    tauri::async_runtime::spawn_blocking(move || {
        cobble_content::list_world_datapacks(&directory, &folder_name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn import_world_datapacks(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    folder_name: String,
) -> Result<Option<cobble_content::WorldDatapacks>, String> {
    let (game, operation) = management::begin(&app, &id, true)?;
    cobble_content::world_directory(&game.directory, &folder_name).map_err(|e| e.to_string())?;
    let Some(files) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Import datapack ZIPs")
        .add_filter("ZIP", &["zip"])
        .pick_files()
        .await
    else {
        return Ok(None);
    };
    let sources = files
        .into_iter()
        .map(|f| f.path().to_path_buf())
        .collect::<Vec<_>>();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_content::import_world_datapacks(&game.directory, &folder_name, &sources)
            .map(Some)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn move_world_datapack(
    app: AppHandle,
    id: String,
    folder_name: String,
    file_name: String,
    recover: bool,
) -> Result<cobble_content::WorldDatapacks, String> {
    let (game, operation) = management::begin(&app, &id, true)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_content::move_world_datapack(&game.directory, &folder_name, &file_name, recover)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
