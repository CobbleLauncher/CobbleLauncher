// SPDX-License-Identifier: MIT
use cobble_minecraft::{
    LaunchBehavior, LauncherPreferences, WindowSize, read_launcher_preferences,
    save_launcher_preferences,
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::WebviewWindow;

#[derive(Serialize)]
pub struct PreferencesView {
    memory_mb: u32,
    game_directory: String,
    custom_game_directory: bool,
    window_size: WindowSize,
    jvm_args: Vec<String>,
    launch_behavior: LaunchBehavior,
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
    Ok(PreferencesView {
        memory_mb: preferences.memory_mb,
        window_size: preferences.window_size,
        jvm_args: preferences.jvm_args,
        launch_behavior: preferences.launch_behavior,
        custom_game_directory: preferences.game_directory.is_some(),
        game_directory: display_directory(&directory),
    })
}

fn display_directory(directory: &std::path::Path) -> String {
    let display = directory.display().to_string();
    if let Some(unc) = display.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        display.strip_prefix(r"\\?\").unwrap_or(&display).into()
    }
}

#[derive(Serialize)]
pub struct DirectoryCandidate {
    path: String,
    available_bytes: Option<u64>,
}

fn directory_candidate(path: &std::path::Path) -> DirectoryCandidate {
    // A first-run destination may not exist yet. Probe its existing volume
    // without creating the game directory or changing the saved preference.
    let available_bytes = path
        .ancestors()
        .find(|ancestor| ancestor.is_dir())
        .and_then(cobble_platform::available_disk_space);
    DirectoryCandidate {
        path: display_directory(path),
        available_bytes,
    }
}

#[tauri::command]
pub async fn get_default_game_directory_candidate() -> Result<DirectoryCandidate, String> {
    tauri::async_runtime::spawn_blocking(|| game_directory().map(|path| directory_candidate(&path)))
        .await
        .map_err(|_| "无法读取游戏位置".to_string())?
}

#[tauri::command]
pub fn get_standard_game_directory() -> Result<DirectoryCandidate, String> {
    Ok(directory_candidate(&standard_directory()?))
}

fn selected_directory_candidate(path: &std::path::Path) -> Result<DirectoryCandidate, String> {
    if !path.is_dir() {
        return Err("请选择已存在的文件夹".into());
    }
    let directory = std::fs::canonicalize(path).map_err(|_| "无法访问这个文件夹")?;
    Ok(directory_candidate(&directory))
}

#[tauri::command]
pub async fn pick_default_game_directory_candidate(
    window: WebviewWindow,
) -> Result<Option<DirectoryCandidate>, String> {
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择默认游戏位置")
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    selected_directory_candidate(folder.path()).map(Some)
}

#[tauri::command]
pub fn set_default_advanced_runtime(
    window_size: WindowSize,
    jvm_args: Vec<String>,
    launch_behavior: LaunchBehavior,
) -> Result<PreferencesView, String> {
    cobble_minecraft::validate_advanced_runtime(&window_size, &jvm_args)
        .map_err(|e| e.to_string())?;
    let mut preferences = read_launcher_preferences().map_err(|e| e.to_string())?;
    preferences.window_size = window_size;
    preferences.jvm_args = jvm_args;
    preferences.launch_behavior = launch_behavior;
    save_launcher_preferences(&preferences).map_err(|e| e.to_string())?;
    view(preferences)
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
    let _directory_change = crate::games::directory_migration::preference_change()?;
    let mut preferences = read_launcher_preferences().map_err(|e| e.to_string())?;
    preferences.game_directory =
        Some(std::fs::canonicalize(folder.path()).map_err(|e| e.to_string())?);
    save_launcher_preferences(&preferences).map_err(|e| e.to_string())?;
    view(preferences).map(Some)
}

#[tauri::command]
pub fn reset_default_game_directory() -> Result<PreferencesView, String> {
    let _directory_change = crate::games::directory_migration::preference_change()?;
    let mut preferences = read_launcher_preferences().map_err(|e| e.to_string())?;
    preferences.game_directory = None;
    save_launcher_preferences(&preferences).map_err(|e| e.to_string())?;
    view(preferences)
}

fn typed_game_directory(value: &str) -> Result<PathBuf, String> {
    let path = std::path::Path::new(value.trim());
    if !path.is_absolute() {
        return Err("请输入完整的绝对目录路径".into());
    }
    if !path.is_dir() {
        return Err("请选择已存在的文件夹".into());
    }
    std::fs::canonicalize(path).map_err(|_| "无法访问这个文件夹".into())
}

#[tauri::command]
pub async fn set_default_game_directory(path: String) -> Result<PreferencesView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let directory = typed_game_directory(&path)?;
        let _directory_change = crate::games::directory_migration::preference_change()?;
        let mut preferences = read_launcher_preferences().map_err(|e| e.to_string())?;
        preferences.game_directory = Some(directory);
        save_launcher_preferences(&preferences).map_err(|e| e.to_string())?;
        view(preferences)
    })
    .await
    .map_err(|_| "默认目录保存任务意外退出")?
}

#[cfg(test)]
mod directory_tests {
    use super::*;
    #[test]
    fn missing_game_directory_probes_parent_without_creating_it() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("games").join("new-game");
        let candidate = directory_candidate(&destination);
        assert_eq!(candidate.path, display_directory(&destination));
        assert_eq!(
            candidate.available_bytes.is_some(),
            cobble_platform::available_disk_space(root.path()).is_some()
        );
        assert!(!destination.exists());
    }
    #[test]
    fn typed_directory_requires_existing_absolute_folder_without_creating_anything() {
        let directory = tempfile::tempdir().unwrap();
        let candidate = selected_directory_candidate(directory.path()).unwrap();
        assert_eq!(
            candidate.path,
            display_directory(&std::fs::canonicalize(directory.path()).unwrap())
        );
        assert_eq!(
            typed_game_directory(directory.path().to_str().unwrap()).unwrap(),
            std::fs::canonicalize(directory.path()).unwrap()
        );
        assert!(typed_game_directory("relative/path").is_err());
        let absent = directory.path().join("not-created");
        assert!(typed_game_directory(absent.to_str().unwrap()).is_err());
        assert!(selected_directory_candidate(&absent).is_err());
        assert!(!absent.exists());
        let file = directory.path().join("file.txt");
        std::fs::write(&file, b"safe").unwrap();
        assert!(typed_game_directory(file.to_str().unwrap()).is_err());
        assert!(selected_directory_candidate(&file).is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"safe");
    }
}
