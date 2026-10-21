// SPDX-License-Identifier: MIT
use cobble_auth::OfflineProfile;

#[tauri::command]
pub fn get_offline_profile() -> Result<Option<OfflineProfile>, String> {
    cobble_auth::read_offline_profile().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn save_offline_profile(name: String) -> Result<OfflineProfile, String> {
    cobble_auth::save_offline_profile(name.trim()).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn remove_offline_profile() -> Result<(), String> {
    cobble_auth::remove_offline_profile().map_err(|e| e.to_string())
}
