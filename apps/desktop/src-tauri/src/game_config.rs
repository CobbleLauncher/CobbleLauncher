// SPDX-License-Identifier: MIT
//! Instance-scoped plain-text config editing with revisions and backup-before-write.
use super::*;
#[path = "game_config_activity.rs"]
mod activity;
use cap_std::fs::{Dir, OpenOptions};
use std::{
    io::{Read, Write},
    path::Path,
};
const LIMIT: u64 = 1024 * 1024;
const MAX_FILES: usize = 1024;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
    id: String,
    name: String,
    size: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBackup {
    id: String,
    created_at: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDocument {
    id: String,
    name: String,
    text: String,
    revision: String,
    backups: Vec<ConfigBackup>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBackupDocument {
    id: String,
    text: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRecoveryFile {
    id: String,
    name: String,
    backups: Vec<ConfigBackup>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRecoveryMetadata {
    files: Vec<ConfigRecoveryFile>,
    truncated: bool,
}
fn failure() -> String {
    "CONFIG_IO".into()
}
fn digest(bytes: &[u8]) -> String {
    cobble_plugin_host::digest(bytes)
}
fn supported(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "toml" | "json" | "json5" | "yaml" | "yml" | "properties" | "cfg" | "txt"
            )
        })
}
fn plain(dir: &Dir, path: &Path, directory: bool) -> Result<(), String> {
    let meta = dir.symlink_metadata(path).map_err(|_| failure())?;
    if meta.file_type().is_symlink()
        || (if directory {
            !meta.is_dir()
        } else {
            !meta.is_file()
        })
    {
        return Err("CONFIG_UNSUPPORTED".into());
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("CONFIG_UNSUPPORTED".into());
        }
    }
    Ok(())
}
fn root(game: &Path) -> Result<Dir, String> {
    Dir::open_ambient_dir(game, cap_std::ambient_authority()).map_err(|_| failure())
}
fn config(root: &Dir) -> Result<Option<Dir>, String> {
    if !root.try_exists("config").map_err(|_| failure())? {
        return Ok(None);
    }
    plain(root, Path::new("config"), true)?;
    root.open_dir("config").map(Some).map_err(|_| failure())
}
fn scan(
    dir: &Dir,
    prefix: &Path,
    depth: usize,
    seen: &mut usize,
    files: &mut Vec<ConfigFile>,
) -> Result<(), String> {
    if depth > 8 {
        return Ok(());
    }
    for entry in dir.entries().map_err(|_| failure())? {
        *seen += 1;
        if *seen > MAX_FILES {
            return Err("CONFIG_TOO_MANY_FILES".into());
        }
        let entry = entry.map_err(|_| failure())?;
        let local = entry.file_name();
        let path = Path::new(&local);
        let relative = prefix.join(path);
        let metadata = dir.symlink_metadata(path).map_err(|_| failure())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        #[cfg(windows)]
        {
            use cap_std::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        if metadata.is_dir() {
            scan(
                &dir.open_dir(path).map_err(|_| failure())?,
                &relative,
                depth + 1,
                seen,
                files,
            )?;
        } else if metadata.is_file()
            && metadata.len() <= LIMIT
            && let Some(name) = relative.to_str()
            && supported(name)
        {
            files.push(ConfigFile {
                id: digest(name.as_bytes()),
                name: name.into(),
                size: metadata.len(),
            });
        }
    }
    Ok(())
}
fn listing(root: &Dir) -> Result<Vec<ConfigFile>, String> {
    let Some(config) = config(root)? else {
        return Ok(vec![]);
    };
    let mut files = vec![];
    scan(&config, Path::new(""), 0, &mut 0, &mut files)?;
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}
fn selected(root: &Dir, id: &str) -> Result<ConfigFile, String> {
    if id.len() != 64 {
        return Err("CONFIG_NOT_FOUND".into());
    }
    listing(root)?
        .into_iter()
        .find(|f| f.id == id)
        .ok_or("CONFIG_NOT_FOUND".into())
}
fn bounded(dir: &Dir, name: &Path) -> Result<Vec<u8>, String> {
    plain(dir, name, false)?;
    let mut bytes = vec![];
    dir.open(name)
        .map_err(|_| failure())?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| failure())?;
    if bytes.len() as u64 > LIMIT {
        return Err("CONFIG_TOO_LARGE".into());
    }
    Ok(bytes)
}
fn backup_dir(root: &Dir, id: &str, create: bool) -> Result<Option<Dir>, String> {
    let base = Path::new(".cobble-config-backups");
    if !root.try_exists(base).map_err(|_| failure())? {
        if !create {
            return Ok(None);
        }
        root.create_dir(base).map_err(|_| failure())?;
    }
    plain(root, base, true)?;
    let base = root.open_dir(base).map_err(|_| failure())?;
    if !base.try_exists(id).map_err(|_| failure())? {
        if !create {
            return Ok(None);
        }
        base.create_dir(id).map_err(|_| failure())?;
    }
    plain(&base, Path::new(id), true)?;
    base.open_dir(id).map(Some).map_err(|_| failure())
}
fn backups(root: &Dir, id: &str) -> Result<Vec<ConfigBackup>, String> {
    let Some(dir) = backup_dir(root, id, false)? else {
        return Ok(vec![]);
    };
    let mut result = vec![];
    for entry in dir.entries().map_err(|_| failure())?.take(MAX_FILES) {
        let entry = entry.map_err(|_| failure())?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some((stamp, hash)) = name.split_once('-') else {
            continue;
        };
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        if let Ok(created_at) = stamp.parse()
            && plain(&dir, Path::new(name), false).is_ok()
        {
            result.push(ConfigBackup {
                id: name.into(),
                created_at,
            });
        }
    }
    result.sort_by_key(|item| std::cmp::Reverse(item.created_at));
    Ok(result)
}
fn document(root: &Dir, id: &str) -> Result<ConfigDocument, String> {
    let file = selected(root, id)?;
    let dir = config(root)?.ok_or("CONFIG_NOT_FOUND")?;
    let bytes = bounded(&dir, Path::new(&file.name))?;
    let revision = digest(&bytes);
    let text = String::from_utf8(bytes).map_err(|_| "CONFIG_NOT_UTF8")?;
    Ok(ConfigDocument {
        id: file.id,
        name: file.name,
        text,
        revision,
        backups: backups(root, id)?,
    })
}
fn backup_document(
    root: &Dir,
    config_id: &str,
    backup_id: &str,
) -> Result<ConfigBackupDocument, String> {
    selected(root, config_id)?;
    if !backups(root, config_id)?
        .iter()
        .any(|backup| backup.id == backup_id)
    {
        return Err("CONFIG_BACKUP_NOT_FOUND".into());
    }
    let dir = backup_dir(root, config_id, false)?.ok_or("CONFIG_BACKUP_NOT_FOUND")?;
    let bytes = bounded(&dir, Path::new(backup_id))?;
    if backup_id.split_once('-').map(|(_, hash)| hash) != Some(digest(&bytes).as_str()) {
        return Err("CONFIG_BACKUP_CHANGED".into());
    }
    let text = String::from_utf8(bytes).map_err(|_| "CONFIG_NOT_UTF8")?;
    Ok(ConfigBackupDocument {
        id: backup_id.into(),
        text,
    })
}
/// Bounded recovery history for the local recovery dashboard; never reads config contents.
pub(crate) fn recovery_metadata(directory: &Path) -> Result<ConfigRecoveryMetadata, String> {
    let root = root(directory)?;
    let files = listing(&root)?;
    let mut truncated = false;
    let mut result = Vec::new();
    for file in files {
        if file.name.len() > 256 {
            truncated = true;
            continue;
        }
        let history = backups(&root, &file.id)?;
        if history.is_empty() {
            continue;
        }
        if result.len() >= 64 {
            truncated = true;
            break;
        }
        let backups = if history.len() > 32 {
            truncated = true;
            history.into_iter().take(32).collect()
        } else {
            history
        };
        result.push(ConfigRecoveryFile {
            id: file.id,
            name: file.name,
            backups,
        });
    }
    Ok(ConfigRecoveryMetadata {
        files: result,
        truncated,
    })
}
/// Resolve only an opaque managed-config ID; never accepts a caller path.
pub(crate) fn report_current_config(
    directory: &Path,
    id: &str,
) -> Result<(String, Vec<u8>), String> {
    let root = root(directory)?;
    let file = selected(&root, id)?;
    let dir = config(&root)?.ok_or("CONFIG_NOT_FOUND")?;
    let bytes = bounded(&dir, Path::new(&file.name))?;
    Ok((file.name.replace("\\", "/"), bytes))
}
/// Immutable read-only diagnosis input; IDs are resolved within this game's config root.
pub(crate) fn report_snapshot(
    directory: &Path,
    id: &str,
    expected_revision: &str,
) -> Result<(Vec<u8>, String), String> {
    let root = root(directory)?;
    let file = selected(&root, id)?;
    let dir = config(&root)?.ok_or("CONFIG_NOT_FOUND")?;
    let bytes = bounded(&dir, Path::new(&file.name))?;
    if digest(&bytes) != expected_revision {
        return Err("CONFIG_CHANGED".into());
    }
    std::str::from_utf8(&bytes).map_err(|_| "CONFIG_NOT_UTF8")?;
    let extension = Path::new(&file.name)
        .extension()
        .and_then(|value| value.to_str())
        .ok_or("CONFIG_UNSUPPORTED")?
        .to_ascii_lowercase();
    Ok((bytes, extension))
}
fn replace(root: &Dir, id: &str, text: &str, revision: &str) -> Result<ConfigDocument, String> {
    if text.len() as u64 > LIMIT {
        return Err("CONFIG_TOO_LARGE".into());
    }
    let file = selected(root, id)?;
    let config = config(root)?.ok_or("CONFIG_NOT_FOUND")?;
    let path = Path::new(&file.name);
    let old = bounded(&config, path)?;
    if digest(&old) != revision {
        return Err("CONFIG_CHANGED".into());
    }
    let parent = path.parent().unwrap_or(Path::new(""));
    let name = path.file_name().ok_or_else(failure)?;
    let parent = if parent.as_os_str().is_empty() {
        config.try_clone().map_err(|_| failure())?
    } else {
        config.open_dir(parent).map_err(|_| failure())?
    };
    write_existing(root, id, &parent, Path::new(name), &old, text, revision)?;
    document(root, id)
}

#[path = "game_config/keybindings.rs"]
pub mod keybindings;
fn write_existing(
    root: &Dir,
    id: &str,
    parent: &Dir,
    name: &Path,
    old: &[u8],
    text: &str,
    revision: &str,
) -> Result<(), String> {
    if text.len() as u64 > LIMIT {
        return Err("CONFIG_TOO_LARGE".into());
    }
    if digest(&bounded(parent, name)?) != revision {
        return Err("CONFIG_CHANGED".into());
    }
    let permissions = parent.metadata(name).map_err(|_| failure())?.permissions();
    let backups = backup_dir(root, id, true)?.ok_or_else(failure)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| failure())?
        .as_millis();
    let backup = format!("{stamp}-{}", digest(old));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    match backups.open_with(&backup, &options) {
        Ok(mut out) => {
            out.set_permissions(permissions.clone())
                .map_err(|_| failure())?;
            out.write_all(old).map_err(|_| failure())?;
            out.sync_all().map_err(|_| failure())?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if bounded(&backups, Path::new(&backup))? != old {
                return Err("CONFIG_BACKUP_CHANGED".into());
            }
        }
        Err(_) => return Err(failure()),
    }
    let temporary = format!(".cobble-config-{stamp}-{}.tmp", std::process::id());
    let mut temporary_created = false;
    let result: Result<(), String> = (|| {
        let mut out = parent
            .open_with(&temporary, &options)
            .map_err(|_| failure())?;
        temporary_created = true;
        out.set_permissions(permissions.clone())
            .map_err(|_| failure())?;
        out.write_all(text.as_bytes()).map_err(|_| failure())?;
        out.sync_all().map_err(|_| failure())?;
        drop(out);
        if digest(&bounded(parent, name)?) != revision {
            return Err("CONFIG_CHANGED".into());
        }
        parent
            .rename(&temporary, parent, name)
            .map_err(|_| failure())?;
        Ok(())
    })();
    if result.is_err() && temporary_created {
        let _ = parent.remove_file(&temporary);
    }
    result?;
    Ok(())
}
fn save(root: &Dir, id: &str, text: &str, revision: &str) -> Result<ConfigDocument, String> {
    let before = document(root, id)?;
    let old = &before.text;
    let mut text = if old.contains("\r\n") && !old.replace("\r\n", "").contains('\n') {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text.to_string()
    };
    if old.starts_with('\u{feff}') && !text.starts_with('\u{feff}') {
        text.insert(0, '\u{feff}');
    }
    replace(root, id, &text, revision)
}

/// File-level adoption for a reviewed change share while the caller holds the
/// existing stopped-game lease. Uses this editor's normal backup-before-write.
pub(crate) fn inspect_shared_config(
    directory: &Path,
    path: &str,
) -> Result<Option<(String, String, String)>, String> {
    let relative = path
        .strip_prefix("config/")
        .ok_or("CHANGE_SHARE_CONFIG_INVALID")?;
    let root = root(directory)?;
    let Some(file) = listing(&root)?
        .into_iter()
        .find(|file| file.name.replace('\\', "/") == relative)
    else {
        return Ok(None);
    };
    let current = document(&root, &file.id)?;
    Ok(Some((current.id, current.text, current.revision)))
}

pub(crate) fn adopt_shared_config(
    directory: &Path,
    path: &str,
    text: &str,
    expected_revision: Option<&str>,
) -> Result<(String, Option<String>, String), String> {
    let root = root(directory)?;
    if let Some((id, _, revision)) = inspect_shared_config(directory, path)? {
        if Some(revision.as_str()) != expected_revision {
            return Err("CHANGE_SHARE_CONFIG_CHANGED".into());
        }
        let before = backups(&root, &id)?;
        let after = replace(&root, &id, text, &revision)?;
        let backup = after
            .backups
            .iter()
            .find(|item| !before.iter().any(|old| old.id == item.id))
            .or_else(|| after.backups.first())
            .ok_or("CHANGE_SHARE_CONFIG_BACKUP_MISSING")?;
        return Ok((id, Some(backup.id.clone()), after.revision));
    }
    if expected_revision.is_some() {
        return Err("CHANGE_SHARE_CONFIG_CHANGED".into());
    }
    let safe =
        cobble_store::SafeRelativePath::parse(path).map_err(|_| "CHANGE_SHARE_CONFIG_INVALID")?;
    if !safe.as_str().starts_with("config/") || !supported(path) {
        return Err("CHANGE_SHARE_CONFIG_INVALID".into());
    }
    use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
    let parts: Vec<_> = safe.as_str().split('/').collect();
    let mut parent = root;
    for part in &parts[..parts.len() - 1] {
        parent = match parent.open_dir_nofollow(part) {
            Ok(next) => next,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                parent
                    .create_dir(part)
                    .map_err(|_| "CHANGE_SHARE_CONFIG_IO")?;
                parent
                    .open_dir_nofollow(part)
                    .map_err(|_| "CHANGE_SHARE_CONFIG_IO")?
            }
            Err(_) => return Err("CHANGE_SHARE_CONFIG_IO".into()),
        };
    }
    let name = parts[parts.len() - 1];
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut output = parent
        .open_with(name, &options)
        .map_err(|_| "CHANGE_SHARE_CONFIG_CHANGED")?;
    let written = output
        .write_all(text.as_bytes())
        .and_then(|_| output.sync_all());
    drop(output);
    if written.is_err() {
        let _ = parent.remove_file(name);
        return Err("CHANGE_SHARE_CONFIG_IO".into());
    }
    let mut native_name = std::path::PathBuf::new();
    for part in &parts[1..] {
        native_name.push(part);
    }
    let name = native_name.to_str().ok_or("CHANGE_SHARE_CONFIG_INVALID")?;
    Ok((digest(name.as_bytes()), None, digest(text.as_bytes())))
}

pub(crate) fn remove_shared_created_config(
    directory: &Path,
    path: &str,
    expected_revision: &str,
) -> Result<(), String> {
    let safe =
        cobble_store::SafeRelativePath::parse(path).map_err(|_| "CHANGE_SHARE_CONFIG_INVALID")?;
    if !safe.as_str().starts_with("config/") {
        return Err("CHANGE_SHARE_CONFIG_INVALID".into());
    }
    let root = root(directory)?;
    let (_, _, revision) =
        inspect_shared_config(directory, path)?.ok_or("CHANGE_SHARE_CONFIG_CHANGED")?;
    if revision != expected_revision {
        return Err("CHANGE_SHARE_CONFIG_CHANGED".into());
    }
    use cap_fs_ext::DirExt;
    let parts: Vec<_> = safe.as_str().split('/').collect();
    let mut parent = root;
    for part in &parts[..parts.len() - 1] {
        parent = parent
            .open_dir_nofollow(part)
            .map_err(|_| "CHANGE_SHARE_CONFIG_CHANGED")?;
    }
    parent
        .remove_file(parts[parts.len() - 1])
        .map_err(|_| "CHANGE_SHARE_CONFIG_IO".into())
}
#[tauri::command]
pub async fn list_game_configs(app: AppHandle, game_id: String) -> Result<Vec<ConfigFile>, String> {
    let directory = game_directory(&app, &game_id)?;
    tauri::async_runtime::spawn_blocking(move || listing(&root(&directory)?))
        .await
        .map_err(|_| failure())?
}
#[tauri::command]
pub async fn read_game_config(
    app: AppHandle,
    game_id: String,
    config_id: String,
) -> Result<ConfigDocument, String> {
    let directory = game_directory(&app, &game_id)?;
    tauri::async_runtime::spawn_blocking(move || document(&root(&directory)?, &config_id))
        .await
        .map_err(|_| failure())?
}
pub async fn read_game_config_backup(
    app: AppHandle,
    game_id: String,
    config_id: String,
    backup_id: String,
) -> Result<ConfigBackupDocument, String> {
    let directory = game_directory(&app, &game_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        backup_document(&root(&directory)?, &config_id, &backup_id)
    })
    .await
    .map_err(|_| failure())?
}
#[tauri::command]
pub async fn save_game_config(
    app: AppHandle,
    game_id: String,
    config_id: String,
    text: String,
    expected_revision: String,
) -> Result<ConfigDocument, String> {
    let activity = activity::Activity::new("save", &game_id, &config_id);
    let (game, operation) = management::begin(&app, &game_id, true).map_err(|_| "CONFIG_BUSY")?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        save(
            &root(&game.directory)?,
            &config_id,
            &text,
            &expected_revision,
        )
    })
    .await
    .map_err(|_| failure())?;
    activity.finish(result)
}
fn restore(
    root: &Dir,
    config_id: &str,
    backup_id: &str,
    expected_revision: &str,
) -> Result<ConfigDocument, String> {
    let text = backup_document(root, config_id, backup_id)?.text;
    replace(root, config_id, &text, expected_revision)
}
#[tauri::command]
pub async fn restore_game_config_backup(
    app: AppHandle,
    game_id: String,
    config_id: String,
    backup_id: String,
    expected_revision: String,
) -> Result<ConfigDocument, String> {
    let activity = activity::Activity::new("restore", &game_id, &config_id);
    let (game, operation) = management::begin(&app, &game_id, true).map_err(|_| "CONFIG_BUSY")?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let root = root(&game.directory)?;
        restore(&root, &config_id, &backup_id, &expected_revision)
    })
    .await
    .map_err(|_| failure())?;
    activity.finish(result)
}

#[cfg(test)]
#[path = "game_config_tests.rs"]
mod tests;
