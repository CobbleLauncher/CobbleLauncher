// SPDX-License-Identifier: MIT
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod games;
mod java;
mod offline;

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
        .manage(games::GameLibrary::load())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_download_settings,
            set_download_settings,
            java::get_java_settings,
            java::pick_java_executable,
            java::use_automatic_java,
            games::list_game_versions,
            games::list_games,
            games::default_game_directory,
            games::pick_game_directory,
            games::create_game,
            games::retry_game,
            games::prepare_game_launch,
            games::start_game,
            offline::get_offline_profile,
            offline::save_offline_profile,
            offline::remove_offline_profile
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Cobble Launcher");
}
