// SPDX-License-Identifier: MIT
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tauri::command]
fn get_app_info() -> cobble_domain::AppInfo {
    cobble_core::app_info()
}

#[tauri::command]
fn get_download_settings() -> Result<cobble_minecraft::DownloadSettings, String> {
    cobble_minecraft::read_download_settings().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_download_settings(settings: cobble_minecraft::DownloadSettings) -> Result<(), String> {
    cobble_minecraft::save_download_settings(&settings).map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_download_settings,
            set_download_settings
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Cobble Launcher");
}
