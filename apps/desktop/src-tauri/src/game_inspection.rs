// SPDX-License-Identifier: MIT
//! Read-only, bounded metadata for registered games. No renderer-supplied paths.
use super::*;
use serde_json::{Value, json};
use std::{io::Read, path::Path};
#[path = "game_inspection_content.rs"]
mod content;
#[path = "game_inspection_files.rs"]
mod files;
#[path = "game_inspection_mods.rs"]
mod mods;
#[path = "game_inspection_servers.rs"]
pub(super) mod servers;
#[path = "game_inspection_settings.rs"]
mod settings;
const LIMIT: usize = 512;
const BYTE_LIMIT: u64 = 1024 * 1024;
fn linked(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
fn directory(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !linked(&meta) => Ok(true),
        Ok(_) => Err("Inspection directory is not a regular directory".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("Inspection directory is unavailable".into()),
    }
}
fn entries(path: &Path) -> Result<Vec<fs::DirEntry>, String> {
    if !directory(path)? {
        return Ok(vec![]);
    }
    let mut result = Vec::new();
    for entry in fs::read_dir(path).map_err(|_| "Inspection directory is unavailable")? {
        if result.len() >= LIMIT {
            return Err("Inspection entry limit exceeded".into());
        }
        let entry = entry.map_err(|_| "Inspection entry is unavailable")?;
        let meta =
            fs::symlink_metadata(entry.path()).map_err(|_| "Inspection entry is unavailable")?;
        if linked(&meta) {
            return Err("Inspection does not follow linked entries".into());
        }
        result.push(entry);
    }
    result.sort_by_key(|entry| entry.file_name());
    Ok(result)
}
fn text(path: &Path) -> Result<Option<String>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Inspection file is unavailable".into()),
    };
    if linked(&meta) || !meta.is_file() || meta.len() > BYTE_LIMIT {
        return Err("Inspection file exceeds safe limits".into());
    }
    let mut bytes = vec![];
    fs::File::open(path)
        .map_err(|_| "Inspection file is unavailable")?
        .take(BYTE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Inspection file is unavailable")?;
    if bytes.len() as u64 > BYTE_LIMIT {
        return Err("Inspection file exceeds safe limits".into());
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "Inspection file is not UTF-8".into())
}
fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(0)
}
fn unavailable(reason: &str) -> Value {
    json!({"items":[],"truncated":false,"available":false,"unavailable":reason})
}
pub async fn data_snapshot(app: &AppHandle, game_id: &str, scope: &str) -> Result<Value, String> {
    ensure_registered_game(app, game_id)?;
    let game = {
        let library = app.state::<GameLibrary>();
        let library = library.0.lock().map_err(|_| "Game library unavailable")?;
        library
            .games
            .iter()
            .find(|game| game.id == game_id)
            .cloned()
            .ok_or("Game not found")?
    };
    let scope = scope.to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        if !directory(&game.directory)? {
            return Ok(unavailable("Game directory is missing"));
        }
        let result = match scope.as_str() {
            "mods" | "packs" | "worlds" | "backups" | "datapacks" => {
                content::collect(&game, &scope)
            }
            "runtime" | "options" => settings::collect(&game, &scope),
            "screenshots" | "storage" => files::collect(&game, &scope),
            "servers" => servers::collect(&game.directory),
            _ => return Err("Unsupported inspection scope".into()),
        };
        Ok(result.unwrap_or_else(|reason| unavailable(&reason)))
    })
    .await
    .map_err(|_| "Inspection worker stopped")?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_files_reject_oversize_and_directory_inputs() {
        let root = tempfile::tempdir().unwrap();
        assert!(text(root.path()).is_err());
        let path = root.path().join("settings");
        fs::write(&path, vec![b'a'; BYTE_LIMIT as usize + 1]).unwrap();
        assert!(text(&path).is_err());
        assert_eq!(text(&root.path().join("missing")).unwrap(), None);
    }
    #[test]
    fn entry_limit_fails_instead_of_claiming_complete_inventory() {
        let root = tempfile::tempdir().unwrap();
        for n in 0..=LIMIT {
            fs::write(root.path().join(n.to_string()), b"").unwrap();
        }
        assert!(entries(root.path()).is_err());
    }
}
