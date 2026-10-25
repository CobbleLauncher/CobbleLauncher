// SPDX-License-Identifier: MIT
//! Main-renderer copy jobs reuse the verified instance copy engine and durable
//! task queue. No plugin identity, permission grant or experiment is synthesized.
use super::*;
use cobble_core::{OperationControl, storage_workspace as disk};
use std::{
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
const RECEIPT: &str = ".cobble-builtin-copy.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ownership {
    version: u32,
    source_id: String,
    target_id: String,
    source_directory: PathBuf,
    name: String,
    include_worlds: bool,
}
fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn error() -> String {
    "GAME_COPY_IO".into()
}
fn valid(id: &str, name: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        || name.trim().is_empty()
        || name.chars().count() > 80
        || name.chars().any(char::is_control)
    {
        return Err("GAME_COPY_INVALID".into());
    }
    Ok(())
}
fn target(base: &Path, id: &str) -> Result<PathBuf, String> {
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("GAME_COPY_TARGET_INVALID".into());
    }
    let metadata = fs::symlink_metadata(base).map_err(|_| error())?;
    if !metadata.is_dir() || linked(&metadata) {
        return Err("GAME_COPY_TARGET_INVALID".into());
    }
    let canonical = dunce::canonicalize(base).map_err(|_| error())?;
    if canonical != base {
        return Err("GAME_COPY_TARGET_INVALID".into());
    }
    Ok(canonical.join(id))
}
fn sidecar(base: &Path, id: &str) -> PathBuf {
    base.join(format!(".cobble-game-copy-{id}.json"))
}
fn read_ownership(path: &Path) -> Result<Ownership, String> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    let metadata = fs::symlink_metadata(path).map_err(|_| error())?;
    if !metadata.is_file() || linked(&metadata) || metadata.len() > 8192 {
        return Err("GAME_COPY_TARGET_UNOWNED".into());
    }
    let parent = cap_std::fs::Dir::open_ambient_dir(
        path.parent().ok_or_else(error)?,
        cap_std::ambient_authority(),
    )
    .map_err(|_| error())?;
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent
        .open_with(path.file_name().ok_or_else(error)?, &options)
        .map_err(|_| error())?;
    serde_json::from_reader(file.take(8193)).map_err(|_| "GAME_COPY_TARGET_UNOWNED".into())
}
fn remove_owned(base: &Path, owned: &Ownership) -> Result<(), String> {
    let directory = target(base, &owned.target_id)?;
    if read_ownership(&sidecar(base, &owned.target_id))? != *owned {
        return Err("GAME_COPY_TARGET_UNOWNED".into());
    }
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(error()),
    };
    if !metadata.is_dir() || linked(&metadata) {
        return Err("GAME_COPY_TARGET_UNOWNED".into());
    }
    let resolved = dunce::canonicalize(&directory).map_err(|_| error())?;
    if resolved != directory
        || resolved.parent() != Some(base)
        || resolved.starts_with(&owned.source_directory)
        || owned.source_directory.starts_with(&resolved)
    {
        return Err("GAME_COPY_TARGET_UNOWNED".into());
    }
    // The exact canonical child and its host-created ownership record were
    // checked above. No source or arbitrary renderer path can reach this delete.
    fs::remove_dir_all(&resolved).map_err(|_| error())
}
fn prepare(base: &Path, owned: &Ownership) -> Result<PathBuf, String> {
    let directory = target(base, &owned.target_id)?;
    let marker = sidecar(base, &owned.target_id);
    if marker.try_exists().map_err(|_| error())? {
        remove_owned(base, owned)?;
    } else {
        if directory.try_exists().map_err(|_| error())? {
            return Err("GAME_COPY_TARGET_UNOWNED".into());
        }
        let mut file = tempfile::NamedTempFile::new_in(base).map_err(|_| error())?;
        file.write_all(&serde_json::to_vec(owned).map_err(|_| error())?)
            .map_err(|_| error())?;
        file.as_file().sync_all().map_err(|_| error())?;
        file.persist_noclobber(&marker).map_err(|_| error())?;
    }
    Ok(directory)
}
fn receipt(directory: &Path, owned: &Ownership) -> Result<(), String> {
    let root = fs::symlink_metadata(directory).map_err(|_| error())?;
    if !root.is_dir() || linked(&root) {
        return Err("GAME_COPY_TARGET_UNOWNED".into());
    }
    if let Ok(metadata) = fs::symlink_metadata(directory.join(RECEIPT))
        && (!metadata.is_file() || linked(&metadata))
    {
        return Err("GAME_COPY_TARGET_UNOWNED".into());
    }
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| error())?;
    file.write_all(&serde_json::to_vec(owned).map_err(|_| error())?)
        .map_err(|_| error())?;
    file.as_file().sync_all().map_err(|_| error())?;
    file.persist(directory.join(RECEIPT)).map_err(|_| error())?;
    Ok(())
}
fn published(app: &AppHandle, directory: &Path, owned: &Ownership) -> Result<Option<Game>, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|_| error())?;
    let Some(game) = library.games.iter().find(|g| g.id == owned.target_id) else {
        return Ok(None);
    };
    let metadata = fs::symlink_metadata(directory).map_err(|_| error())?;
    if !metadata.is_dir()
        || linked(&metadata)
        || dunce::canonicalize(directory).map_err(|_| error())? != directory
    {
        return Err("GAME_COPY_TARGET_UNOWNED".into());
    }
    if game.directory != directory
        || game.name != owned.name
        || game.status != "installed"
        || read_ownership(&directory.join(RECEIPT))? != *owned
    {
        return Err("GAME_COPY_TARGET_UNOWNED".into());
    }
    Ok(Some(game.clone()))
}
fn publish(app: &AppHandle, directory: &Path, owned: &Ownership) -> Result<(), String> {
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|_| error())?;
    if library.load_error.is_some()
        || library
            .games
            .iter()
            .any(|g| g.id == owned.target_id || g.directory == directory)
    {
        return Err("GAME_COPY_LIBRARY_CHANGED".into());
    }
    let mut game = library
        .games
        .iter()
        .find(|g| {
            g.id == owned.source_id
                && dunce::canonicalize(&g.directory).ok().as_ref() == Some(&owned.source_directory)
        })
        .cloned()
        .ok_or("GAME_COPY_SOURCE_CHANGED")?;
    game.id = owned.target_id.clone();
    game.name = owned.name.clone();
    game.directory = directory.into();
    game.content_installing = false;
    game.progress = None;
    game.error = None;
    game.launch_state = idle();
    game.launch_error = None;
    game.log_path = None;
    game.last_launched_at = None;
    let mut updated = library.games.clone();
    updated.push(game.clone());
    let path = library_file()?;
    let parent = path.parent().ok_or_else(error)?;
    fs::create_dir_all(parent).map_err(|_| error())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| error())?;
    file.write_all(&serde_json::to_vec_pretty(&updated).map_err(|_| error())?)
        .map_err(|_| error())?;
    file.as_file().sync_all().map_err(|_| error())?;
    file.persist(path).map_err(|_| error())?;
    library.games = updated;
    drop(library);
    let _ = app.emit("game-install", &game);
    Ok(())
}
fn safe_copy_error(cause: impl std::fmt::Display) -> String {
    let message = cause.to_string();
    if message.contains("STORAGE_CANCELLED") {
        "GAME_COPY_CANCELLED".into()
    } else if message.contains("STORAGE_SOURCE_CHANGED") {
        "GAME_COPY_SOURCE_CHANGED".into()
    } else if message.contains("STORAGE_LINK_DENIED") {
        "GAME_COPY_LINK_DENIED".into()
    } else {
        error()
    }
}

#[tauri::command]
pub async fn start_game_copy(
    app: AppHandle,
    id: String,
    name: String,
    include_saves: Option<bool>,
) -> Result<String, String> {
    valid(&id, &name)?;
    workspace::snapshot(&app, &id)?;
    let base = default_directory()?;
    fs::create_dir_all(&base).map_err(|_| error())?;
    let parent = dunce::canonicalize(base).map_err(|_| error())?;
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| error())?;
    let target_id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    tasks::start_game_copy_task(
        &app,
        &id,
        &target_id,
        &parent,
        name.trim(),
        include_saves.unwrap_or(false),
    )
}

pub(crate) async fn perform(
    app: &AppHandle,
    game_id: &str,
    target_id: &str,
    target_parent: &Path,
    name: &str,
    include_worlds: bool,
    control: OperationControl,
) -> Result<(), String> {
    valid(game_id, name)?;
    let directory = target(target_parent, target_id)?;
    let initial = workspace::snapshot(app, game_id)?;
    let owned = Ownership {
        version: 1,
        source_id: game_id.into(),
        target_id: target_id.into(),
        source_directory: dunce::canonicalize(&initial.directory).map_err(|_| error())?,
        name: name.into(),
        include_worlds,
    };
    if let Some(game) = published(app, &directory, &owned)? {
        if read_ownership(&sidecar(target_parent, target_id))
            .ok()
            .as_ref()
            == Some(&owned)
        {
            let _ = fs::remove_file(sidecar(target_parent, target_id));
        }
        let _ = app.emit("game-install", &game);
        return Ok(());
    }
    control
        .check_cancelled()
        .map_err(|_| "GAME_COPY_CANCELLED")?;
    let (current, lease) = workspace::acquire(app, game_id)?;
    if dunce::canonicalize(&current.directory).map_err(|_| error())? != owned.source_directory {
        return Err("GAME_COPY_SOURCE_CHANGED".into());
    }
    let worlds = workspace::world_locks(&current.directory)?;
    let cancel = Arc::new(AtomicBool::new(control.is_cancelled()));
    let flag = cancel.clone();
    let watch = control.clone();
    let watcher = tauri::async_runtime::spawn(async move {
        watch.cancelled().await;
        flag.store(true, Ordering::Release);
    });
    let worker_app = app.clone();
    let base = target_parent.to_path_buf();
    let progress_id = target_id.to_owned();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        let _worlds = worlds;
        let target = prepare(&base, &owned)?;
        let work = (|| {
            tasks::game_copy_progress(&worker_app, &progress_id, 0, 0);
            let inventory = disk::inventory_instance(&current.directory, include_worlds, &cancel)
                .map_err(safe_copy_error)?;
            if cobble_platform::available_disk_space(&base)
                .is_some_and(|n| n < inventory.bytes.saturating_add(16 * 1024 * 1024))
            {
                return Err("GAME_COPY_NO_SPACE".into());
            }
            let mut last = Instant::now() - Duration::from_secs(1);
            disk::copy_instance_verified(
                &current.directory,
                &target,
                &inventory,
                include_worlds,
                &cancel,
                |completed| {
                    if last.elapsed() >= Duration::from_millis(100) || completed == inventory.bytes
                    {
                        last = Instant::now();
                        tasks::game_copy_progress(
                            &worker_app,
                            &progress_id,
                            completed,
                            inventory.bytes,
                        );
                    }
                },
            )
            .map_err(safe_copy_error)?;
            if cancel.load(Ordering::Acquire) {
                return Err("GAME_COPY_CANCELLED".into());
            }
            receipt(&target, &owned)?;
            publish(&worker_app, &target, &owned)
        })();
        if work.is_err() {
            let _ = remove_owned(&base, &owned);
        }
        if work.is_ok() || !target.exists() {
            let _ = fs::remove_file(sidecar(&base, &owned.target_id));
        }
        work
    })
    .await
    .map_err(|_| error());
    watcher.abort();
    result?
}

#[cfg(test)]
#[path = "native_builtin_copy/tests.rs"]
mod tests;
