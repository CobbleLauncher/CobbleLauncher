// SPDX-License-Identifier: MIT
use super::*;
use cobble_content::{LocalPack, LocalPackKind};

fn installed_directory(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    if game.status != "installed" {
        return Err("请先完成游戏安装".into());
    }
    Ok(game.directory.clone())
}

#[tauri::command]
pub async fn list_game_packs(
    app: AppHandle,
    id: String,
    kind: LocalPackKind,
) -> Result<Vec<LocalPack>, String> {
    let directory = installed_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        cobble_content::list_local_packs(&directory, kind).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn import_game_packs(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    kind: LocalPackKind,
) -> Result<Option<Vec<LocalPack>>, String> {
    let (game, _operation) = management::begin(&app, &id, true)?;
    update_game(&app, &id, |_| {}, false);
    let title = match kind {
        LocalPackKind::ResourcePack => "导入资源包",
        LocalPackKind::ShaderPack => "导入光影包",
    };
    let Some(files) = rfd::AsyncFileDialog::new()
        .set_title(title)
        .add_filter("ZIP", &["zip"])
        .set_parent(&window)
        .pick_files()
        .await
    else {
        return Ok(None);
    };
    let paths: Vec<_> = files.iter().map(|file| file.path().to_owned()).collect();
    tauri::async_runtime::spawn_blocking(move || {
        cobble_content::import_local_packs(&game.directory, kind, &paths).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map(Some)
}

#[tauri::command]
pub async fn remove_game_pack(
    app: AppHandle,
    id: String,
    kind: LocalPackKind,
    file_name: String,
) -> Result<Vec<LocalPack>, String> {
    let (game, _operation) = management::begin(&app, &id, true)?;
    update_game(&app, &id, |_| {}, false);
    tauri::async_runtime::spawn_blocking(move || {
        cobble_content::remove_local_pack(&game.directory, kind, &file_name)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn open_game_pack_directory(
    app: AppHandle,
    id: String,
    kind: LocalPackKind,
) -> Result<(), String> {
    let folder = installed_directory(&app, &id)?.join(match kind {
        LocalPackKind::ResourcePack => "resourcepacks",
        LocalPackKind::ShaderPack => "shaderpacks",
    });
    tauri::async_runtime::spawn_blocking(move || {
        if let Ok(metadata) = fs::symlink_metadata(&folder) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err("内容目录是链接，请检查游戏文件夹".to_owned());
                }
            }
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err("内容目录必须是普通文件夹".into());
            }
        }
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        open::that(folder).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
