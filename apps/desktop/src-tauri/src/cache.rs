// SPDX-License-Identifier: MIT
#[tauri::command]
pub fn get_download_tuning() -> cobble_net::DownloadTuning {
    cobble_net::download_tuning()
}
#[tauri::command]
pub fn set_download_tuning(tuning: cobble_net::DownloadTuning) -> Result<(), String> {
    cobble_net::save_download_tuning(tuning).map_err(|error| error.to_string())
}
#[tauri::command]
pub async fn get_public_cache_stats() -> Result<cobble_net::MetadataCacheStats, String> {
    let cache = cobble_net::PublicMetadataCache::shared().ok_or("无法找到缓存目录")?;
    Ok(cache.stats().await)
}
#[tauri::command]
pub async fn clear_public_cache() -> Result<cobble_net::MetadataCacheStats, String> {
    let cache = cobble_net::PublicMetadataCache::shared().ok_or("无法找到缓存目录")?;
    cache.clear().await;
    Ok(cache.stats().await)
}
