// SPDX-License-Identifier: MIT
//! Default game-root relocation uses the existing verified copy engine and task queue.
use super::*;
use cobble_core::{OperationControl, storage_workspace as disk};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static PREFERENCE_GATE: Mutex<()> = Mutex::new(());
type Plans = HashMap<String, (Plan, Instant)>;
static PLANS: OnceLock<Mutex<Plans>> = OnceLock::new();
pub(crate) fn require_idle() -> Result<(), String> {
    if ACTIVE.load(Ordering::Acquire) {
        Err("DIRECTORY_MIGRATION_BUSY".into())
    } else {
        Ok(())
    }
}
pub(crate) fn preference_change() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    let guard = PREFERENCE_GATE
        .try_lock()
        .map_err(|_| "DIRECTORY_MIGRATION_BUSY")?;
    require_idle()?;
    Ok(guard)
}
struct Guard;
impl Guard {
    fn acquire() -> Result<Self, String> {
        let _preference = PREFERENCE_GATE
            .try_lock()
            .map_err(|_| "DIRECTORY_MIGRATION_BUSY")?;
        ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| "DIRECTORY_MIGRATION_BUSY".into())
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    id: String,
    relative: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Plan {
    pub id: String,
    source: PathBuf,
    target: PathBuf,
    entries: Vec<Entry>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    plan_id: String,
    source: String,
    target: String,
    game_count: usize,
    required_bytes: u64,
    available_bytes: Option<u64>,
    enough_space: Option<bool>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u8,
    plan: Plan,
    inventory: Option<String>,
    #[serde(default)]
    released: bool,
}
fn io_error(_: impl std::fmt::Display) -> String {
    "DIRECTORY_MIGRATION_IO".into()
}
fn copy_error(e: impl std::fmt::Display) -> String {
    let text = e.to_string();
    if text.contains("CANCELLED") {
        "DIRECTORY_MIGRATION_CANCELLED".into()
    } else if text.contains("LINK") {
        "DIRECTORY_MIGRATION_LINK".into()
    } else if text.contains("CHANGED") {
        "DIRECTORY_MIGRATION_CHANGED".into()
    } else if text.contains("SCAN_LIMIT") {
        "DIRECTORY_MIGRATION_LIMIT".into()
    } else {
        io_error(text)
    }
}
fn real_directory(path: &Path) -> Result<PathBuf, String> {
    let m = fs::symlink_metadata(path).map_err(io_error)?;
    let linked = m.file_type().is_symlink();
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        linked || m.file_attributes() & 0x400 != 0
    };
    if !m.is_dir() || linked {
        return Err("DIRECTORY_MIGRATION_LINK".into());
    }
    dunce::canonicalize(path).map_err(io_error)
}
fn resolved(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        dunce::canonicalize(path).map_err(io_error)
    } else {
        Ok(dunce::simplified(path).to_owned())
    }
}
fn destination(source: &Path, parent: &Path) -> Result<PathBuf, String> {
    if source.file_name().is_none() || source.parent().is_none() {
        return Err("DIRECTORY_MIGRATION_TARGET".into());
    }
    let parent = real_directory(parent)?;
    let target = parent.join(source.file_name().ok_or("DIRECTORY_MIGRATION_TARGET")?);
    if target.starts_with(source) || source.starts_with(&target) {
        return Err("DIRECTORY_MIGRATION_TARGET".into());
    }
    Ok(target)
}
fn entries(games: &[Game], source: &Path) -> Result<Vec<Entry>, String> {
    let mut result = Vec::new();
    for game in games {
        // Nonexistent registered games still retain their relative paths.
        let path = resolved(&game.directory)?;
        if let Ok(relative) = path.strip_prefix(source) {
            if game.launch_state != "idle" || game.content_installing || game.status == "installing"
            {
                return Err("DIRECTORY_MIGRATION_GAME_BUSY".into());
            }
            result.push(Entry {
                id: game.id.clone(),
                relative: relative.into(),
            });
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}
#[tauri::command]
pub async fn preview_game_directory_migration(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Option<Preview>, String> {
    require_idle()?;
    let source = real_directory(&crate::preferences::game_directory()?)?;
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let target = destination(&source, folder.path())?;
    if target.try_exists().map_err(io_error)? {
        return Err("DIRECTORY_MIGRATION_EXISTS".into());
    }
    let selected = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(io_error)?;
        if library.load_error.is_some() {
            return Err("DIRECTORY_MIGRATION_IO".into());
        }
        entries(&library.games, &source)?
    };
    let scan = source.clone();
    let required = tauri::async_runtime::spawn_blocking(move || {
        disk::size_game_root(&scan).map_err(copy_error)
    })
    .await
    .map_err(io_error)??;
    let free =
        cobble_platform::available_disk_space(target.parent().ok_or("DIRECTORY_MIGRATION_TARGET")?);
    let id = next_pack_id()?;
    let preview = Preview {
        plan_id: id.clone(),
        source: source.display().to_string(),
        target: target.display().to_string(),
        game_count: selected.len(),
        required_bytes: required,
        available_bytes: free,
        enough_space: free.map(|n| n >= required.saturating_add(16 * 1024 * 1024)),
    };
    let mut plans = PLANS.get_or_init(Mutex::default).lock().map_err(io_error)?;
    plans.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(1800));
    if plans.len() >= 8 {
        return Err("DIRECTORY_MIGRATION_BUSY".into());
    }
    plans.insert(
        id.clone(),
        (
            Plan {
                id,
                source,
                target,
                entries: selected,
            },
            Instant::now(),
        ),
    );
    Ok(Some(preview))
}
#[tauri::command]
pub fn start_game_directory_migration(app: AppHandle, plan_id: String) -> Result<String, String> {
    require_idle()?;
    let plan = PLANS
        .get_or_init(Mutex::default)
        .lock()
        .map_err(io_error)?
        .remove(&plan_id)
        .filter(|(_, at)| at.elapsed() < Duration::from_secs(1800))
        .ok_or("DIRECTORY_MIGRATION_EXPIRED")?
        .0;
    tasks::start_directory_migration_task(&app, plan)
}
fn receipt_path(plan: &Plan) -> Result<PathBuf, String> {
    if plan.id.is_empty()
        || plan.id.len() > 64
        || !plan.id.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Err("DIRECTORY_MIGRATION_TARGET".into());
    }
    Ok(plan
        .target
        .parent()
        .ok_or("DIRECTORY_MIGRATION_TARGET")?
        .join(format!(".cobble-directory-migration-{}.json", plan.id)))
}
fn fingerprint(inventory: &disk::Inventory) -> Result<String, String> {
    struct Writer(Sha256);
    impl Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Writer(Sha256::new());
    serde_json::to_writer(&mut writer, inventory).map_err(io_error)?;
    Ok(format!("{:x}", writer.0.finalize()))
}
fn write_receipt(plan: &Plan, inventory: Option<disk::Inventory>) -> Result<(), String> {
    let path = receipt_path(plan)?;
    let mut f = tempfile::NamedTempFile::new_in(path.parent().ok_or("DIRECTORY_MIGRATION_TARGET")?)
        .map_err(io_error)?;
    let verified = inventory.is_some();
    f.write_all(
        &serde_json::to_vec(&Receipt {
            version: 1,
            plan: plan.clone(),
            inventory: inventory.as_ref().map(fingerprint).transpose()?,
            released: false,
        })
        .map_err(io_error)?,
    )
    .map_err(io_error)?;
    f.as_file().sync_all().map_err(io_error)?;
    if verified {
        f.persist(path).map_err(io_error)?;
    } else {
        f.persist_noclobber(path).map_err(io_error)?;
    }
    Ok(())
}
fn read_receipt(plan: &Plan) -> Result<Receipt, String> {
    let path = receipt_path(plan)?;
    let m = fs::symlink_metadata(&path).map_err(io_error)?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > 32 * 1024 * 1024 {
        return Err("DIRECTORY_MIGRATION_TARGET".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err("DIRECTORY_MIGRATION_TARGET".into());
        }
    }
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    let parent = cap_std::fs::Dir::open_ambient_dir(
        path.parent().ok_or("DIRECTORY_MIGRATION_TARGET")?,
        cap_std::ambient_authority(),
    )
    .map_err(io_error)?;
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent
        .open_with(
            path.file_name().ok_or("DIRECTORY_MIGRATION_TARGET")?,
            &options,
        )
        .map_err(io_error)?;
    let receipt: Receipt =
        serde_json::from_reader(file.take(32 * 1024 * 1024 + 1)).map_err(io_error)?;
    if receipt.version != 1 || receipt.plan != *plan {
        return Err("DIRECTORY_MIGRATION_TARGET".into());
    }
    Ok(receipt)
}
fn prepare(plan: &Plan, cancel: &AtomicBool) -> Result<bool, String> {
    if destination(
        &real_directory(&plan.source)?,
        plan.target.parent().ok_or("DIRECTORY_MIGRATION_TARGET")?,
    )? != plan.target
    {
        return Err("DIRECTORY_MIGRATION_TARGET".into());
    }
    if receipt_path(plan)?.try_exists().map_err(io_error)? {
        let receipt = read_receipt(plan)?;
        if receipt.inventory.is_some() && !receipt.released {
            return Ok(true);
        }
        if receipt.released {
            return Err("DIRECTORY_MIGRATION_CHANGED".into());
        }
        if plan.target.try_exists().map_err(io_error)? {
            let target = real_directory(&plan.target)?;
            if target != plan.target
                || target.parent() != plan.target.parent()
                || target.starts_with(&plan.source)
                || plan.source.starts_with(&target)
            {
                return Err("DIRECTORY_MIGRATION_TARGET".into());
            }
            let source = disk::inventory_game_root(&plan.source, cancel).map_err(copy_error)?;
            let partial = disk::inventory_game_root(&target, cancel).map_err(copy_error)?;
            if partial
                .files
                .iter()
                .any(|(path, stamp)| source.files.get(path) != Some(stamp))
                || partial
                    .directories
                    .iter()
                    .any(|path| source.directories.binary_search(path).is_err())
            {
                return Err("DIRECTORY_MIGRATION_CHANGED".into());
            }
            // This exact absent-at-start child is bound to this durable task's
            // host-created receipt. An interrupted partial copy can be retried.
            fs::remove_dir_all(target).map_err(io_error)?;
        }
    } else {
        if plan.target.try_exists().map_err(io_error)? {
            return Err("DIRECTORY_MIGRATION_EXISTS".into());
        }
        write_receipt(plan, None)?;
    }
    Ok(false)
}
fn verify_retained(plan: &Plan, expected: &str, cancel: &AtomicBool) -> Result<(), String> {
    if real_directory(&plan.source)? != plan.source
        || real_directory(&plan.target)? != plan.target
        || plan.source.starts_with(&plan.target)
        || plan.target.starts_with(&plan.source)
    {
        return Err("DIRECTORY_MIGRATION_CHANGED".into());
    }
    let target = disk::inventory_game_root(&plan.target, cancel).map_err(copy_error)?;
    if fingerprint(&target)? != expected
        || disk::inventory_game_root(&plan.source, cancel).map_err(copy_error)? != target
    {
        return Err("DIRECTORY_MIGRATION_CHANGED".into());
    }
    Ok(())
}
fn retarget(games: &[Game], plan: &Plan) -> Result<Vec<Game>, String> {
    let mut updated = games.to_vec();
    for game in &mut updated {
        let path = resolved(&game.directory)?;
        if let Some(entry) = plan.entries.iter().find(|e| e.id == game.id) {
            if path != plan.source.join(&entry.relative)
                && path != plan.target.join(&entry.relative)
            {
                return Err("DIRECTORY_MIGRATION_CHANGED".into());
            }
            game.directory = plan.target.join(&entry.relative);
            game.log_path = None;
        } else if path.starts_with(&plan.source) {
            return Err("DIRECTORY_MIGRATION_CHANGED".into());
        }
    }
    if plan
        .entries
        .iter()
        .any(|e| !updated.iter().any(|g| g.id == e.id))
    {
        return Err("DIRECTORY_MIGRATION_CHANGED".into());
    }
    Ok(updated)
}
fn publish(app: &AppHandle, plan: &Plan) -> Result<(), String> {
    let current = real_directory(&crate::preferences::game_directory()?)?;
    if current != plan.source && current != plan.target {
        return Err("DIRECTORY_MIGRATION_CHANGED".into());
    }
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(io_error)?;
    let updated = retarget(&library.games, plan)?;
    let path = library_file()?;
    let parent = path.parent().ok_or("DIRECTORY_MIGRATION_IO")?;
    fs::create_dir_all(parent).map_err(io_error)?;
    let mut f = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
    f.write_all(&serde_json::to_vec_pretty(&updated).map_err(io_error)?)
        .map_err(io_error)?;
    f.as_file().sync_all().map_err(io_error)?;
    f.persist(path).map_err(io_error)?;
    library.games = updated;
    // Library and preferences are two durable files. A retry accepts already
    // retargeted entries and finishes the preference write after interruption.
    let mut preferences = cobble_minecraft::read_launcher_preferences().map_err(io_error)?;
    preferences.game_directory = Some(plan.target.clone());
    let result = cobble_minecraft::save_launcher_preferences(&preferences).map_err(io_error);
    for game in &library.games {
        if plan.entries.iter().any(|e| e.id == game.id) {
            let _ = app.emit("game-install", game);
        }
    }
    result
}
pub(super) async fn perform(
    app: &AppHandle,
    plan: &Plan,
    control: OperationControl,
) -> Result<(), String> {
    let guard = Guard::acquire()?;
    let mut leases = Vec::new();
    let mut locks = Vec::new();
    let mut locked = std::collections::HashSet::new();
    for entry in &plan.entries {
        let (game, lease) = management::begin(app, &entry.id, false)
            .map_err(|_| "DIRECTORY_MIGRATION_GAME_BUSY")?;
        if resolved(&game.directory)? != plan.source.join(&entry.relative)
            && resolved(&game.directory)? != plan.target.join(&entry.relative)
        {
            return Err("DIRECTORY_MIGRATION_CHANGED".into());
        }
        if sessions::recover(&game)?.is_some() {
            return Err("DIRECTORY_MIGRATION_GAME_BUSY".into());
        }
        if game.directory.exists() && locked.insert(game.directory.clone()) {
            locks.extend(
                workspace::world_locks(&game.directory)
                    .map_err(|_| "DIRECTORY_MIGRATION_GAME_BUSY")?,
            );
        }
        leases.push(lease);
    }
    let cancel = Arc::new(AtomicBool::new(control.is_cancelled()));
    let flag = cancel.clone();
    let watch = control.clone();
    let watcher = tauri::async_runtime::spawn(async move {
        watch.cancelled().await;
        flag.store(true, Ordering::Release);
    });
    let app = app.clone();
    let plan = plan.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let _leases = leases;
        let _locks = locks;
        let current = real_directory(&crate::preferences::game_directory()?)?;
        if current != plan.source && current != plan.target {
            return Err("DIRECTORY_MIGRATION_CHANGED".into());
        }
        let verified = prepare(&plan, &cancel)?;
        tasks::directory_migration_progress(&app, &plan.id, 0, 0);
        let inventory = disk::inventory_game_root(&plan.source, &cancel).map_err(copy_error)?;
        if verified {
            if read_receipt(&plan)?.inventory.as_deref() != Some(fingerprint(&inventory)?.as_str())
                || disk::inventory_game_root(&plan.target, &cancel).map_err(copy_error)?
                    != inventory
            {
                return Err("DIRECTORY_MIGRATION_CHANGED".into());
            }
        } else {
            if cobble_platform::available_disk_space(
                plan.target.parent().ok_or("DIRECTORY_MIGRATION_TARGET")?,
            )
            .is_some_and(|n| n < inventory.bytes.saturating_add(16 * 1024 * 1024))
            {
                return Err("DIRECTORY_MIGRATION_NO_SPACE".into());
            }
            let mut last = Instant::now() - Duration::from_secs(1);
            disk::copy_game_root_verified(
                &plan.source,
                &plan.target,
                &inventory,
                &cancel,
                |completed| {
                    if last.elapsed() >= Duration::from_millis(100) || completed == inventory.bytes
                    {
                        last = Instant::now();
                        tasks::directory_migration_progress(
                            &app,
                            &plan.id,
                            completed,
                            inventory.bytes,
                        );
                    }
                },
            )
            .map_err(copy_error)?;
            write_receipt(&plan, Some(inventory))?;
        }
        if cancel.load(Ordering::Acquire) {
            return Err("DIRECTORY_MIGRATION_CANCELLED".into());
        }
        publish(&app, &plan)
    })
    .await
    .map_err(io_error);
    watcher.abort();
    result?
}
#[tauri::command]
pub async fn open_game_directory_migration_source(
    app: AppHandle,
    task_id: String,
) -> Result<(), String> {
    let plan = tasks::directory_migration_plan(&app, &task_id, true)?;
    if read_receipt(&plan)?.inventory.is_none() || real_directory(&plan.source)? != plan.source {
        return Err("DIRECTORY_MIGRATION_TARGET".into());
    }
    tauri::async_runtime::spawn_blocking(move || open::that(plan.source).map_err(io_error))
        .await
        .map_err(io_error)?
}

#[tauri::command]
pub fn inspect_game_directory_migration(
    app: AppHandle,
    task_id: String,
) -> Result<serde_json::Value, String> {
    let plan = tasks::directory_migration_plan(&app, &task_id, false)?;
    let receipt = read_receipt(&plan).ok();
    Ok(
        serde_json::json!({"source":plan.source,"target":plan.target,"sourceRetained":!receipt.as_ref().is_some_and(|r| r.released) && plan.source.exists(),"canRecycle":cfg!(windows) && receipt.as_ref().is_some_and(|r| r.inventory.is_some() && !r.released)}),
    )
}

#[tauri::command]
pub async fn cleanup_game_directory_migration_source(
    app: AppHandle,
    task_id: String,
) -> Result<(), String> {
    let plan = tasks::directory_migration_plan(&app, &task_id, true)?;
    let guard = Guard::acquire()?;
    let mut leases = Vec::new();
    let mut locks = Vec::new();
    let mut locked = std::collections::HashSet::new();
    for entry in &plan.entries {
        let (game, lease) = management::begin(&app, &entry.id, false)
            .map_err(|_| "DIRECTORY_MIGRATION_GAME_BUSY")?;
        if resolved(&game.directory)? != plan.target.join(&entry.relative)
            || sessions::recover(&game)?.is_some()
        {
            return Err("DIRECTORY_MIGRATION_CHANGED".into());
        }
        for path in [plan.source.join(&entry.relative), game.directory] {
            if path.exists() && locked.insert(path.clone()) {
                locks.extend(
                    workspace::world_locks(&path).map_err(|_| "DIRECTORY_MIGRATION_GAME_BUSY")?,
                );
            }
        }
        leases.push(lease);
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let _leases = leases;
        let _locks = locks;
        let mut receipt = read_receipt(&plan)?;
        if receipt.released {
            return Ok(());
        }
        let expected = receipt
            .inventory
            .as_ref()
            .ok_or("DIRECTORY_MIGRATION_INCOMPLETE")?;
        let source = real_directory(&plan.source)?;
        let target = real_directory(&plan.target)?;
        if source != plan.source
            || target != plan.target
            || source.starts_with(&target)
            || target.starts_with(&source)
            || real_directory(&crate::preferences::game_directory()?)? != target
        {
            return Err("DIRECTORY_MIGRATION_CHANGED".into());
        }
        let cancel = AtomicBool::new(false);
        verify_retained(&plan, expected, &cancel)?;
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(io_error)?;
        for game in &library.games {
            let path = resolved(&game.directory)?;
            if path.starts_with(&source) || source.starts_with(&path) {
                return Err("DIRECTORY_MIGRATION_SOURCE_IN_USE".into());
            }
        }
        // Recycle only the exact canonical source bound to this verified receipt;
        // never permanently remove the original directory.
        cobble_platform::recycle_directory(&source)
            .map_err(|_| "DIRECTORY_MIGRATION_RECYCLE_FAILED")?;
        drop(library);
        receipt.released = true;
        let path = receipt_path(&plan)?;
        let mut f =
            tempfile::NamedTempFile::new_in(path.parent().ok_or("DIRECTORY_MIGRATION_TARGET")?)
                .map_err(io_error)?;
        f.write_all(&serde_json::to_vec(&receipt).map_err(io_error)?)
            .map_err(io_error)?;
        f.as_file().sync_all().map_err(io_error)?;
        f.persist(path).map_err(io_error)?;
        Ok(())
    })
    .await
    .map_err(io_error)?
}

#[cfg(test)]
#[path = "game_directory_migration/tests.rs"]
mod tests;
