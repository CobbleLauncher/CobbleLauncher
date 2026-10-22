// SPDX-License-Identifier: MIT
use cobble_auth::{OfflineProfile, ProfileLibrary};

#[tauri::command]
pub fn list_player_profiles() -> Result<ProfileLibrary, String> {
    cobble_auth::list_player_profiles().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn add_offline_profile(name: String, note: Option<String>) -> Result<ProfileLibrary, String> {
    cobble_auth::add_offline_profile(name.trim(), note.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn select_player_profile(id: String) -> Result<ProfileLibrary, String> {
    cobble_auth::select_player_profile(&id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_profile_note(id: String, note: Option<String>) -> Result<ProfileLibrary, String> {
    cobble_auth::update_profile_note(&id, note.as_deref()).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn remove_player_profile(id: String) -> Result<ProfileLibrary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cobble_auth::remove_player_profile(&id).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

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
