// SPDX-License-Identifier: MIT
use super::*;

pub(super) struct Operation {
    app: AppHandle,
    id: String,
}
impl Drop for Operation {
    fn drop(&mut self) {
        let state = self.app.state::<GameLibrary>();
        if let Ok(mut library) = state.0.lock()
            && let Some(game) = library.games.iter_mut().find(|game| game.id == self.id)
        {
            game.content_installing = false;
            let _ = self.app.emit("game-install", &*game);
        }
    }
}

pub(super) fn begin(
    app: &AppHandle,
    id: &str,
    installed: bool,
) -> Result<(Game, Operation), String> {
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter_mut()
        .find(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    if game.status == "installing" || game.launch_state != "idle" || game.content_installing {
        return Err("请先关闭游戏或等待当前操作完成".into());
    }
    if installed && game.status != "installed" {
        return Err("请先完成游戏安装".into());
    }
    game.content_installing = true;
    let game = game.clone();
    drop(library);
    let operation = Operation {
        app: app.clone(),
        id: id.into(),
    };
    if game.directory.is_dir() {
        cobble_core::recover_modrinth_update(&game.directory).map_err(|e| e.to_string())?;
    }
    Ok((game, operation))
}

#[tauri::command]
pub async fn copy_game(
    app: AppHandle,
    id: String,
    name: String,
    include_saves: bool,
) -> Result<Game, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("副本名称需为 1–80 个字符".into());
    }
    let (source, _operation) = begin(&app, &id, true)?;
    let base = default_directory()?;
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let new_id = format!(
        "{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let target = base.join(&new_id);
    let from = source.directory.clone();
    let destination = target.clone();
    tauri::async_runtime::spawn_blocking(move || {
        cobble_core::copy_instance(&from, &destination, include_saves)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let mut game = source;
    game.id = new_id;
    game.name = name.into();
    game.directory = target;
    game.content_installing = false;
    game.progress = None;
    game.launch_state = idle();
    game.launch_error = None;
    game.log_path = None;
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|e| e.to_string())?;
    library.games.push(game.clone());
    if let Err(error) = save(&library.games) {
        library.games.pop();
        // The completed copy remains available on disk if writing the library
        // fails; never delete a user's copied files as a rollback.
        return Err(format!(
            "副本已保存到 {}，但未能更新游戏库：{error}",
            game.directory.display()
        ));
    }
    Ok(game)
}

#[tauri::command]
pub async fn export_game(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    include_saves: bool,
) -> Result<bool, String> {
    let (game, _operation) = begin(&app, &id, true)?;
    let filename: String = game
        .name
        .chars()
        .map(|c| if r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect();
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("导出整合包")
        .add_filter("Modrinth 整合包", &["mrpack"])
        .set_file_name(format!("{filename}.mrpack"))
        .save_file()
        .await
    else {
        return Ok(false);
    };
    let destination = file.path().to_path_buf();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temporary = destination.with_file_name(format!(".cobble-export-{nonce}.part"));
    tauri::async_runtime::spawn_blocking(move || {
        let kind = game
            .loader_version
            .as_ref()
            .map(|_| game.loader_kind.as_deref().unwrap_or("fabric"));
        let result = cobble_core::export_instance_mrpack(
            &game.directory,
            &temporary,
            &game.name,
            &game.version,
            kind,
            game.loader_version.as_deref(),
            include_saves,
        )
        .map_err(|e| e.to_string());
        result?;
        // The native save dialog obtains overwrite confirmation for this exact
        // destination. Build first, then replace the selected archive.
        let publish = fs::rename(&temporary, &destination).map_err(|e| e.to_string());
        if publish.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        publish
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(true)
}

fn deletion_target(game: &Game) -> Result<PathBuf, String> {
    if game.directory.file_name().and_then(|name| name.to_str()) != Some(game.id.as_str()) {
        return Err("这个目录不是启动器创建的独立游戏目录，请仅从游戏库移除".into());
    }
    let metadata = fs::symlink_metadata(&game.directory).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("游戏目录是链接，请仅从游戏库移除".into());
        }
    }
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("游戏目录无效，请仅从游戏库移除".into());
    }
    let resolved = fs::canonicalize(&game.directory).map_err(|e| e.to_string())?;
    let parent = fs::canonicalize(game.directory.parent().ok_or("无效的游戏目录")?)
        .map_err(|e| e.to_string())?;
    if resolved.parent() != Some(parent.as_path()) {
        return Err("游戏目录已改变，请仅从游戏库移除".into());
    }
    Ok(resolved)
}

#[tauri::command]
pub async fn remove_game(app: AppHandle, id: String, delete_files: bool) -> Result<(), String> {
    let (game, _operation) = begin(&app, &id, false)?;
    if delete_files && game.directory.exists() {
        let target = deletion_target(&game)?;
        tauri::async_runtime::spawn_blocking(move || fs::remove_dir_all(target))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    }
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|e| e.to_string())?;
    let index = library
        .games
        .iter()
        .position(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    let removed = library.games.remove(index);
    if let Err(error) = save(&library.games) {
        library.games.insert(index, removed);
        return Err(error);
    }
    library.mod_plans.remove(&id);
    library.update_plans.remove(&id);
    Ok(())
}
