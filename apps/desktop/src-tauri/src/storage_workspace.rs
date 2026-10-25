// SPDX-License-Identifier: MIT
//! Host-owned folder selections and relocation tasks.
use super::*;
use cobble_core::storage_workspace as disk;
use cobble_plugin_api::{InspectionScope, PermissionNode};
use serde_json::{Value, json};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

type Auth = crate::plugins::plugin_views::ViewAuthorization;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverviewRequest {
    pub game_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameRequest {
    pub game_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanRequest {
    pub plan_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskRequest {
    pub task_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyRequest {}
#[derive(Clone)]
struct Plan {
    owner: String,
    game_id: String,
    source: PathBuf,
    target: PathBuf,
    inventory: disk::Inventory,
    created: Instant,
}
struct Task {
    owner: String,
    cancel: Arc<AtomicBool>,
    value: Value,
}
#[derive(Default)]
struct State {
    plans: HashMap<String, Plan>,
    tasks: HashMap<String, Task>,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State::default()))
}
fn nonce() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{:x}{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}
fn read_permission(auth: &Auth) -> Result<(), String> {
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    if !auth.manifest.permissions.as_deref().unwrap_or_default().iter().any(|node| matches!(node, PermissionNode::GameInspectionRead { scopes } if scopes.contains(&InspectionScope::Storage))) { return Err("PLUGIN_PERMISSION_DENIED".into()); }
    Ok(())
}
fn action_permission(auth: &Auth, moving: bool) -> Result<(), String> {
    read_permission(auth)?;
    if !auth
        .manifest
        .permissions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|node| {
            if moving {
                matches!(node, PermissionNode::InstanceMoveRequest {})
            } else {
                matches!(node, PermissionNode::StorageCacheClear {})
            }
        })
    {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    Ok(())
}
fn scoped(auth: &Auth, id: &str) -> Result<(), String> {
    if auth.instance_id.as_ref().is_some_and(|bound| bound != id) {
        return Err("PLUGIN_GAME_SCOPE".into());
    }
    Ok(())
}
fn game(app: &AppHandle, id: &str) -> Result<Game, String> {
    app.state::<GameLibrary>()
        .0
        .lock()
        .map_err(|_| "STORAGE_STATE_UNAVAILABLE")?
        .games
        .iter()
        .find(|game| game.id == id)
        .cloned()
        .ok_or_else(|| "PLUGIN_GAME_SCOPE".into())
}
fn snapshot_root(id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("PLUGIN_GAME_SCOPE".into());
    }
    Ok(dirs::data_local_dir()
        .ok_or("STORAGE_STATE_UNAVAILABLE")?
        .join("CobbleLauncher/world-snapshots")
        .join(id))
}
fn safe_error(error: impl std::fmt::Display) -> String {
    let text = error.to_string();
    for code in [
        "STORAGE_CANCELLED",
        "STORAGE_SOURCE_CHANGED",
        "STORAGE_VERIFY_FAILED",
        "STORAGE_TARGET_OVERLAP",
        "STORAGE_LINK_DENIED",
        "STORAGE_SCAN_LIMIT",
    ] {
        if text.contains(code) {
            return code.into();
        }
    }
    "STORAGE_IO_FAILED".into()
}
pub(crate) fn available(path: &std::path::Path) -> Option<u64> {
    cobble_platform::available_disk_space(path)
}
async fn confirm(app: &AppHandle, auth: &Auth, moving: bool) -> Result<bool, String> {
    let zh = auth.locale == "zh-CN";
    let title = if moving {
        if zh { "迁移游戏" } else { "Move game" }
    } else if zh {
        "清理缓存"
    } else {
        "Clear cache"
    };
    let details = if moving {
        if zh {
            "复制并核验后切换游戏位置。源副本会保留。"
        } else {
            "Copy and verify the game, then switch its location. The original copy is kept."
        }
    } else if zh {
        "仅删除可重新获取的公开资料缓存。游戏、模组和存档不会删除。"
    } else {
        "Clear downloadable public metadata only. Games, Mods and worlds are kept."
    };
    crate::plugins::plugin_confirm::ask(app, auth, title, details).await
}
pub async fn overview(app: AppHandle, auth: &Auth, args: OverviewRequest) -> Result<Value, String> {
    read_permission(auth)?;
    if let Some(id) = &args.game_id {
        scoped(auth, id)?;
    }
    let requested = args.game_id.or_else(|| auth.instance_id.clone());
    let selected = requested.clone();
    let games = app
        .state::<GameLibrary>()
        .0
        .lock()
        .map_err(|_| "STORAGE_STATE_UNAVAILABLE")?
        .games
        .iter()
        .filter(|game| requested.as_ref().is_none_or(|id| &game.id == id))
        .cloned()
        .collect::<Vec<_>>();
    let rows = tauri::async_runtime::spawn_blocking(move || games.into_iter().map(|game| {
        let instance = disk::size(&game.directory).ok();
        let backups = snapshot_root(&game.id).ok().and_then(|path| match path.try_exists() { Ok(true) => disk::size(&path).ok(), Ok(false) => Some(0), Err(_) => None });
        json!({"gameId":game.id,"name":game.name,"instanceBytes":instance,"backupBytes":backups,"activity":if game.content_installing { "busy" } else { game.launch_state.as_str() },"movable":game.status=="installed" && game.launch_state=="idle" && !game.content_installing})
    }).collect::<Vec<_>>()).await.map_err(|_| "STORAGE_SCAN_FAILED")?;
    let cache = cobble_net::PublicMetadataCache::shared()
        .ok_or("STORAGE_CACHE_UNAVAILABLE")?
        .stats()
        .await;
    let mut retained = Vec::new();
    if let Ok(root) = receipt_root()
        && let Ok(entries) = fs::read_dir(root)
    {
        for entry in entries.take(256).flatten() {
            let Some(id) = entry
                .path()
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_owned)
            else {
                continue;
            };
            if let Ok(receipt) = read_receipt(&id)
                && receipt.plugin_id == auth.manifest.id
                && selected
                    .as_ref()
                    .is_none_or(|game| game == &receipt.game_id)
                && receipt.source.exists()
                && game(&app, &receipt.game_id).is_ok_and(|game| game.directory == receipt.target)
            {
                retained.push(json!({"taskId":id,"gameId":receipt.game_id,"bytes":receipt.inventory.bytes,"sourceLabel":receipt.source.file_name().unwrap_or_default().to_string_lossy(),"canRecycle":cfg!(windows)}));
            }
        }
    }
    Ok(
        json!({"games":rows,"retained":retained,"cache":{"bytes":cache.disk_bytes,"reclaimableBytes":cache.disk_bytes}}),
    )
}
pub async fn clean_cache(
    app: AppHandle,
    auth: &Auth,
    _args: EmptyRequest,
) -> Result<Value, String> {
    action_permission(auth, false)?;
    if !confirm(&app, auth, false).await? {
        return Ok(json!({"cleared":false,"reclaimedBytes":0}));
    }
    read_permission(auth)?;
    let cache = cobble_net::PublicMetadataCache::shared().ok_or("STORAGE_CACHE_UNAVAILABLE")?;
    let before = cache.stats().await.disk_bytes;
    cache.clear().await;
    let after = cache.stats().await.disk_bytes;
    Ok(json!({"cleared":true,"reclaimedBytes":before.saturating_sub(after)}))
}
pub async fn pick_destination(
    app: AppHandle,
    auth: &Auth,
    args: GameRequest,
) -> Result<Value, String> {
    action_permission(auth, true)?;
    scoped(auth, &args.game_id)?;
    let game = game(&app, &args.game_id)?;
    if game.status != "installed" || game.launch_state != "idle" || game.content_installing {
        return Err("STORAGE_INSTANCE_BUSY".into());
    }
    let mut dialog = rfd::AsyncFileDialog::new().set_title(if auth.locale == "zh-CN" {
        "选择游戏迁移位置"
    } else {
        "Choose destination folder"
    });
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(folder) = dialog.pick_folder().await else {
        return Ok(Value::Null);
    };
    plan_for_parent(auth, game, folder.path().to_owned()).await
}
async fn plan_for_parent(auth: &Auth, game: Game, parent: PathBuf) -> Result<Value, String> {
    read_permission(auth)?;
    let parent = dunce::canonicalize(parent).map_err(safe_error)?;
    let target = parent.join(&game.id);
    if target.exists() {
        return Err("STORAGE_TARGET_EXISTS".into());
    }
    let source = game.directory.clone();
    let scan = source.clone();
    let cancel = auth.cancel.clone();
    let inventory = tauri::async_runtime::spawn_blocking(move || {
        disk::inventory(&scan, &cancel).map_err(safe_error)
    })
    .await
    .map_err(|_| "STORAGE_SCAN_FAILED")??;
    let plan_id = nonce();
    let free = available(&parent);
    let required = inventory.bytes;
    let label = parent
        .file_name()
        .unwrap_or(parent.as_os_str())
        .to_string_lossy()
        .into_owned();
    let mut state = state().lock().map_err(|_| "STORAGE_STATE_UNAVAILABLE")?;
    state
        .plans
        .retain(|_, plan| plan.created.elapsed() < Duration::from_secs(1800));
    if state.plans.len() >= 32 {
        return Err("STORAGE_BUSY".into());
    }
    state.plans.insert(
        plan_id.clone(),
        Plan {
            owner: auth.session_id.clone(),
            game_id: game.id,
            source,
            target,
            inventory,
            created: Instant::now(),
        },
    );
    Ok(
        json!({"planId":plan_id,"targetLabel":label,"requiredBytes":required,"availableBytes":free,"enoughSpace":free.map(|bytes|bytes>=required),"sourceRetained":true}),
    )
}
fn update(id: &str, change: impl FnOnce(&mut Value)) {
    if let Ok(mut state) = state().lock()
        && let Some(task) = state.tasks.get_mut(id)
    {
        change(&mut task.value);
    }
}
fn publish(app: &AppHandle, plan: &Plan, cancelled: &AtomicBool) -> Result<(), String> {
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|_| "STORAGE_STATE_UNAVAILABLE")?;
    let index = library
        .games
        .iter()
        .position(|game| game.id == plan.game_id)
        .ok_or("PLUGIN_GAME_SCOPE")?;
    if library.games[index].directory != plan.source
        || library.games[index].launch_state != "idle"
        || cancelled.load(Ordering::Acquire)
    {
        return Err("STORAGE_INSTANCE_CHANGED".into());
    }
    // Persist a complete replacement file before changing the in-memory directory.
    let mut games = library.games.clone();
    games[index].directory = plan.target.clone();
    games[index].log_path = None;
    let path = library_file()?;
    let temporary = path.with_file_name(format!("games-move-{}.tmp", nonce()));
    let bytes = serde_json::to_vec_pretty(&games).map_err(|_| "STORAGE_INDEX_FAILED")?;
    let result = (|| {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("STORAGE_INDEX_FAILED".into());
    }
    library.games[index].directory = plan.target.clone();
    library.games[index].log_path = None;
    let game = library.games[index].clone();
    drop(library);
    let _ = app.emit("game-install", game);
    // Existing desktop shortcuts resolve the stable game ID through this library.
    Ok(())
}
pub async fn start_move(app: AppHandle, auth: &Auth, args: PlanRequest) -> Result<Value, String> {
    action_permission(auth, true)?;
    let selected = {
        let state = state().lock().map_err(|_| "STORAGE_STATE_UNAVAILABLE")?;
        let plan = state
            .plans
            .get(&args.plan_id)
            .ok_or("STORAGE_PLAN_EXPIRED")?;
        if plan.owner != auth.session_id {
            return Err("PLUGIN_VIEW_CALLER_DENIED".into());
        }
        scoped(auth, &plan.game_id)?;
        plan.clone()
    };
    let zh = auth.locale == "zh-CN";
    let details = if zh {
        format!(
            "源目录：{}\n目标目录：{}\n需要复制：{} 字节\n核验后切换位置，旧副本保留。",
            selected.source.display(),
            selected.target.display(),
            selected.inventory.bytes
        )
    } else {
        format!(
            "From: {}\nTo: {}\nCopy: {} bytes\nSwitch location after verification. Keep the original copy.",
            selected.source.display(),
            selected.target.display(),
            selected.inventory.bytes
        )
    };
    if !crate::plugins::plugin_confirm::ask(
        &app,
        auth,
        if zh { "迁移游戏" } else { "Move game" },
        &details,
    )
    .await?
    {
        return Ok(json!({"taskId":null}));
    }
    read_permission(auth)?;
    let plan = {
        let mut state = state().lock().map_err(|_| "STORAGE_STATE_UNAVAILABLE")?;
        let plan = state
            .plans
            .get(&args.plan_id)
            .ok_or("STORAGE_PLAN_EXPIRED")?;
        if plan.owner != auth.session_id {
            return Err("PLUGIN_VIEW_CALLER_DENIED".into());
        }
        scoped(auth, &plan.game_id)?;
        if plan.created.elapsed() >= Duration::from_secs(1800) {
            return Err("STORAGE_PLAN_EXPIRED".into());
        }
        state
            .plans
            .remove(&args.plan_id)
            .ok_or("STORAGE_PLAN_EXPIRED")?
    };
    if available(plan.target.parent().ok_or("STORAGE_TARGET_INVALID")?)
        .is_some_and(|free| free < plan.inventory.bytes)
    {
        return Err("STORAGE_SPACE_LOW".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let task_id = tasks::register_external(
        &app,
        &plan.game_id,
        "迁移游戏",
        "storage_move",
        cancel.clone(),
    )?;
    {
        let mut state = state().lock().map_err(|_| "STORAGE_STATE_UNAVAILABLE")?;
        if state.tasks.len() >= 64 {
            state
                .tasks
                .retain(|_, task| task.value["status"] == "running");
        }
        if state.tasks.len() >= 64 {
            tasks::finish_external(&app, &task_id, Err("STORAGE_BUSY".into()));
            return Err("STORAGE_BUSY".into());
        }
        state.tasks.insert(task_id.clone(), Task { owner: auth.session_id.clone(), cancel: cancel.clone(), value: json!({"taskId":task_id,"status":"running","phase":"copying","completedBytes":0,"totalBytes":plan.inventory.bytes,"error":null,"sourceRetained":true}) });
    }
    let id = task_id.clone();
    let game_id = plan.game_id.clone();
    let plugin_id = auth.manifest.id.clone();
    tauri::async_runtime::spawn(async move {
        let result = async {
            tasks::wait_external_turn(&app, &id).await?;
            if plan.created.elapsed() >= Duration::from_secs(1800) {
                return Err("STORAGE_PLAN_EXPIRED".into());
            }
            let (current, operation) = management::begin(&app, &plan.game_id, true)?;
            if current.directory != plan.source || sessions::recover(&current)?.is_some() {
                return Err("STORAGE_INSTANCE_CHANGED".into());
            }
            let world_locks = workspace::world_locks(&plan.source)?;
            let worker_app = app.clone();
            let worker_id = id.clone();
            let worker_cancel = cancel.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let _operation = operation;
                let _world_locks = world_locks;
                let plan = Arc::new(plan);
                disk::copy_verified(
                    &plan.source,
                    &plan.target,
                    &plan.inventory,
                    &worker_cancel,
                    |bytes| {
                        update(&worker_id, |task| task["completedBytes"] = bytes.into());
                        tasks::external_progress(
                            &worker_app,
                            &worker_id,
                            "copying",
                            bytes as usize,
                            plan.inventory.bytes as usize,
                        );
                    },
                )
                .map_err(safe_error)
                .and_then(|_| {
                    update(&worker_id, |task| task["phase"] = "publishing".into());
                    record_receipt(&worker_id, &plugin_id, &plan)?;
                    publish(&worker_app, &plan, &worker_cancel)
                })
            })
            .await
            .map_err(|_| "STORAGE_IO_FAILED".to_string())?
        }
        .await;
        update(&id, |task| {
            task["status"] = if result.is_ok() {
                "succeeded"
            } else if cancel.load(Ordering::Acquire) {
                "cancelled"
            } else {
                "failed"
            }
            .into();
            task["phase"] = "complete".into();
            task["error"] = result
                .as_ref()
                .err()
                .cloned()
                .map(Value::String)
                .unwrap_or(Value::Null);
        });
        tasks::finish_external(&app, &id, result.map(|_| Some(game_id)));
    });
    Ok(json!({"taskId":task_id}))
}
pub async fn task(_app: AppHandle, auth: &Auth, args: TaskRequest) -> Result<Value, String> {
    action_permission(auth, true)?;
    let state = state().lock().map_err(|_| "STORAGE_STATE_UNAVAILABLE")?;
    let task = state
        .tasks
        .get(&args.task_id)
        .ok_or("STORAGE_TASK_UNKNOWN")?;
    if task.owner != auth.session_id {
        return Err("PLUGIN_VIEW_CALLER_DENIED".into());
    }
    Ok(task.value.clone())
}
pub async fn cancel(app: AppHandle, auth: &Auth, args: TaskRequest) -> Result<Value, String> {
    action_permission(auth, true)?;
    let state = state().lock().map_err(|_| "STORAGE_STATE_UNAVAILABLE")?;
    let task = state
        .tasks
        .get(&args.task_id)
        .ok_or("STORAGE_TASK_UNKNOWN")?;
    if task.owner != auth.session_id {
        return Err("PLUGIN_VIEW_CALLER_DENIED".into());
    }
    if task.value["status"] == "running" && task.value["phase"] != "publishing" {
        task.cancel.store(true, Ordering::Release);
        let _ = tasks::cancel_task(app, args.task_id);
    }
    Ok(Value::Null)
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    id: String,
    plugin_id: String,
    game_id: String,
    source: PathBuf,
    target: PathBuf,
    inventory: disk::Inventory,
}
fn receipt_root() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("STORAGE_STATE_UNAVAILABLE")?
        .join("CobbleLauncher/storage-moves"))
}
fn receipt_path(id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("STORAGE_RECEIPT_INVALID".into());
    }
    Ok(receipt_root()?.join(format!("{id}.json")))
}
fn record_receipt(id: &str, plugin_id: &str, plan: &Plan) -> Result<(), String> {
    let path = receipt_path(id)?;
    fs::create_dir_all(path.parent().ok_or("STORAGE_RECEIPT_INVALID")?)
        .map_err(|_| "STORAGE_RECEIPT_FAILED")?;
    let receipt = Receipt {
        id: id.into(),
        plugin_id: plugin_id.into(),
        game_id: plan.game_id.clone(),
        source: plan.source.clone(),
        target: plan.target.clone(),
        inventory: plan.inventory.clone(),
    };
    let bytes = serde_json::to_vec(&receipt).map_err(|_| "STORAGE_RECEIPT_FAILED")?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("STORAGE_RECEIPT_FAILED".into());
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "STORAGE_RECEIPT_FAILED")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "STORAGE_RECEIPT_FAILED".into())
}
fn read_receipt(id: &str) -> Result<Receipt, String> {
    let path = receipt_path(id)?;
    let metadata = fs::symlink_metadata(&path).map_err(|_| "STORAGE_RECEIPT_UNKNOWN")?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 32 * 1024 * 1024
    {
        return Err("STORAGE_RECEIPT_INVALID".into());
    }
    let receipt: Receipt =
        serde_json::from_slice(&fs::read(path).map_err(|_| "STORAGE_RECEIPT_FAILED")?)
            .map_err(|_| "STORAGE_RECEIPT_INVALID")?;
    if receipt.id != id {
        return Err("STORAGE_RECEIPT_INVALID".into());
    }
    Ok(receipt)
}
pub async fn release_source(
    app: AppHandle,
    auth: &Auth,
    args: TaskRequest,
) -> Result<Value, String> {
    action_permission(auth, true)?;
    let receipt = read_receipt(&args.task_id)?;
    if receipt.plugin_id != auth.manifest.id {
        return Err("PLUGIN_VIEW_CALLER_DENIED".into());
    }
    scoped(auth, &receipt.game_id)?;
    let (current, operation) = management::begin(&app, &receipt.game_id, true)?;
    if current.directory != receipt.target || sessions::recover(&current)?.is_some() {
        return Err("STORAGE_INSTANCE_CHANGED".into());
    }
    let zh = auth.locale == "zh-CN";
    let description = if zh {
        format!(
            "将此旧副本移入回收站：\n{}\n游戏现在使用迁移后的目录。",
            receipt.source.display()
        )
    } else {
        format!(
            "Move this original copy to the Recycle Bin:\n{}\nThe game now uses its new location.",
            receipt.source.display()
        )
    };
    let title = if zh {
        "删除旧副本"
    } else {
        "Remove original copy"
    };
    if !crate::plugins::plugin_confirm::ask(&app, auth, title, &description).await? {
        return Ok(json!({"released":false,"recycledBytes":0}));
    }
    read_permission(auth)?;
    let cancelled = auth.cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        let _world_locks = workspace::world_locks(&receipt.source)?;
        let source = dunce::canonicalize(&receipt.source).map_err(safe_error)?;
        let target = dunce::canonicalize(&receipt.target).map_err(safe_error)?;
        if source == target || source.starts_with(&target) || target.starts_with(&source) {
            return Err("STORAGE_TARGET_OVERLAP".into());
        }
        let library = app.state::<GameLibrary>();
        let library = library.0.lock().map_err(|_| "STORAGE_STATE_UNAVAILABLE")?;
        for game in &library.games {
            let path = dunce::canonicalize(&game.directory).map_err(safe_error)?;
            if path.starts_with(&source) || source.starts_with(&path) {
                return Err("STORAGE_SOURCE_IN_USE".into());
            }
        }
        if disk::inventory(&receipt.source, &cancelled).map_err(safe_error)? != receipt.inventory {
            return Err("STORAGE_SOURCE_CHANGED".into());
        }
        // Hold library mutex through recycle so no new registration can reference the old copy.
        cobble_platform::recycle_directory(&source)?;
        drop(library);
        let _ = fs::remove_file(receipt_path(&receipt.id)?);
        Ok(json!({"released":true,"recycledBytes":receipt.inventory.bytes}))
    })
    .await
    .map_err(|_| "STORAGE_RECYCLE_FAILED")?
}
pub async fn open_source(_app: AppHandle, auth: &Auth, args: TaskRequest) -> Result<Value, String> {
    action_permission(auth, true)?;
    let receipt = read_receipt(&args.task_id)?;
    if receipt.plugin_id != auth.manifest.id {
        return Err("PLUGIN_VIEW_CALLER_DENIED".into());
    }
    scoped(auth, &receipt.game_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        open::that(receipt.source)
            .map(|_| Value::Null)
            .map_err(|_| "STORAGE_OPEN_FAILED".into())
    })
    .await
    .map_err(|_| "STORAGE_OPEN_FAILED")?
}

/// Test-only folder selection for the existing isolated native adapter. Never registered as IPC.
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub async fn isolated_plan(
    app: AppHandle,
    auth: &Auth,
    parent: PathBuf,
    game_id: String,
) -> Result<Value, String> {
    action_permission(auth, true)?;
    scoped(auth, &game_id)?;
    let game = game(&app, &game_id)?;
    if game.status != "installed" || game.launch_state != "idle" || game.content_installing {
        return Err("STORAGE_INSTANCE_BUSY".into());
    }
    fn fixture_root(path: &std::path::Path) -> Option<PathBuf> {
        let path = dunce::canonicalize(path).ok()?;
        let temporary = dunce::canonicalize(std::env::temp_dir()).ok()?;
        if !path.starts_with(temporary) {
            return None;
        }
        path.ancestors()
            .find(|ancestor| {
                ancestor
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("cobble-plugin-webview-probe-"))
            })
            .map(std::path::Path::to_path_buf)
    }
    let source_root = fixture_root(&game.directory).ok_or("PROBE_ROOT_REQUIRED")?;
    if fixture_root(&parent).as_ref() != Some(&source_root) {
        return Err("PROBE_ROOT_REQUIRED".into());
    }
    plan_for_parent(auth, game, parent).await
}
