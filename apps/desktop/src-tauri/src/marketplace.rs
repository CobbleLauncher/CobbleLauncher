// SPDX-License-Identifier: MIT
use super::*;

#[tauri::command]
pub async fn search_modrinth_packs(
    query: String,
    minecraft_version: Option<String>,
    loader: Option<String>,
    offset: u32,
) -> Result<cobble_content::SearchResults, String> {
    cobble_content::search_modrinth_packs(
        &query,
        minecraft_version.as_deref(),
        loader.as_deref(),
        offset,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_modrinth_pack_versions(
    project_id: String,
    minecraft_version: Option<String>,
    loader: Option<String>,
) -> Result<Vec<cobble_content::PackVersion>, String> {
    cobble_content::list_modrinth_pack_versions(
        &project_id,
        minecraft_version.as_deref(),
        loader.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn preview_modrinth_pack(
    app: AppHandle,
    project_id: String,
    version_id: String,
) -> Result<PackPreview, String> {
    let id = next_pack_id()?;
    let archive = pack_archive_path(&id)?;
    let directory = archive.parent().ok_or("无法定位整合包缓存")?.to_owned();
    tauri::async_runtime::spawn_blocking(move || fs::create_dir_all(directory))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let pack = cobble_core::download_modrinth_pack(&project_id, &version_id, &archive)
        .await
        .map_err(|e| e.to_string())?;
    store_pack_preview(&app, id, pack)
}
