// SPDX-License-Identifier: MIT
use super::*;

fn installed_directory(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    if game.status != "installed" {
        return Err("请先完成游戏安装".into());
    }
    Ok(game.directory.clone())
}

#[tauri::command]
pub async fn list_game_worlds(
    app: AppHandle,
    id: String,
) -> Result<Vec<cobble_content::LocalWorld>, String> {
    let directory = installed_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        cobble_content::list_local_worlds(&directory).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn open_game_world_directory(
    app: AppHandle,
    id: String,
    folder_name: String,
) -> Result<(), String> {
    let directory = installed_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let folder =
            cobble_content::world_directory(&directory, &folder_name).map_err(|e| e.to_string())?;
        open::that(folder).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn export_game_world(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    folder_name: String,
) -> Result<bool, String> {
    let (game, operation) = management::begin(&app, &id, true)?;
    update_game(&app, &id, |_| {}, false);
    let directory = game.directory.clone();
    let selected = folder_name.clone();
    // Check the selected world before opening the native save dialog.
    tauri::async_runtime::spawn_blocking(move || {
        cobble_content::world_directory(&directory, &selected).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    let filename: String = folder_name
        .chars()
        .map(|c| {
            if r#"<>:"/\|?*"#.contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("导出世界备份")
        .add_filter("ZIP 备份", &["zip"])
        .set_file_name(format!("{filename}-backup.zip"))
        .save_file()
        .await
    else {
        return Ok(false);
    };
    let destination = file.path().to_path_buf();
    let temporary = destination.with_file_name(format!(".cobble-world-{}.part", next_pack_id()?));
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_core::export_world_backup(&game.directory, &folder_name, &temporary)
            .map_err(|e| e.to_string())?;
        // The save dialog confirms replacement of this exact destination.
        let result = fs::rename(&temporary, &destination).map_err(|e| e.to_string());
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(true)
}
