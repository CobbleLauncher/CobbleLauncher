// SPDX-License-Identifier: MIT
use super::*;

pub(super) struct WorldSelection {
    preview_id: String,
    source: WorldArchiveSource,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct WorldArchiveSource {
    pub(super) path: PathBuf,
    size: u64,
    modified_millis: Option<u128>,
}
impl WorldArchiveSource {
    fn read(path: PathBuf) -> Result<Self, String> {
        let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
        if !metadata.is_file() {
            return Err("请选择 ZIP 备份文件".into());
        }
        Ok(Self {
            path,
            size: metadata.len(),
            modified_millis: metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .map(|value| value.as_millis()),
        })
    }
    pub(super) fn checked_path(&self) -> Result<&std::path::Path, String> {
        let current = Self::read(self.path.clone())?;
        if current.size != self.size || current.modified_millis != self.modified_millis {
            return Err("备份文件已变化，请重新选择并预览".into());
        }
        Ok(&self.path)
    }
}

#[derive(Serialize)]
pub struct WorldBackupPreview {
    preview_id: String,
    name: String,
    minecraft_version: Option<String>,
    metadata_error: bool,
    file_count: usize,
    total_bytes: u64,
    suggested_folder: String,
}

#[derive(Serialize)]
pub struct WorldRestoreResult {
    worlds: Vec<cobble_content::LocalWorld>,
    rollback_id: Option<String>,
}

pub(super) async fn restore_result(
    directory: PathBuf,
    outcome: cobble_core::WorldImportOutcome,
) -> Result<WorldRestoreResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(WorldRestoreResult {
            worlds: cobble_content::list_local_worlds(&directory).map_err(|e| e.to_string())?,
            rollback_id: outcome.backup_id,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn preview_world_backup(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
) -> Result<Option<WorldBackupPreview>, String> {
    let directory = installed_directory(&app, &id)?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择世界备份")
        .add_filter("ZIP 备份", &["zip"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let (source, preview) = tauri::async_runtime::spawn_blocking(move || {
        let source = WorldArchiveSource::read(file.path().to_path_buf())?;
        let preview = cobble_content::preview_world_archive(source.checked_path()?)
            .map_err(|e| e.to_string())?;
        Ok::<_, String>((source, preview))
    })
    .await
    .map_err(|e| e.to_string())??;
    let preview_id = next_pack_id()?;
    let mut suggested_folder = preview.suggested_folder_name.clone();
    let base = suggested_folder.clone();
    let mut suffix = 2;
    while directory.join("saves").join(&suggested_folder).exists() {
        suggested_folder = format!("{base}-{suffix}");
        suffix += 1;
    }
    let value = WorldBackupPreview {
        preview_id: preview_id.clone(),
        name: preview.world.name,
        minecraft_version: preview.world.minecraft_version,
        metadata_error: preview.world.metadata_error,
        file_count: preview.file_count,
        total_bytes: preview.total_bytes,
        suggested_folder,
    };
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|e| e.to_string())?;
    if !library.games.iter().any(|game| game.id == id) {
        return Err("找不到这个游戏".into());
    }
    library
        .world_previews
        .insert(id, WorldSelection { preview_id, source });
    Ok(Some(value))
}

#[tauri::command]
pub async fn restore_game_world(
    app: AppHandle,
    id: String,
    preview_id: String,
    folder_name: String,
    replace_existing: bool,
) -> Result<WorldRestoreResult, String> {
    let source = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|e| e.to_string())?;
        let selection = library
            .world_previews
            .get(&id)
            .filter(|selection| selection.preview_id == preview_id)
            .ok_or("请重新选择世界备份")?;
        selection.source.clone()
    };
    source.checked_path()?;
    match tasks::run(
        app,
        tasks::TaskRequest::WorldImport {
            game_id: id,
            source,
            folder_name,
            replace_existing,
            operation_id: next_pack_id()?,
        },
        "恢复世界",
    )
    .await?
    {
        tasks::TaskResult::Worlds(result) => Ok(result),
        _ => Err("世界恢复结果不可用".into()),
    }
}

#[tauri::command]
pub async fn list_game_world_backups(
    app: AppHandle,
    id: String,
) -> Result<Vec<cobble_core::WorldReplacementBackup>, String> {
    let directory = installed_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        cobble_core::list_world_replacement_backups(&directory).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn rollback_game_world_restore(
    app: AppHandle,
    id: String,
    rollback_id: String,
) -> Result<WorldRestoreResult, String> {
    match tasks::run(
        app,
        tasks::TaskRequest::WorldRollback {
            game_id: id,
            backup_id: rollback_id,
            operation_id: next_pack_id()?,
        },
        "恢复替换前的世界",
    )
    .await?
    {
        tasks::TaskResult::Worlds(result) => Ok(result),
        _ => Err("世界恢复结果不可用".into()),
    }
}

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
pub async fn list_game_worlds(
    app: AppHandle,
    id: String,
) -> Result<Vec<cobble_content::LocalWorld>, String> {
    let directory = installed_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        cobble_content::list_local_worlds(&directory).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn open_game_world_directory(
    app: AppHandle,
    id: String,
    folder_name: String,
) -> Result<(), String> {
    let directory = installed_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let folder =
            cobble_content::world_directory(&directory, &folder_name).map_err(|e| e.to_string())?;
        open::that(folder).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn export_game_world(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    folder_name: String,
) -> Result<bool, String> {
    let (game, operation) = management::begin(&app, &id, true)?;
    update_game(&app, &id, |_| {}, false);
    let directory = game.directory.clone();
    let selected = folder_name.clone();
    // Check the selected world before opening the native save dialog.
    tauri::async_runtime::spawn_blocking(move || {
        cobble_content::world_directory(&directory, &selected).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    let filename: String = folder_name
        .chars()
        .map(|c| {
            if r#"<>:"/\|?*"#.contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("导出世界备份")
        .add_filter("ZIP 备份", &["zip"])
        .set_file_name(format!("{filename}-backup.zip"))
        .save_file()
        .await
    else {
        return Ok(false);
    };
    let destination = file.path().to_path_buf();
    let temporary = destination.with_file_name(format!(".cobble-world-{}.part", next_pack_id()?));
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        cobble_core::export_world_backup(&game.directory, &folder_name, &temporary)
            .map_err(|e| e.to_string())?;
        // The save dialog confirms replacement of this exact destination.
        let result = fs::rename(&temporary, &destination).map_err(|e| e.to_string());
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_archive_source_roundtrips_and_requires_a_new_preview_after_edits() {
        let file = std::env::temp_dir().join(format!(
            "cobble-world-source-{}-{}",
            std::process::id(),
            next_pack_id().unwrap()
        ));
        fs::write(&file, b"preview source").unwrap();
        let source = WorldArchiveSource::read(file.clone()).unwrap();
        let saved = serde_json::to_vec(&source).unwrap();
        let restored: WorldArchiveSource = serde_json::from_slice(&saved).unwrap();
        assert_eq!(restored.checked_path().unwrap(), file);
        fs::write(&file, b"different archive after preview").unwrap();
        assert!(restored.checked_path().is_err());
        fs::remove_file(&file).unwrap();
        assert!(restored.checked_path().is_err());
    }
}
