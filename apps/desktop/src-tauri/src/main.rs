// SPDX-License-Identifier: MIT
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod games;
mod java;
mod offline;
mod preferences;

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
            preferences::get_launcher_preferences,
            preferences::set_default_memory,
            preferences::pick_default_game_directory,
            preferences::reset_default_game_directory,
            games::set_game_memory,
            games::set_game_java_mode,
            games::pick_game_java_executable,
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
            games::set_game_profile,
            games::list_fabric_versions,
            games::list_loader_versions,
            games::rename_game,
            games::management::copy_game,
            games::management::export_game,
            games::management::remove_game,
            games::open_game_directory,
            games::open_game_log,
            games::preview_mrpack,
            games::create_mrpack_game,
            games::search_game_mods,
            games::preview_game_mod_install,
            games::install_game_mod_plan,
            games::mod_updates::check_game_mod_updates,
            games::mod_updates::recognize_game_mods,
            games::local_packs::list_game_packs,
            games::local_packs::import_game_packs,
            games::local_packs::remove_game_pack,
            games::local_packs::open_game_pack_directory,
            games::mod_updates::preview_game_mod_updates,
            games::mod_updates::apply_game_mod_updates,
            games::list_game_mods,
            games::import_game_mods,
            games::set_game_mod_enabled,
            games::remove_game_mod,
            offline::list_player_profiles,
            offline::add_offline_profile,
            offline::select_player_profile,
            offline::update_profile_note,
            offline::remove_player_profile,
            offline::get_offline_profile,
            offline::save_offline_profile,
            offline::remove_offline_profile
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Cobble Launcher");
}
