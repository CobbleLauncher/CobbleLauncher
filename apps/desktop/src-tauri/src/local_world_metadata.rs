// SPDX-License-Identifier: MIT
//! Registered local worlds only; the file is read while running without claiming live state.
use tauri::AppHandle;
pub(super) fn validate_world_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || value.chars().any(char::is_control)
        || matches!(value, "." | "..")
        || value.contains('/')
        || value.contains('\\')
    {
        return Err("PLUGIN_WORLD_SCOPE".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn read_local_world_metadata(
    app: AppHandle,
    game_id: String,
    world_id: String,
) -> Result<cobble_content::LocalWorldMetadata, String> {
    validate_world_id(&world_id)?;
    crate::games::ensure_registered_game(&app, &game_id)?;
    let directory = crate::games::game_directory(&app, &game_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        cobble_content::local_world_metadata(&directory, &world_id)
    })
    .await
    .map_err(|_| "WORLD_METADATA_UNAVAILABLE".into())
}
