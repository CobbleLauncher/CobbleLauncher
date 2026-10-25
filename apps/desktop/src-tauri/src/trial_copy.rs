// SPDX-License-Identifier: MIT
//! Host-owned trial-copy workers. Paths never cross the plugin boundary.
use super::*;
use cobble_core::storage_workspace as disk;
use cobble_plugin_api::{InspectionScope, PermissionNode};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
type Auth = crate::plugins::plugin_views::ViewAuthorization;
#[path = "trial_experiments.rs"]
pub mod experiments;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRequest {
    pub game_id: String,
    pub name: String,
    #[serde(default)]
    pub include_worlds: bool,
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelRequest {
    pub task_id: Option<String>,
}
#[derive(Clone)]
struct Plan {
    owner: String,
    session: String,
    game: workspace::GameSnapshot,
    name: String,
    include_worlds: bool,
    target: PathBuf,
    new_id: String,
    inventory: disk::Inventory,
    disabled_mods: Vec<String>,
    created: Instant,
}
struct Job {
    owner: String,
    session: String,
    created: Instant,
    cancel: Arc<AtomicBool>,
    value: Mutex<Value>,
    finished: AtomicBool,
    done: tokio::sync::Notify,
}
#[derive(Default)]
struct Store {
    plans: HashMap<String, Plan>,
    active: HashMap<String, Arc<AtomicBool>>,
    jobs: HashMap<String, Arc<Job>>,
}
static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
fn store() -> &'static Mutex<Store> {
    STORE.get_or_init(|| Mutex::new(Store::default()))
}
struct Active {
    session: String,
    cancel: Arc<AtomicBool>,
}
impl Drop for Active {
    fn drop(&mut self) {
        if let Ok(mut s) = store().lock()
            && s.active
                .get(&self.session)
                .is_some_and(|c| Arc::ptr_eq(c, &self.cancel))
        {
            s.active.remove(&self.session);
        }
        crate::plugins::plugin_views::clear_job(&self.session, &self.cancel)
    }
}
fn active(auth: &Auth) -> Result<Active, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut s = store().lock().map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
        if s.active.contains_key(&auth.session_id) || s.active.len() >= 2 {
            return Err("TRIAL_COPY_BUSY".into());
        }
        s.active.insert(auth.session_id.clone(), cancel.clone());
    }
    let active = Active {
        session: auth.session_id.clone(),
        cancel,
    };
    crate::plugins::plugin_views::register_job(&auth.session_id, active.cancel.clone())?;
    Ok(active)
}
fn nonce() -> Result<String, String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
    Ok(b.iter().map(|b| format!("{b:02x}")).collect())
}
fn authorize(auth: &Auth, id: Option<&str>) -> Result<(), String> {
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    if id.is_some_and(|id| auth.instance_id.as_deref().is_some_and(|bound| bound != id)) {
        return Err("PLUGIN_GAME_SCOPE".into());
    }
    let p = auth.manifest.permissions.as_deref().unwrap_or_default();
    if !p.contains(&PermissionNode::InstanceCopyRequest{})||!p.iter().any(|n|matches!(n,PermissionNode::GameInspectionRead{scopes}if scopes.contains(&InspectionScope::Storage))){return Err("PLUGIN_PERMISSION_DENIED".into())}
    Ok(())
}
fn valid_input(args: &PreviewRequest) -> Result<(), String> {
    if args.game_id.is_empty()
        || args.game_id.len() > 128
        || !args
            .game_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        || args.name.trim().is_empty()
        || args.name.chars().count() > 80
        || args.name.chars().any(char::is_control)
    {
        return Err("TRIAL_COPY_REQUEST_INVALID".into());
    }
    Ok(())
}
fn safe_error(e: impl std::fmt::Display) -> String {
    let value = e.to_string();
    for (code, out) in [
        ("STORAGE_CANCELLED", "TRIAL_COPY_CANCELLED"),
        ("STORAGE_SOURCE_CHANGED", "TRIAL_COPY_STALE"),
        ("STORAGE_LINK_DENIED", "TRIAL_COPY_LINK_DENIED"),
        ("STORAGE_TARGET_OVERLAP", "TRIAL_COPY_TARGET_INVALID"),
        ("STORAGE_SCAN_LIMIT", "TRIAL_COPY_SCAN_LIMIT"),
    ] {
        if value.contains(code) {
            return out.into();
        }
    }
    "TRIAL_COPY_IO_FAILED".into()
}
fn nearest(path: &std::path::Path) -> Option<&std::path::Path> {
    path.ancestors().find(|p| p.exists())
}
fn preview_value(id: &str, p: &Plan) -> Value {
    let required = p.inventory.bytes.saturating_add(16 * 1024 * 1024);
    let free = p
        .target
        .parent()
        .and_then(nearest)
        .and_then(cobble_platform::available_disk_space);
    json!({"planId":id,"game":{"id":p.game.id,"name":p.game.name,"version":p.game.version},"name":p.name,"includeWorlds":p.include_worlds,"fileCount":p.inventory.files.len(),"totalBytes":p.inventory.bytes,"requiredBytes":required,"availableBytes":free,"enoughSpace":free.map(|n|n>=required),"destinationLabel":p.target.parent().and_then(|p|p.file_name()).map(|s|s.to_string_lossy().into_owned()).unwrap_or_else(||"Games".into())})
}
async fn inspect(
    app: AppHandle,
    auth: &Auth,
    args: PreviewRequest,
    parent: PathBuf,
) -> Result<Value, String> {
    authorize(auth, Some(&args.game_id))?;
    valid_input(&args)?;
    let active = active(auth)?;
    let auth = auth.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (game, _lease) = workspace::acquire(&app, &args.game_id)?;
        let _worlds = workspace::world_locks(&game.directory)?;
        let inventory =
            disk::inventory_instance(&game.directory, args.include_worlds, &active.cancel)
                .map_err(safe_error)?;
        if active.cancel.load(Ordering::Acquire) || auth.cancel.load(Ordering::Acquire) {
            return Err("TRIAL_COPY_CANCELLED".into());
        }
        let new_id = nonce()?;
        let target = parent.join(&new_id);
        let source = fs::canonicalize(&game.directory).map_err(|_| "TRIAL_COPY_IO_FAILED")?;
        if target.starts_with(&source) || source.starts_with(&target) {
            return Err("TRIAL_COPY_TARGET_INVALID".into());
        }
        let plan = Plan {
            owner: auth.manifest.id.clone(),
            session: auth.session_id.clone(),
            game,
            name: args.name.trim().into(),
            include_worlds: args.include_worlds,
            target,
            new_id,
            inventory,
            disabled_mods: vec![],
            created: Instant::now(),
        };
        let id = nonce()?;
        let result = preview_value(&id, &plan);
        let mut s = store().lock().map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
        s.plans.retain(|_, p| {
            p.created.elapsed() < Duration::from_secs(300) && p.session != auth.session_id
        });
        if s.plans.len() >= 32 {
            return Err("TRIAL_COPY_BUSY".into());
        }
        s.plans.insert(id, plan);
        drop(s);
        drop(active);
        Ok(result)
    })
    .await
    .map_err(|_| "TRIAL_COPY_IO_FAILED")?
}
/// Test-only parent selection for an owned synthetic source; never exposed as IPC.
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub async fn isolated_preview(
    app: AppHandle,
    auth: &Auth,
    args: PreviewRequest,
    parent: PathBuf,
) -> Result<Value, String> {
    let parent = dunce::canonicalize(parent).map_err(|_| "PROBE_SCOPE_INVALID")?;
    let source = dunce::canonicalize(workspace::snapshot(&app, &args.game_id)?.directory)
        .map_err(|_| "PROBE_SCOPE_INVALID")?;
    let temporary = dunce::canonicalize(std::env::temp_dir()).map_err(|_| "PROBE_SCOPE_INVALID")?;
    let root = parent
        .ancestors()
        .find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with("cobble-plugin-webview-probe-"))
        })
        .ok_or("PROBE_SCOPE_INVALID")?;
    if !root.starts_with(&temporary) || !source.starts_with(root) {
        return Err("PROBE_SCOPE_INVALID".into());
    }
    inspect(app, auth, args, parent).await
}
pub async fn preview(app: AppHandle, auth: &Auth, args: PreviewRequest) -> Result<Value, String> {
    let parent = default_directory().map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
    inspect(app, auth, args, parent).await
}
pub async fn pick_destination(
    app: AppHandle,
    auth: &Auth,
    args: PreviewRequest,
) -> Result<Value, String> {
    authorize(auth, Some(&args.game_id))?;
    valid_input(&args)?;
    let window = app
        .get_webview_window("main")
        .ok_or("TRIAL_COPY_UNAVAILABLE")?;
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title(if auth.locale == "zh-CN" {
            "选择副本位置"
        } else {
            "Choose copy location"
        })
        .pick_folder()
        .await
    else {
        return Ok(Value::Null);
    };
    let parent = fs::canonicalize(folder.path()).map_err(|_| "TRIAL_COPY_TARGET_INVALID")?;
    inspect(app, auth, args, parent).await
}
fn plan(auth: &Auth, id: &str) -> Result<Plan, String> {
    let s = store().lock().map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
    let p = s
        .plans
        .get(id)
        .filter(|p| {
            p.owner == auth.manifest.id
                && p.session == auth.session_id
                && p.created.elapsed() < Duration::from_secs(300)
        })
        .ok_or("TRIAL_COPY_PLAN_EXPIRED")?
        .clone();
    authorize(auth, Some(&p.game.id))?;
    Ok(p)
}
fn job(auth: &Auth, id: &str) -> Result<Arc<Job>, String> {
    authorize(auth, None)?;
    store()
        .lock()
        .map_err(|_| "TRIAL_COPY_UNAVAILABLE")?
        .jobs
        .get(id)
        .filter(|j| j.owner == auth.manifest.id && j.session == auth.session_id)
        .cloned()
        .ok_or_else(|| "TRIAL_COPY_TASK_NOT_FOUND".into())
}
fn publish(app: &AppHandle, p: &Plan) -> Result<Value, String> {
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|_| "TRIAL_COPY_LIBRARY_FAILED")?;
    if library
        .games
        .iter()
        .any(|g| g.id == p.new_id || g.directory == p.target)
    {
        return Err("TRIAL_COPY_LIBRARY_FAILED".into());
    }
    let mut game = library
        .games
        .iter()
        .find(|g| g.id == p.game.id && g.directory == p.game.directory)
        .cloned()
        .ok_or("TRIAL_COPY_STALE")?;
    game.id = p.new_id.clone();
    game.name = p.name.clone();
    game.directory = p.target.clone();
    game.content_installing = false;
    game.progress = None;
    game.error = None;
    game.launch_state = idle();
    game.launch_error = None;
    game.log_path = None;
    game.last_launched_at = None;
    let mut updated = library.games.clone();
    updated.push(game.clone());
    let path = library_file().map_err(|_| "TRIAL_COPY_LIBRARY_FAILED")?;
    let parent = path.parent().ok_or("TRIAL_COPY_LIBRARY_FAILED")?;
    fs::create_dir_all(parent).map_err(|_| "TRIAL_COPY_LIBRARY_FAILED")?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "TRIAL_COPY_LIBRARY_FAILED")?;
    use std::io::Write;
    temporary
        .write_all(&serde_json::to_vec_pretty(&updated).map_err(|_| "TRIAL_COPY_LIBRARY_FAILED")?)
        .map_err(|_| "TRIAL_COPY_LIBRARY_FAILED")?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "TRIAL_COPY_LIBRARY_FAILED")?;
    temporary
        .persist(path)
        .map_err(|_| "TRIAL_COPY_LIBRARY_FAILED")?;
    library.games = updated;
    drop(library);
    let _ = app.emit("game-install", &game);
    Ok(json!({"id":game.id,"name":game.name,"version":game.version}))
}
pub async fn start(app: AppHandle, auth: &Auth, args: PlanRequest) -> Result<Value, String> {
    let p = plan(auth, &args.plan_id)?;
    let active = active(auth)?;
    let mut details = if auth.locale == "zh-CN" {
        format!(
            "源：{}\n副本：{}\n位置：{}\n存档：{}\n所需空间：{} 字节",
            p.game.name,
            p.name,
            p.target.display(),
            if p.include_worlds {
                "复制"
            } else {
                "不复制"
            },
            p.inventory.bytes.saturating_add(16 * 1024 * 1024)
        )
    } else {
        format!(
            "Source: {}\nCopy: {}\nLocation: {}\nWorlds: {}\nRequired space: {} bytes",
            p.game.name,
            p.name,
            p.target.display(),
            if p.include_worlds {
                "included"
            } else {
                "excluded"
            },
            p.inventory.bytes.saturating_add(16 * 1024 * 1024)
        )
    };
    if !p.disabled_mods.is_empty() {
        details.push_str(&format!("\n{}: {}", if auth.locale == "zh-CN" { "仅在副本禁用" } else { "Disable in copy only" }, p.disabled_mods.join(", ")));
    }
    if !crate::plugins::plugin_confirm::ask(
        &app,
        auth,
        if auth.locale == "zh-CN" {
            "创建试更新副本"
        } else {
            "Create trial copy"
        },
        &details,
    )
    .await?
    {
        return Err("PLUGIN_ACTION_CANCELLED".into());
    }
    if active.cancel.load(Ordering::Acquire) {
        return Err("TRIAL_COPY_CANCELLED".into());
    }
    drop(active);
    let cancel = Arc::new(AtomicBool::new(false));
    let id = tasks::register_external(
        &app,
        &p.game.id,
        "创建试更新副本",
        "trial_copy",
        cancel.clone(),
    )?;
    let task = Arc::new(Job {
        owner: auth.manifest.id.clone(),
        session: auth.session_id.clone(),
        created: Instant::now(),
        cancel: cancel.clone(),
        value: Mutex::new(
            json!({"taskId":id,"status":"running","completedBytes":0,"totalBytes":p.inventory.bytes,"error":null,"game":null}),
        ),
        finished: AtomicBool::new(false),
        done: tokio::sync::Notify::new(),
    });
    {
        let mut s = store().lock().map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
        s.plans.remove(&args.plan_id);
        while s.jobs.len() >= 32 {
            let oldest = s
                .jobs
                .iter()
                .filter(|(_, j)| j.finished.load(Ordering::Acquire))
                .min_by_key(|(_, j)| j.created)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                s.jobs.remove(&oldest);
            } else {
                tasks::finish_external(&app, &id, Err("TRIAL_COPY_BUSY".into()));
                return Err("TRIAL_COPY_BUSY".into());
            }
        }
        s.jobs.insert(id.clone(), task.clone());
    }
    let worker = task.clone();
    let return_id = id.clone();
    tauri::async_runtime::spawn(async move {
        let work = worker.clone();
        let worker_app = app.clone();
        let worker_id = id.clone();
        let result = async {
            tasks::wait_external_turn(&app, &id).await?;
            if p.created.elapsed() >= Duration::from_secs(300) {
                return Err("TRIAL_COPY_PLAN_EXPIRED".into());
            }
            let (current, lease) = workspace::acquire(&app, &p.game.id)?;
            if current.directory != p.game.directory || current.version != p.game.version {
                return Err("TRIAL_COPY_STALE".into());
            }
            let worlds = workspace::world_locks(&current.directory)?;
            tauri::async_runtime::spawn_blocking(move || {
                let _lease = lease;
                let _worlds = worlds;
                let parent = p.target.parent().ok_or("TRIAL_COPY_TARGET_INVALID")?;
                fs::create_dir_all(parent).map_err(|_| "TRIAL_COPY_IO_FAILED")?;
                if cobble_platform::available_disk_space(parent)
                    .is_some_and(|n| n < p.inventory.bytes.saturating_add(16 * 1024 * 1024))
                {
                    return Err("TRIAL_COPY_NO_SPACE".into());
                }
                disk::copy_instance_verified(
                    &p.game.directory,
                    &p.target,
                    &p.inventory,
                    p.include_worlds,
                    &work.cancel,
                    |bytes| {
                        if let Ok(mut value) = work.value.lock() {
                            value["completedBytes"] = json!(bytes)
                        }
                        tasks::external_progress(
                            &worker_app,
                            &worker_id,
                            "copying",
                            bytes as usize,
                            p.inventory.bytes as usize,
                        );
                    },
                )
                .map_err(safe_error)?;
                if work.cancel.load(Ordering::Acquire) {
                    return Err("TRIAL_COPY_CANCELLED".into());
                }
                experiments::apply(&p)?;
                experiments::record(&p)?;
                publish(&worker_app, &p)
            })
            .await
            .map_err(|_| "TRIAL_COPY_IO_FAILED".to_string())?
        }
        .await;
        let task_result = result
            .as_ref()
            .map(|game| game["id"].as_str().map(str::to_owned))
            .map_err(Clone::clone);
        tasks::finish_external(&app, &id, task_result);
        {
            let mut value = worker.value.lock().unwrap_or_else(|e| e.into_inner());
            match result {
                Ok(game) => {
                    value["status"] = json!("succeeded");
                    value["game"] = game;
                }
                Err(code) => {
                    value["status"] = json!(if code == "TRIAL_COPY_CANCELLED" {
                        "cancelled"
                    } else {
                        "failed"
                    });
                    value["error"] = json!(code);
                }
            }
            worker.finished.store(true, Ordering::Release);
        }
        worker.done.notify_waiters();
    });
    Ok(json!({"taskId":return_id}))
}
pub async fn task(_app: AppHandle, auth: &Auth, args: TaskRequest) -> Result<Value, String> {
    let j = job(auth, &args.task_id)?;
    let value = j
        .value
        .lock()
        .map_err(|_| "TRIAL_COPY_UNAVAILABLE")?
        .clone();
    Ok(value)
}
pub async fn cancel(app: AppHandle, auth: &Auth, args: CancelRequest) -> Result<Value, String> {
    authorize(auth, None)?;
    if let Some(id) = args.task_id {
        let j = job(auth, &id)?;
        {
            let mut v = j.value.lock().map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
            if !j.finished.load(Ordering::Acquire) {
                j.cancel.store(true, Ordering::Release);
                v["status"] = json!("cancelling")
            }
        }
        let _ = tasks::cancel_task(app, id);
        loop {
            let notified = j.done.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if j.finished.load(Ordering::Acquire) {
                break;
            }
            notified.await;
        }
    } else if let Some(c) = store()
        .lock()
        .map_err(|_| "TRIAL_COPY_UNAVAILABLE")?
        .active
        .get(&auth.session_id)
    {
        c.store(true, Ordering::Release)
    }
    Ok(Value::Null)
}
pub async fn open_game(app: AppHandle, auth: &Auth, args: TaskRequest) -> Result<Value, String> {
    let j = job(auth, &args.task_id)?;
    let value = j.value.lock().map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
    if value["status"] != "succeeded" {
        return Err("TRIAL_COPY_NOT_COMPLETE".into());
    }
    let id = value["game"]["id"]
        .as_str()
        .ok_or("TRIAL_COPY_NOT_COMPLETE")?;
    workspace::snapshot(&app, id)?;
    app.emit_to("main", "plugin-game-open-request", json!({"gameId":id}))
        .map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
    Ok(json!({"requested":true}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trial_input_defaults_to_excluding_worlds_and_rejects_paths() {
        let v: PreviewRequest =
            serde_json::from_value(json!({"gameId":"game","name":"Trial"})).unwrap();
        assert!(!v.include_worlds);
        assert!(valid_input(&v).is_ok());
        assert!(
            serde_json::from_value::<PreviewRequest>(
                json!({"gameId":"game","name":"Trial","directory":"C:/secret"})
            )
            .is_err()
        );
        let bad = PreviewRequest {
            game_id: "../game".into(),
            name: "Trial".into(),
            include_worlds: false,
        };
        assert!(valid_input(&bad).is_err());
    }
    fn auth() -> Auth {
        Auth {
            manifest: serde_json::from_str(include_str!(
                "../../../../examples/trial-copy/plugin.json"
            ))
            .unwrap(),
            instance_id: Some("source".into()),
            keybinding_target_id: None,
            handoff_id: None,
            config_restore_target: None,
            locale: "en-US".into(),
            theme: "light".into(),
            appearance: None,
            cancel: Arc::new(AtomicBool::new(false)),
            session_id: "fixture-view".into(),
        }
    }
    #[test]
    fn copy_requires_its_grant_and_exact_bound_source() {
        let mut a = auth();
        assert!(authorize(&a, Some("source")).is_ok());
        assert_eq!(
            authorize(&a, Some("another")).unwrap_err(),
            "PLUGIN_GAME_SCOPE"
        );
        a.manifest
            .permissions
            .as_mut()
            .unwrap()
            .retain(|p| !matches!(p, PermissionNode::InstanceCopyRequest {}));
        assert_eq!(
            authorize(&a, Some("source")).unwrap_err(),
            "PLUGIN_PERMISSION_DENIED"
        );
        let a = auth();
        a.cancel.store(true, Ordering::Release);
        assert_eq!(
            authorize(&a, Some("source")).unwrap_err(),
            "PLUGIN_VIEW_REVOKED"
        );
    }
    #[test]
    fn task_cannot_be_read_or_cancelled_by_another_view_or_plugin() {
        let a = auth();
        let id = nonce().unwrap();
        let j = Arc::new(Job {
            owner: a.manifest.id.clone(),
            session: a.session_id.clone(),
            created: Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
            value: Mutex::new(json!({"status":"succeeded"})),
            finished: AtomicBool::new(true),
            done: tokio::sync::Notify::new(),
        });
        store().lock().unwrap().jobs.insert(id.clone(), j);
        assert!(job(&a, &id).is_ok());
        let mut other = a.clone();
        other.session_id = "another-view".into();
        assert!(job(&other, &id).is_err());
        other = a.clone();
        other.manifest.id = "another-plugin".into();
        assert!(job(&other, &id).is_err());
        store().lock().unwrap().jobs.remove(&id);
    }
}
