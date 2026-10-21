// SPDX-License-Identifier: MIT
use cobble_minecraft::{LauncherPreferences, read_launcher_preferences, save_launcher_preferences};
use serde::Serialize;
use std::path::PathBuf;
use tauri::WebviewWindow;

#[derive(Serialize)]
pub struct PreferencesView {
    memory_mb: u32,
    game_directory: String,
    custom_game_directory: bool,
}

fn standard_directory() -> Result<PathBuf, String> {
    Ok(dirs::data_local_dir()
        .ok_or("无法找到游戏目录")?
        .join("CobbleLauncher/games"))
}

fn view(preferences: LauncherPreferences) -> Result<PreferencesView, String> {
    let directory = preferences
        .game_directory
        .clone()
        .unwrap_or(standard_directory()?);
    let display = directory.display().to_string();
    Ok(PreferencesView {
        memory_mb: preferences.memory_mb,
        custom_game_directory: preferences.game_directory.is_some(),
        game_directory: if let Some(unc) = display.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            display.strip_prefix(r"\\?\").unwrap_or(&display).into()
        },
    })
}

pub fn game_directory() -> Result<PathBuf, String> {
    match read_launcher_preferences()
        .map_err(|e| e.to_string())?
        .game_directory
    {
        Some(directory) => Ok(directory),
        None => standard_directory(),
    }
}

#[tauri::command]
pub fn get_launcher_preferences() -> Result<PreferencesView, String> {
    view(read_launcher_preferences().map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn set_default_memory(memory_mb: u32) -> Result<PreferencesView, String> {
    let mut preferences = read_launcher_preferences().map_err(|e| e.to_string())?;
    preferences.memory_mb = memory_mb;
    save_launcher_preferences(&preferences).map_err(|e| e.to_string())?;
    view(preferences)
}

#[tauri::command]
pub async fn pick_default_game_directory(
    window: WebviewWindow,
) -> Result<Option<PreferencesView>, String> {
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择默认游戏位置")
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let mut preferences = read_launcher_preferences().map_err(|e| e.to_string())?;
    preferences.game_directory =
        Some(std::fs::canonicalize(folder.path()).map_err(|e| e.to_string())?);
    save_launcher_preferences(&preferences).map_err(|e| e.to_string())?;
    view(preferences).map(Some)
}

#[tauri::command]
pub fn reset_default_game_directory() -> Result<PreferencesView, String> {
    let mut preferences = read_launcher_preferences().map_err(|e| e.to_string())?;
    preferences.game_directory = None;
    save_launcher_preferences(&preferences).map_err(|e| e.to_string())?;
    view(preferences)
}
