// SPDX-License-Identifier: MIT
//! Local task history and controls. Requests and paths remain in the host.
use super::*;
use cobble_core::OperationControl;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}
impl TaskState {
    fn active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Cancelling)
    }
    fn occupies_slot(self) -> bool {
        matches!(self, Self::Running | Self::Cancelling)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct TaskSnapshot {
    pub id: String,
    pub game_id: Option<String>,
    #[serde(default)]
    pub world_folder: Option<String>,
    pub game_name: String,
    pub title: String,
    pub kind: String,
    pub state: TaskState,
    pub progress: Option<InstallProgress>,
    pub error: Option<String>,
    pub can_retry: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub has_result: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(super) enum TaskRequest {
    GameInstall {
        game_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        initial_mod_plan: Option<cobble_content::ModrinthPlan>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        initial_environment_plan: Option<cobble_content::EnvironmentPlan>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        initial_shared_configs: Vec<cobble_core::environment_install::SharedConfig>,
        #[serde(default)]
        initial_personal_options: Option<cobble_core::personal_options::PersonalOptions>,
    },
    GameReadiness {
        game_id: String,
        plan: native_readiness::RepairPlan,
    },
    GameCopy {
        game_id: String,
        target_id: String,
        target_parent: PathBuf,
        name: String,
        include_worlds: bool,
    },
    DirectoryMigration {
        plan: directory_migration::Plan,
    },
    GameRepair {
        game_id: String,
    },
    GameJava {
        game_id: String,
        required: u32,
        generation: u64,
        profile_id: String,
        server_id: Option<String>,
    },
    ModInstall {
        game_id: String,
        plan: cobble_content::ModrinthPlan,
    },
    CurseForgeModInstall {
        game_id: String,
        plan: cobble_content::curseforge::InstallPlan,
    },
    ModUpdate {
        game_id: String,
        plan: cobble_core::ModUpdatePlan,
    },
    ContentPack {
        game_id: String,
        kind: cobble_content::LocalPackKind,
        project_id: String,
        version_id: String,
    },
    ModpackDownload {
        archive_id: String,
        project_id: String,
        version_id: String,
    },
    CurseForgePackDownload {
        archive_id: String,
        project_id: u32,
        file_id: u32,
    },
    WorldImport {
        game_id: String,
        source: worlds::WorldArchiveSource,
        folder_name: String,
        replace_existing: bool,
        operation_id: String,
    },
    WorldRollback {
        game_id: String,
        backup_id: String,
        operation_id: String,
    },
    WorldAutomaticBackup {
        game_id: String,
        session: sessions::Session,
    },
    WorldSnapshot {
        game_id: String,
        folder_name: String,
        name: Option<String>,
    },
    WorldSnapshotRestore {
        game_id: String,
        folder_name: String,
        snapshot_id: String,
        operation_id: String,
    },
    TrialCopy {
        game_id: String,
    },
    GameTransfer {
        game_id: String,
    },
    SharedChanges {
        game_id: String,
    },
    StorageMove {
        game_id: String,
    },
    PackUpdateCopy {
        game_id: String,
        #[serde(default)]
        target_id: String,
        #[serde(default)]
        archive_id: String,
    },
}
impl TaskRequest {
    fn game_id(&self) -> Option<&str> {
        match self {
            Self::GameInstall { game_id, .. }
            | Self::GameReadiness { game_id, .. }
            | Self::GameCopy { game_id, .. }
            | Self::GameRepair { game_id }
            | Self::GameJava { game_id, .. }
            | Self::ModInstall { game_id, .. }
            | Self::CurseForgeModInstall { game_id, .. }
            | Self::ModUpdate { game_id, .. }
            | Self::ContentPack { game_id, .. }
            | Self::WorldImport { game_id, .. }
            | Self::WorldRollback { game_id, .. }
            | Self::WorldAutomaticBackup { game_id, .. }
            | Self::WorldSnapshot { game_id, .. }
            | Self::WorldSnapshotRestore { game_id, .. } => Some(game_id),
            Self::TrialCopy { game_id }
            | Self::GameTransfer { game_id }
            | Self::SharedChanges { game_id }
            | Self::StorageMove { game_id }
            | Self::PackUpdateCopy { game_id, .. } => Some(game_id),
            Self::ModpackDownload { .. }
            | Self::CurseForgePackDownload { .. }
            | Self::DirectoryMigration { .. } => None,
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::GameInstall { .. } => "game_install",
            Self::GameReadiness { .. } => "game_readiness",
            Self::GameCopy { .. } => "game_copy",
            Self::DirectoryMigration { .. } => "directory_migration",
            Self::GameRepair { .. } => "game_repair",
            Self::GameJava { .. } => "game_java",
            Self::ModInstall { .. } => "mod_install",
            Self::CurseForgeModInstall { .. } => "mod_install",
            Self::ModUpdate { .. } => "mod_update",
            Self::ContentPack {
                kind: cobble_content::LocalPackKind::ResourcePack,
                ..
            } => "resource_pack",
            Self::ContentPack { .. } => "shader_pack",
            Self::ModpackDownload { .. } => "modpack_download",
            Self::CurseForgePackDownload { .. } => "curseforge_pack_download",
            Self::WorldImport { .. } | Self::WorldRollback { .. } => "world_restore",
            Self::WorldAutomaticBackup { .. } => "world_automatic_backup",
            Self::WorldSnapshot { .. } => "world_snapshot",
            Self::WorldSnapshotRestore { .. } => "world_snapshot_restore",
            Self::TrialCopy { .. } => "trial_copy",
            Self::GameTransfer { .. } => "game_transfer",
            Self::SharedChanges { .. } => "change_share",
            Self::StorageMove { .. } => "storage_move",
            Self::PackUpdateCopy { .. } => "pack_update_copy",
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct StoredTask {
    snapshot: TaskSnapshot,
    request: TaskRequest,
}
#[derive(Default)]
struct TaskStore {
    records: Vec<StoredTask>,
    controls: HashMap<String, OperationControl>,
    external_controls: HashMap<String, Arc<std::sync::atomic::AtomicBool>>,
    load_error: Option<String>,
}
pub struct TaskLibrary(Mutex<TaskStore>, tokio::sync::Notify);

fn should_queue(records: &[StoredTask], game_id: Option<&str>) -> bool {
    records.iter().any(|r| {
        matches!(r.request, TaskRequest::DirectoryMigration { .. }) && r.snapshot.state.active()
    }) || records
        .iter()
        .filter(|r| r.snapshot.state.occupies_slot())
        .count()
        >= 4
        || game_id.is_some_and(|id| {
            records
                .iter()
                .any(|r| r.snapshot.game_id.as_deref() == Some(id) && r.snapshot.state.active())
        })
}

fn can_start(records: &[StoredTask], index: usize) -> bool {
    let migration = matches!(
        records[index].request,
        TaskRequest::DirectoryMigration { .. }
    );
    if records.iter().enumerate().any(|(i, r)| {
        i != index
            && r.snapshot.state.occupies_slot()
            && (migration || matches!(r.request, TaskRequest::DirectoryMigration { .. }))
    }) || records[..index].iter().any(|r| {
        r.snapshot.state == TaskState::Queued
            && (migration || matches!(r.request, TaskRequest::DirectoryMigration { .. }))
    }) {
        return false;
    }
    if records
        .iter()
        .filter(|r| r.snapshot.state.occupies_slot())
        .count()
        >= 4
    {
        return false;
    }
    let Some(game_id) = records[index].snapshot.game_id.as_deref() else {
        return true;
    };
    !records
        .iter()
        .any(|r| r.snapshot.game_id.as_deref() == Some(game_id) && r.snapshot.state.occupies_slot())
        && !records[..index].iter().any(|r| {
            r.snapshot.game_id.as_deref() == Some(game_id) && r.snapshot.state == TaskState::Queued
        })
}

fn task_file() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("无法找到配置目录")?
        .join("CobbleLauncher/tasks.json"))
}
fn persist(store: &TaskStore) -> Result<(), String> {
    let file = task_file()?;
    fs::create_dir_all(file.parent().ok_or("无法找到任务目录")?).map_err(|e| e.to_string())?;
    persist_records(&file, &store.records)
}
fn persist_records(file: &std::path::Path, records: &[StoredTask]) -> Result<(), String> {
    let temporary = file.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec(records).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, file).map_err(|e| e.to_string())
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn advance(previous: u64) -> u64 {
    now().max(previous.saturating_add(1))
}
fn recover_records(records: &mut [StoredTask]) {
    for record in records {
        if record.snapshot.state.active() {
            record.snapshot.state = TaskState::Interrupted;
            record.snapshot.can_retry = !matches!(
                record.request,
                TaskRequest::TrialCopy { .. }
                    | TaskRequest::GameTransfer { .. }
                    | TaskRequest::SharedChanges { .. }
                    | TaskRequest::StorageMove { .. }
                    | TaskRequest::PackUpdateCopy { .. }
            );
            record.snapshot.error = Some(
                if record.snapshot.can_retry {
                    "上次操作已中断，可以继续"
                } else {
                    "操作中断，请检查目标游戏后重新发起"
                }
                .into(),
            );
            record.snapshot.updated_at = advance(record.snapshot.updated_at);
        }
    }
}
fn terminal_state(success: bool, user_cancelled: bool) -> TaskState {
    if success {
        TaskState::Succeeded
    } else if user_cancelled {
        TaskState::Cancelled
    } else {
        TaskState::Failed
    }
}

fn removable_history(record: &StoredTask) -> bool {
    // The durable request also locates the retained-source receipt after restart.
    if matches!(record.request, TaskRequest::DirectoryMigration { .. }) {
        return false;
    }
    let shared_setup = matches!(&record.request,TaskRequest::GameInstall{initial_shared_configs,initial_personal_options,..}if !initial_shared_configs.is_empty() || initial_personal_options.is_some());
    !record.snapshot.state.active()
        && !(record.snapshot.state != TaskState::Succeeded
            && (shared_setup
                || matches!(
                    record.request,
                    TaskRequest::GameInstall {
                        initial_mod_plan: Some(_),
                        ..
                    } | TaskRequest::GameInstall {
                        initial_environment_plan: Some(_),
                        ..
                    } | TaskRequest::GameCopy { .. }
                )))
}

pub(super) fn game_install_task(app: &AppHandle, game_id: &str) -> Result<Option<String>, String> {
    let state = app.state::<TaskLibrary>();
    let store = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(error) = &store.load_error {
        return Err(error.clone());
    }
    Ok(store
        .records
        .iter()
        .rev()
        .find_map(|record| match &record.request {
            TaskRequest::GameInstall { game_id: id, .. } if id == game_id => {
                Some(record.snapshot.id.clone())
            }
            _ => None,
        }))
}

pub(super) fn environment_plan_for_task(
    app: &AppHandle,
    task_id: &str,
) -> Result<cobble_content::EnvironmentPlan, String> {
    let state = app.state::<TaskLibrary>();
    let store = state.0.lock().map_err(|_| "TASK_HISTORY_UNAVAILABLE")?;
    let record = store
        .records
        .iter()
        .find(|r| r.snapshot.id == task_id)
        .ok_or("TASK_NOT_FOUND")?;
    match &record.request {
        TaskRequest::GameInstall {
            initial_environment_plan: Some(plan),
            ..
        } => Ok(plan.clone()),
        TaskRequest::GameReadiness { plan, .. } => plan
            .environment
            .clone()
            .ok_or("ENVIRONMENT_PLAN_UNAVAILABLE".into()),
        _ => Err("ENVIRONMENT_PLAN_UNAVAILABLE".into()),
    }
}

pub(super) fn start_game_copy_task(
    app: &AppHandle,
    game_id: &str,
    target_id: &str,
    target_parent: &std::path::Path,
    name: &str,
    include_worlds: bool,
) -> Result<String, String> {
    let id = register(
        app,
        TaskRequest::GameCopy {
            game_id: game_id.into(),
            target_id: target_id.into(),
            target_parent: target_parent.to_owned(),
            name: name.into(),
            include_worlds,
        },
        "复制游戏",
    )?;
    let worker_app = app.clone();
    let worker_id = id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = execute(worker_app, worker_id).await;
    });
    Ok(id)
}

pub(super) fn start_directory_migration_task(
    app: &AppHandle,
    plan: directory_migration::Plan,
) -> Result<String, String> {
    let id = register(
        app,
        TaskRequest::DirectoryMigration { plan },
        "迁移游戏位置",
    )?;
    let worker = app.clone();
    let task = id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = execute(worker, task).await;
    });
    Ok(id)
}
pub(super) fn directory_migration_plan(
    app: &AppHandle,
    id: &str,
    completed_only: bool,
) -> Result<directory_migration::Plan, String> {
    let state = app.state::<TaskLibrary>();
    let store = state.0.lock().map_err(|_| "DIRECTORY_MIGRATION_IO")?;
    store
        .records
        .iter()
        .find_map(|r| match &r.request {
            TaskRequest::DirectoryMigration { plan }
                if r.snapshot.id == id
                    && (!completed_only || r.snapshot.state == TaskState::Succeeded) =>
            {
                Some(plan.clone())
            }
            _ => None,
        })
        .ok_or("DIRECTORY_MIGRATION_INCOMPLETE".into())
}
pub(super) fn directory_migration_progress(
    app: &AppHandle,
    plan_id: &str,
    completed: u64,
    total: u64,
) {
    let state = app.state::<TaskLibrary>();
    let id = state.0.lock().ok().and_then(|s| {
        s.records.iter().find_map(|r| match &r.request {
            TaskRequest::DirectoryMigration { plan }
                if plan.id == plan_id && r.snapshot.state.active() =>
            {
                Some(r.snapshot.id.clone())
            }
            _ => None,
        })
    });
    if let Some(id) = id {
        progress(
            app,
            &id,
            InstallProgress {
                phase: "copying".into(),
                completed: completed.min(usize::MAX as u64) as usize,
                total: total.min(usize::MAX as u64) as usize,
                reused: 0,
                detail: None,
            },
        );
    }
}

pub(super) fn game_copy_progress(app: &AppHandle, target_id: &str, completed: u64, total: u64) {
    let state = app.state::<TaskLibrary>();
    let id = state.0.lock().ok().and_then(|store| {
        store
            .records
            .iter()
            .find_map(|record| match &record.request {
                TaskRequest::GameCopy { target_id: id, .. }
                    if id == target_id && record.snapshot.state.active() =>
                {
                    Some(record.snapshot.id.clone())
                }
                _ => None,
            })
    });
    if let Some(id) = id {
        progress(
            app,
            &id,
            InstallProgress {
                detail: None,
                phase: "copying".into(),
                completed: completed.min(usize::MAX as u64) as usize,
                total: total.min(usize::MAX as u64) as usize,
                reused: 0,
            },
        );
    }
}

/// Only used before the creator spawns its worker, when saving the game failed.
pub(super) fn discard_unstarted_install(app: &AppHandle, task_id: &str) -> Result<(), String> {
    let state = app.state::<TaskLibrary>();
    let mut store = state.0.lock().map_err(|error| error.to_string())?;
    let index = store
        .records
        .iter()
        .position(|record| record.snapshot.id == task_id)
        .ok_or("找不到这个任务")?;
    if store.controls.contains_key(task_id)
        || !matches!(
            store.records[index].request,
            TaskRequest::GameInstall { .. }
        )
    {
        return Err("安装任务已经开始".into());
    }
    let record = store.records.remove(index);
    if let Err(error) = persist(&store) {
        store.records.insert(index, record);
        return Err(error);
    }
    drop(store);
    state.1.notify_waiters();
    Ok(())
}
impl TaskLibrary {
    pub fn load() -> Self {
        let result = (|| -> Result<Vec<StoredTask>, String> {
            match fs::read(task_file()?) {
                Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
                Err(error) => Err(error.to_string()),
            }
        })();
        let mut store = TaskStore::default();
        match result {
            Ok(mut records) => {
                recover_records(&mut records);
                store.records = records;
            }
            Err(error) => store.load_error = Some(format!("无法读取任务记录：{error}")),
        }
        Self(Mutex::new(store), tokio::sync::Notify::new())
    }
}

fn emit(app: &AppHandle, snapshot: &TaskSnapshot) {
    let _ = app.emit("task-updated", snapshot);
}

pub(super) fn register(
    app: &AppHandle,
    request: TaskRequest,
    title: &str,
) -> Result<String, String> {
    let game_id = request.game_id().map(str::to_owned);
    let game_name = if let Some(id) = &game_id {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|e| e.to_string())?;
        library
            .games
            .iter()
            .find(|game| &game.id == id)
            .ok_or("找不到这个游戏")?
            .name
            .clone()
    } else {
        String::new()
    };
    let state = app.state::<TaskLibrary>();
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(error) = &store.load_error {
        return Err(error.clone());
    }
    if store
        .records
        .iter()
        .filter(|r| r.snapshot.state.active())
        .count()
        >= 32
    {
        return Err("正在进行的任务已达到上限".into());
    }
    if matches!(request, TaskRequest::DirectoryMigration { .. })
        && store.records.iter().any(|r| {
            matches!(r.request, TaskRequest::DirectoryMigration { .. }) && r.snapshot.state.active()
        })
    {
        return Err("DIRECTORY_MIGRATION_BUSY".into());
    }
    let queued = should_queue(&store.records, game_id.as_deref())
        || matches!(request, TaskRequest::DirectoryMigration { .. })
            && store.records.iter().any(|r| r.snapshot.state.active());
    let id = next_pack_id()?;
    let stamp = now();
    let snapshot = TaskSnapshot {
        id: id.clone(),
        game_id,
        world_folder: match &request {
            TaskRequest::WorldSnapshot { folder_name, .. }
            | TaskRequest::WorldSnapshotRestore { folder_name, .. } => Some(folder_name.clone()),
            _ => None,
        },
        game_name,
        title: title.into(),
        kind: request.kind().into(),
        state: if queued {
            TaskState::Queued
        } else {
            TaskState::Running
        },
        progress: None,
        error: None,
        can_retry: false,
        created_at: stamp,
        updated_at: stamp,
        has_result: false,
    };
    store.records.push(StoredTask {
        snapshot: snapshot.clone(),
        request,
    });
    // Keep recent history; active tasks are never removed.
    while store.records.len() > 100 {
        let Some(index) = store.records.iter().position(removable_history) else {
            break;
        };
        store.records.remove(index);
    }
    if let Err(error) = persist(&store) {
        store.records.retain(|record| record.snapshot.id != id);
        return Err(error);
    }
    drop(store);
    emit(app, &snapshot);
    Ok(id)
}

/// Host-owned work can keep running after the requesting view is destroyed.
pub(crate) fn register_external(
    app: &AppHandle,
    game_id: &str,
    title: &str,
    kind: &str,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> Result<String, String> {
    let request = match kind {
        "trial_copy" => TaskRequest::TrialCopy {
            game_id: game_id.into(),
        },
        "game_transfer" => TaskRequest::GameTransfer {
            game_id: game_id.into(),
        },
        "change_share" => TaskRequest::SharedChanges {
            game_id: game_id.into(),
        },
        "storage_move" => TaskRequest::StorageMove {
            game_id: game_id.into(),
        },
        _ => return Err("未知任务类型".into()),
    };
    register_external_request(app, request, title, cancel)
}

pub(super) fn register_pack_update_copy(
    app: &AppHandle,
    game_id: &str,
    target_id: &str,
    archive_id: &str,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> Result<String, String> {
    if ![target_id, archive_id]
        .iter()
        .all(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err("无效的更新副本标识".into());
    }
    register_external_request(
        app,
        TaskRequest::PackUpdateCopy {
            game_id: game_id.into(),
            target_id: target_id.into(),
            archive_id: archive_id.into(),
        },
        "创建更新副本",
        cancel,
    )
}

fn register_external_request(
    app: &AppHandle,
    request: TaskRequest,
    title: &str,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> Result<String, String> {
    let id = register(app, request, title)?;
    let state = app.state::<TaskLibrary>();
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    if store
        .records
        .iter()
        .any(|r| r.snapshot.id == id && r.snapshot.state == TaskState::Cancelling)
    {
        cancel.store(true, std::sync::atomic::Ordering::Release);
    }
    store.external_controls.insert(id.clone(), cancel);
    Ok(id)
}

pub(crate) fn external_progress(
    app: &AppHandle,
    id: &str,
    phase: &str,
    completed: usize,
    total: usize,
) {
    progress(
        app,
        id,
        InstallProgress {
            detail: None,
            phase: phase.into(),
            completed,
            total,
            reused: 0,
        },
    );
}

async fn wait_turn(app: &AppHandle, id: &str) -> Result<(), String> {
    loop {
        let state = app.state::<TaskLibrary>();
        let notified = state.1.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let promoted = {
            let mut store = state.0.lock().map_err(|e| e.to_string())?;
            let index = store
                .records
                .iter()
                .position(|r| r.snapshot.id == id)
                .ok_or("找不到这个任务")?;
            match store.records[index].snapshot.state {
                TaskState::Running => return Ok(()),
                TaskState::Queued => {}
                TaskState::Cancelling | TaskState::Cancelled => return Err("任务已取消".into()),
                _ => return Err("任务未能开始".into()),
            }
            if can_start(&store.records, index) {
                let previous = store.records[index].snapshot.clone();
                store.records[index].snapshot.state = TaskState::Running;
                store.records[index].snapshot.updated_at = advance(previous.updated_at);
                let snapshot = store.records[index].snapshot.clone();
                if let Err(error) = persist(&store) {
                    store.records[index].snapshot = previous;
                    return Err(error);
                }
                Some(snapshot)
            } else {
                None
            }
        };
        if let Some(snapshot) = promoted {
            emit(app, &snapshot);
            return Ok(());
        }
        notified.await;
    }
}

pub(crate) async fn wait_external_turn(app: &AppHandle, id: &str) -> Result<(), String> {
    wait_turn(app, id).await
}

pub(crate) fn finish_external(app: &AppHandle, id: &str, result: Result<Option<String>, String>) {
    let result_name = result
        .as_ref()
        .ok()
        .and_then(|id| id.as_ref())
        .and_then(|id| {
            app.state::<GameLibrary>()
                .0
                .lock()
                .ok()?
                .games
                .iter()
                .find(|game| &game.id == id)
                .map(|game| game.name.clone())
        });
    let state = app.state::<TaskLibrary>();
    let Ok(mut store) = state.0.lock() else {
        return;
    };
    store.external_controls.remove(id);
    let Some(record) = store.records.iter_mut().find(|r| r.snapshot.id == id) else {
        return;
    };
    if record.snapshot.state == TaskState::Cancelled {
        drop(store);
        state.1.notify_waiters();
        return;
    }
    let cancelled = record.snapshot.state == TaskState::Cancelling;
    record.snapshot.state = terminal_state(result.is_ok(), cancelled);
    record.snapshot.error = if cancelled && result.is_err() {
        None
    } else {
        result.as_ref().err().cloned()
    };
    record.snapshot.can_retry = false;
    if let Ok(Some(game_id)) = result {
        record.snapshot.game_id = Some(game_id);
        if let Some(name) = result_name {
            record.snapshot.game_name = name;
        }
        record.snapshot.has_result = true;
    }
    record.snapshot.updated_at = advance(record.snapshot.updated_at);
    let mut snapshot = record.snapshot.clone();
    if let Err(error) = persist(&store) {
        snapshot.error = Some(format!("任务记录未能保存：{error}"));
        if let Some(record) = store
            .records
            .iter_mut()
            .find(|record| record.snapshot.id == id)
        {
            record.snapshot.error = snapshot.error.clone();
        }
    }
    drop(store);
    emit(app, &snapshot);
    state.1.notify_waiters();
}

pub(crate) fn active_count(app: &AppHandle) -> usize {
    app.state::<TaskLibrary>()
        .0
        .lock()
        .map(|s| {
            s.records
                .iter()
                .filter(|r| r.snapshot.state.active())
                .count()
        })
        .unwrap_or(0)
}

pub(crate) fn cancel_active(app: &AppHandle) {
    let ids = app
        .state::<TaskLibrary>()
        .0
        .lock()
        .map(|s| {
            s.records
                .iter()
                .filter(|r| r.snapshot.state.active())
                .map(|r| r.snapshot.id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for id in ids {
        let _ = cancel_task(app.clone(), id);
    }
}

/// Remove only an unpublished target that this interrupted task marked as its own.
/// The retained archive cache is left for inspection or a new plan.
pub(crate) fn reconcile_interrupted_pack_copies(app: &AppHandle) -> Result<usize, String> {
    let pending = {
        let state = app.state::<TaskLibrary>();
        let store = state.0.lock().map_err(|e| e.to_string())?;
        store
            .records
            .iter()
            .filter_map(|record| {
                if record.snapshot.state != TaskState::Interrupted {
                    return None;
                }
                let TaskRequest::PackUpdateCopy {
                    game_id,
                    target_id,
                    archive_id,
                } = &record.request
                else {
                    return None;
                };
                Some((
                    record.snapshot.id.clone(),
                    game_id.clone(),
                    target_id.clone(),
                    archive_id.clone(),
                ))
            })
            .collect::<Vec<_>>()
    };
    if pending.is_empty() {
        return Ok(0);
    }
    let base = default_directory()?;
    let base = fs::canonicalize(base).map_err(|e| e.to_string())?;
    let registered = {
        let library = app.state::<GameLibrary>();
        let library = library.0.lock().map_err(|e| e.to_string())?;
        library
            .games
            .iter()
            .map(|game| (game.id.clone(), game.directory.clone()))
            .collect::<Vec<_>>()
    };
    let mut cleaned = 0;
    for (task_id, source_id, target_id, archive_id) in pending {
        cleaned += usize::from(cleanup_pack_copy_target(
            &base,
            &registered,
            &task_id,
            &source_id,
            &target_id,
            &archive_id,
        )?);
    }
    let state = app.state::<TaskLibrary>();
    let store = state.0.lock().map_err(|e| e.to_string())?;
    persist(&store)?;
    Ok(cleaned)
}

fn cleanup_pack_copy_target(
    base: &std::path::Path,
    registered: &[(String, PathBuf)],
    task_id: &str,
    source_id: &str,
    target_id: &str,
    archive_id: &str,
) -> Result<bool, String> {
    if ![target_id, archive_id]
        .iter()
        .all(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Ok(false);
    }
    let target = base.join(target_id);
    let resolved_target = fs::canonicalize(&target).ok();
    if registered.iter().any(|(id, path)| {
        id == target_id
            || path == &target
            || resolved_target
                .as_ref()
                .is_some_and(|target| fs::canonicalize(path).ok().as_ref() == Some(target))
    }) {
        return Ok(false);
    }
    let marker = base.join(format!(".cobble-pack-update-{target_id}.json"));
    let metadata = match fs::symlink_metadata(&marker) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 4096 {
        return Ok(false);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Ok(false);
        }
    }
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if value["taskId"] != task_id
        || value["sourceGameId"] != source_id
        || value["targetId"] != target_id
        || value["archiveId"] != archive_id
    {
        return Ok(false);
    }
    match fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Ok(false);
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Ok(false);
                }
            }
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    fs::remove_file(&marker).map_err(|e| e.to_string())?;
    Ok(true)
}

fn progress(app: &AppHandle, id: &str, value: InstallProgress) {
    let state = app.state::<TaskLibrary>();
    let Ok(mut store) = state.0.lock() else {
        return;
    };
    let Some(record) = store
        .records
        .iter_mut()
        .find(|record| record.snapshot.id == id)
    else {
        return;
    };
    if !record.snapshot.state.active() {
        return;
    }
    record.snapshot.progress = Some(value.clone());
    record.snapshot.updated_at = advance(record.snapshot.updated_at);
    let snapshot = record.snapshot.clone();
    let game_id = matches!(record.request, TaskRequest::GameInstall { .. })
        .then(|| snapshot.game_id.clone())
        .flatten();
    drop(store);
    emit(app, &snapshot);
    if let Some(game_id) = game_id {
        update_game(app, &game_id, |game| game.progress = Some(value), false);
    }
}

pub(super) enum TaskResult {
    Game,
    Java(cobble_minecraft::JavaRuntime),
    ModInstall(cobble_core::ModrinthInstallOutcome),
    ModUpdate(cobble_core::ModUpdateOutcome),
    Packs(Vec<cobble_content::LocalPack>),
    Pack(PackPreview),
    CurseForgePack(curseforge_pack::Preview),
    Worlds(worlds::WorldRestoreResult),
    Snapshot(cobble_core::WorldSnapshot),
}

pub(super) async fn run(
    app: AppHandle,
    request: TaskRequest,
    title: &str,
) -> Result<TaskResult, String> {
    let id = register(&app, request, title)?;
    // The command caller may disappear with its page. Keep the worker owned by
    // the application and return its result only while that caller still waits.
    let (sender, receiver) = tokio::sync::oneshot::channel();
    tauri::async_runtime::spawn(async move {
        let _ = sender.send(execute(app, id).await);
    });
    receiver.await.map_err(|_| "任务已中断".to_string())?
}

pub(super) async fn execute(app: AppHandle, id: String) -> Result<TaskResult, String> {
    if let Err(error) = wait_turn(&app, &id).await {
        let state = app.state::<TaskLibrary>();
        let install_game = state.0.lock().ok().and_then(|store| {
            store
                .records
                .iter()
                .find(|record| record.snapshot.id == id)
                .and_then(|record| match &record.request {
                    TaskRequest::GameInstall { game_id, .. } => Some(game_id.clone()),
                    _ => None,
                })
        });
        if let Ok(mut store) = state.0.lock()
            && let Some(record) = store.records.iter_mut().find(|r| r.snapshot.id == id)
            && record.snapshot.state.active()
        {
            record.snapshot.state = if record.snapshot.state == TaskState::Cancelling {
                TaskState::Cancelled
            } else {
                TaskState::Failed
            };
            record.snapshot.error = if record.snapshot.state == TaskState::Cancelled {
                None
            } else {
                Some(error.clone())
            };
            record.snapshot.can_retry =
                record.snapshot.state == TaskState::Failed || install_game.is_some();
            record.snapshot.updated_at = advance(record.snapshot.updated_at);
            let snapshot = record.snapshot.clone();
            let _ = persist(&store);
            drop(store);
            emit(&app, &snapshot);
            state.1.notify_waiters();
        }
        if let Some(game_id) = install_game {
            update_game(
                &app,
                &game_id,
                |game| {
                    game.status = "failed".into();
                    game.error = Some(error.clone());
                },
                true,
            );
        }
        return Err(error);
    }
    let progress_app = app.clone();
    let progress_id = id.clone();
    let throttle = Mutex::new((String::new(), Instant::now() - Duration::from_secs(1)));
    let control = OperationControl::new(Arc::new(move |value: InstallProgress| {
        let Ok(mut last) = throttle.lock() else {
            return;
        };
        if last.0 != value.phase
            || value.completed == value.total
            || last.1.elapsed() >= Duration::from_millis(100)
        {
            *last = (value.phase.clone(), Instant::now());
            progress(&progress_app, &progress_id, value);
        }
    }));
    let request = {
        let state = app.state::<TaskLibrary>();
        let mut store = state.0.lock().map_err(|e| e.to_string())?;
        let record = store
            .records
            .iter()
            .find(|record| record.snapshot.id == id)
            .ok_or("找不到这个任务")?;
        let request = record.request.clone();
        if record.snapshot.state == TaskState::Cancelling {
            control.cancel();
        }
        store.controls.insert(id.clone(), control.clone());
        request
    };
    let result = perform(&app, &request, control.clone()).await;
    let user_cancelled = {
        let state = app.state::<TaskLibrary>();
        let store = state.0.lock().map_err(|e| e.to_string())?;
        store.records.iter().any(|record| {
            record.snapshot.id == id && record.snapshot.state == TaskState::Cancelling
        })
    };
    if let TaskRequest::GameInstall { game_id, .. } = &request {
        update_game(
            &app,
            game_id,
            |game| {
                game.status = if result.is_ok() {
                    "installed"
                } else {
                    "failed"
                }
                .into();
                game.error = result.as_ref().err().map(|error| {
                    if user_cancelled {
                        "安装已取消，可以继续安装".into()
                    } else {
                        error.clone()
                    }
                });
            },
            true,
        );
    }
    let state = app.state::<TaskLibrary>();
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    store.controls.remove(&id);
    let record = store
        .records
        .iter_mut()
        .find(|record| record.snapshot.id == id)
        .ok_or("找不到这个任务")?;
    record.snapshot.state = terminal_state(result.is_ok(), user_cancelled);
    record.snapshot.error = if record.snapshot.state == TaskState::Cancelled {
        None
    } else {
        result.as_ref().err().cloned()
    };
    record.snapshot.can_retry = result.is_err();
    if result.is_ok()
        && let TaskRequest::GameCopy {
            target_id, name, ..
        } = &request
    {
        record.snapshot.game_id = Some(target_id.clone());
        record.snapshot.game_name = name.clone();
    }
    record.snapshot.has_result = result.is_ok()
        && matches!(
            request,
            TaskRequest::ModpackDownload { .. } | TaskRequest::CurseForgePackDownload { .. }
        );
    if let Ok(TaskResult::Pack(preview)) = &result {
        record.snapshot.title = format!("下载 {}", preview.name);
    }
    if let Ok(TaskResult::CurseForgePack(preview)) = &result {
        record.snapshot.title = format!("下载 {}", preview.name);
    }
    record.snapshot.updated_at = advance(record.snapshot.updated_at);
    let mut snapshot = record.snapshot.clone();
    if let Err(error) = persist(&store) {
        snapshot.error = Some(format!("任务记录未能保存：{error}"));
        if let Some(record) = store
            .records
            .iter_mut()
            .find(|record| record.snapshot.id == id)
        {
            record.snapshot.error = snapshot.error.clone();
        }
    }
    drop(store);
    emit(&app, &snapshot);
    state.1.notify_waiters();
    result
}

async fn initial_mods_after_base(
    base: impl std::future::Future<Output = Result<(), String>>,
    plan: Option<&cobble_content::ModrinthPlan>,
    directory: &std::path::Path,
    control: OperationControl,
) -> Result<TaskResult, String> {
    base.await?;
    if let Some(plan) = plan {
        control
            .check_cancelled()
            .map_err(|error| error.to_string())?;
        cobble_core::install_modrinth_plan_controlled(plan, directory, control)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(TaskResult::Game)
}

async fn perform(
    app: &AppHandle,
    request: &TaskRequest,
    control: OperationControl,
) -> Result<TaskResult, String> {
    match request {
        TaskRequest::DirectoryMigration { plan } => {
            directory_migration::perform(app, plan, control).await?;
            Ok(TaskResult::Game)
        }
        TaskRequest::GameCopy {
            game_id,
            target_id,
            target_parent,
            name,
            include_worlds,
        } => {
            builtin_copy::perform(
                app,
                game_id,
                target_id,
                target_parent,
                name,
                *include_worlds,
                control,
            )
            .await?;
            Ok(TaskResult::Game)
        }
        TaskRequest::GameReadiness { game_id, plan } => {
            native_readiness::perform(app, game_id, plan, control).await?;
            Ok(TaskResult::Game)
        }
        TaskRequest::TrialCopy { .. }
        | TaskRequest::GameTransfer { .. }
        | TaskRequest::SharedChanges { .. }
        | TaskRequest::StorageMove { .. }
        | TaskRequest::PackUpdateCopy { .. } => Err("请重新发起操作".into()),
        TaskRequest::GameJava { required, .. } => {
            let space = native_readiness_space::estimate_java(*required, control.clone()).await?;
            if !space.can_proceed {
                return Err("SPACE_INSUFFICIENT".into());
            }
            cobble_core::ensure_launch_java_controlled(
                *required,
                &cobble_minecraft::JavaOverride::Automatic,
                control,
            )
            .await
            .map(TaskResult::Java)
            .map_err(|error| error.to_string())
        }
        TaskRequest::GameRepair { game_id } => {
            let (game, _operation) = management::begin(app, game_id, true)?;
            native_reverse_install::preflight(&game, None, &[], None, control.clone()).await?;
            let settings = game
                .download_source
                .clone()
                .map(Ok)
                .unwrap_or_else(cobble_minecraft::read_download_settings)
                .map_err(|e| e.to_string())?;
            cobble_core::install_game_controlled(
                &game.version,
                &game.directory,
                settings.download_source().map_err(|e| e.to_string())?,
                control,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(TaskResult::Game)
        }
        TaskRequest::GameInstall {
            game_id,
            initial_mod_plan,
            initial_environment_plan,
            initial_shared_configs,
            initial_personal_options,
        } => {
            let game = {
                let state = app.state::<GameLibrary>();
                let library = state.0.lock().map_err(|e| e.to_string())?;
                library
                    .games
                    .iter()
                    .find(|game| &game.id == game_id)
                    .ok_or("找不到这个游戏")?
                    .clone()
            };
            if initial_mod_plan.is_some() && initial_environment_plan.is_some() {
                return Err("ENVIRONMENT_PLAN_AMBIGUOUS".into());
            }
            cobble_core::environment_install::validate_shared_configs(initial_shared_configs)
                .map_err(|e| e.to_string())?;
            if let Some(options) = initial_personal_options {
                cobble_core::personal_options::validate(options)?;
                if options.target_version != game.version {
                    return Err("PERSONAL_OPTIONS_TARGET_CHANGED".into());
                }
            }
            if let Some(plan) = initial_environment_plan {
                native_reverse::validate_target(&game, plan)?;
                cobble_core::environment_install::validate_environment_plan(plan)
                    .map_err(|e| e.to_string())?;
            }
            if game.launcher_import_id.is_none() && game.pack_source_id.is_none() {
                let legacy = initial_mod_plan
                    .as_ref()
                    .map(|graph| {
                        let loader = game
                            .loader_kind
                            .clone()
                            .ok_or("ENVIRONMENT_LOADER_UNAVAILABLE")?;
                        Ok::<_, String>(cobble_content::EnvironmentPlan {
                            game_version: game.version.clone(),
                            loader,
                            roots: vec![cobble_content::EnvironmentSelection {
                                source: cobble_content::EnvironmentSource::Modrinth,
                                project_id: graph.project_id.clone(),
                                version_id: graph.version_id.clone(),
                            }],
                            modrinth: Some(graph.clone()),
                            curseforge: None,
                            dependency_edges: vec![],
                        })
                    })
                    .transpose()?;
                native_reverse_install::preflight(
                    &game,
                    initial_environment_plan.as_ref().or(legacy.as_ref()),
                    initial_shared_configs,
                    initial_personal_options.as_ref(),
                    control.clone(),
                )
                .await?;
            }
            // Recover the commit-to-history gap without reimporting user data.
            let marker = if game.launcher_import_id.is_some() {
                Some(".cobble-import.json")
            } else if let Some(id) = &game.pack_source_id {
                Some(if curseforge_pack::is_source(id)? {
                    ".cobble-curseforge-pack.json"
                } else {
                    ".cobble-pack.json"
                })
            } else {
                None
            };
            if initial_mod_plan.is_none()
                && initial_environment_plan.is_none()
                && initial_shared_configs.is_empty()
                && initial_personal_options.is_none()
                && let Some(marker) = marker
                && let Ok(bytes) = fs::read(game.directory.join(marker))
                && serde_json::from_slice::<serde_json::Value>(&bytes)
                    .ok()
                    .and_then(|value| value.get("complete").and_then(serde_json::Value::as_bool))
                    == Some(true)
            {
                control.check_cancelled().map_err(|e| e.to_string())?;
                cobble_core::prepare_launch(&game.version, &game.directory)
                    .await
                    .map_err(|e| e.to_string())?;
                if game.launcher_import_id.is_some() {
                    launcher_import::reconcile_completed(&game, control.clone()).await?;
                }
                return Ok(TaskResult::Game);
            }
            let settings = game
                .download_source
                .clone()
                .map(Ok)
                .unwrap_or_else(cobble_minecraft::read_download_settings)
                .map_err(|e| e.to_string())?;
            let source = settings.download_source().map_err(|e| e.to_string())?;
            let base = async {
                if game.launcher_import_id.is_some() {
                    launcher_import::install(&game, source, control.clone()).await?;
                } else if let Some(source_id) = &game.pack_source_id {
                    if curseforge_pack::is_source(source_id)? {
                        curseforge_pack::install(&game, source, control.clone()).await?;
                    } else {
                        let archive = pack_archive_path(source_id)?;
                        let pack =
                            cobble_content::read_mrpack(&archive).map_err(|e| e.to_string())?;
                        let java = cobble_core::capture_java_choice(&game.runtime_overrides.java)
                            .map_err(|e| e.to_string())?;
                        let space = native_readiness_space::estimate_pack(
                            &pack,
                            &game.pack_optional_files,
                            &game.directory,
                            settings.clone(),
                            &java,
                            native_readiness_space::PackSpaceExtras::default(),
                            control.clone(),
                        )
                        .await?;
                        if !space.can_proceed {
                            return Err("SPACE_INSUFFICIENT".into());
                        }
                        cobble_core::install_mrpack_controlled(
                            &archive,
                            &game.directory,
                            source,
                            &game.pack_optional_files,
                            control.clone(),
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                    }
                } else {
                    match &game.loader_version {
                        Some(loader) => match game.loader_kind.as_deref().unwrap_or("fabric") {
                            "fabric" => {
                                cobble_core::install_fabric_controlled(
                                    &game.version,
                                    loader,
                                    &game.directory,
                                    source,
                                    control.clone(),
                                )
                                .await
                            }
                            "forge" | "neoforge" => {
                                cobble_core::install_forge_controlled(
                                    &game.version,
                                    loader,
                                    if game.loader_kind.as_deref() == Some("neoforge") {
                                        cobble_core::ForgeKind::NeoForge
                                    } else {
                                        cobble_core::ForgeKind::Forge
                                    },
                                    &game.directory,
                                    source,
                                    control.clone(),
                                )
                                .await
                            }
                            _ => Err("不支持这个模组加载器".into()),
                        },
                        None => {
                            cobble_core::install_game_controlled(
                                &game.version,
                                &game.directory,
                                source,
                                control.clone(),
                            )
                            .await
                        }
                    }
                    .map_err(|e| e.to_string())?;
                }
                Ok(())
            };
            initial_mods_after_base(
                base,
                initial_mod_plan.as_ref(),
                &game.directory,
                control.clone(),
            )
            .await?;
            if let Some(plan) = initial_environment_plan {
                native_reverse_install::install_initial(app, &game, plan, control.clone()).await?;
            }
            let directory = game.directory.clone();
            let configs = initial_shared_configs.clone();
            let config_control = control.clone();
            tauri::async_runtime::spawn_blocking(move || {
                cobble_core::environment_install::install_shared_configs_controlled(
                    &configs,
                    &directory,
                    config_control,
                )
            })
            .await
            .map_err(|_| "ENVIRONMENT_CONFIG_WORKER_FAILED")?
            .map_err(|e| e.to_string())?;
            if let Some(options) = initial_personal_options {
                let directory = game.directory.clone();
                let options = options.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    cobble_core::personal_options::install_controlled(&directory, &options, control)
                })
                .await
                .map_err(|_| "PERSONAL_OPTIONS_WORKER_FAILED")??;
            }
            Ok(TaskResult::Game)
        }
        TaskRequest::CurseForgePackDownload {
            archive_id,
            project_id,
            file_id,
        } => curseforge_pack::download(app, archive_id, *project_id, *file_id, control)
            .await
            .map(TaskResult::CurseForgePack),
        TaskRequest::ModInstall { game_id, plan } => {
            let (game, _operation) = management::begin(app, game_id, true)?;
            update_game(app, game_id, |_| {}, false);
            cobble_core::install_modrinth_plan_controlled(plan, &game.directory, control)
                .await
                .map(TaskResult::ModInstall)
                .map_err(|e| e.to_string())
        }
        TaskRequest::CurseForgeModInstall { game_id, plan } => {
            let (game, _operation) = management::begin(app, game_id, true)?;
            let key = crate::curseforge::credential().ok_or("CurseForge 服务尚未配置")?;
            let managed = cobble_core::list_managed_curseforge_mods(&game.directory)
                .map_err(|_| "无法检查已安装的 CurseForge Mod")?;
            if managed
                .iter()
                .any(|item| item.project_id == plan.graph.project_id)
            {
                let update = cobble_core::prepare_curseforge_update(&game.directory, plan)
                    .map_err(|_| "更新方案已变更，请重新预览")?;
                let pack_files: Vec<_> = update
                    .changes
                    .iter()
                    .filter_map(|change| {
                        let previous = update.previous_filename(&change.project_id)?;
                        game.pack_mod_files
                            .iter()
                            .any(|path| path == &format!("mods/{previous}"))
                            .then(|| format!("mods/{}", change.filename))
                    })
                    .collect();
                let outcome = cobble_core::apply_curseforge_updates_controlled(
                    &game.directory,
                    &update,
                    key,
                    control,
                )
                .await
                .map_err(|_| "CurseForge 更新未完成，请重试或恢复更新".to_owned())?;
                if !pack_files.is_empty() {
                    update_game(
                        app,
                        game_id,
                        |game| {
                            for file in pack_files {
                                if !game.pack_mod_files.contains(&file) {
                                    game.pack_mod_files.push(file);
                                }
                            }
                        },
                        true,
                    );
                }
                Ok(TaskResult::ModInstall(
                    cobble_core::ModrinthInstallOutcome {
                        installed: outcome.updated + outcome.added,
                        reused: plan
                            .graph
                            .mods
                            .len()
                            .saturating_sub(outcome.updated + outcome.added),
                    },
                ))
            } else {
                cobble_core::install_curseforge_plan_controlled(plan, &game.directory, key, control)
                    .await
                    .map(TaskResult::ModInstall)
                    .map_err(|_| "CurseForge 安装失败，请检查文件或重试".to_owned())
            }
        }
        TaskRequest::ModUpdate { game_id, plan } => {
            let (game, _operation) = management::begin(app, game_id, true)?;
            let installed = with_game_mods(app, game_id, false, cobble_content::list_mods)?;
            let pack_files: Vec<_> = plan
                .changes
                .iter()
                .filter_map(|change| {
                    let previous = plan.previous_filename(&change.project_id)?;
                    installed
                        .iter()
                        .any(|local| {
                            local.pack_owned
                                && local
                                    .item
                                    .file_name
                                    .strip_suffix(".disabled")
                                    .unwrap_or(&local.item.file_name)
                                    == previous
                        })
                        .then(|| format!("mods/{}", change.filename))
                })
                .collect();
            update_game(app, game_id, |_| {}, false);
            let outcome = if plan.is_curseforge() {
                let key = crate::curseforge::credential().ok_or("CurseForge 服务尚未配置")?;
                cobble_core::apply_curseforge_updates_controlled(
                    &game.directory,
                    plan,
                    key,
                    control,
                )
                .await
            } else {
                cobble_core::apply_modrinth_updates_controlled(&game.directory, plan, control).await
            }
            .map_err(|e| e.to_string())?;
            if !pack_files.is_empty() {
                update_game(
                    app,
                    game_id,
                    |game| {
                        for file in pack_files {
                            if !game.pack_mod_files.contains(&file) {
                                game.pack_mod_files.push(file);
                            }
                        }
                    },
                    true,
                );
            }
            Ok(TaskResult::ModUpdate(outcome))
        }
        TaskRequest::ContentPack {
            game_id,
            kind,
            project_id,
            version_id,
        } => {
            let (game, _operation) = management::begin(app, game_id, true)?;
            update_game(app, game_id, |_| {}, false);
            cobble_core::install_modrinth_content_pack_controlled(
                &game.directory,
                *kind,
                &game.version,
                project_id,
                version_id,
                control,
            )
            .await
            .map(TaskResult::Packs)
            .map_err(|e| e.to_string())
        }
        TaskRequest::ModpackDownload {
            archive_id,
            project_id,
            version_id,
        } => {
            let archive = pack_archive_path(archive_id)?;
            fs::create_dir_all(archive.parent().ok_or("无法定位整合包缓存")?)
                .map_err(|e| e.to_string())?;
            let pack = cobble_core::download_modrinth_pack_controlled(
                project_id, version_id, &archive, control,
            )
            .await
            .map_err(|e| e.to_string())?;
            store_pack_preview(
                app,
                archive_id.clone(),
                pack,
                Some(pack_origin::PackOrigin::modrinth(project_id, version_id)),
            )
            .map(TaskResult::Pack)
        }
        TaskRequest::WorldImport {
            game_id,
            source,
            folder_name,
            replace_existing,
            operation_id,
        } => {
            let (game, _operation) = management::begin(app, game_id, true)?;
            update_game(app, game_id, |_| {}, false);
            if !cobble_core::world_operation_is_complete(&game.directory, operation_id)
                .map_err(|e| e.to_string())?
            {
                source.checked_path()?;
            }
            let outcome = cobble_core::import_world_archive(
                &source.path,
                &game.directory,
                folder_name,
                *replace_existing,
                operation_id,
                control,
            )
            .await
            .map_err(|e| e.to_string())?;
            worlds::restore_result(game.directory, outcome)
                .await
                .map(TaskResult::Worlds)
        }
        TaskRequest::WorldAutomaticBackup { game_id, session } => {
            world_snapshots::automatic::perform(app, game_id, session, control).await?;
            Ok(TaskResult::Game)
        }
        TaskRequest::WorldSnapshot {
            game_id,
            folder_name,
            name,
        } => world_snapshots::perform_create(app, game_id, folder_name, name.clone(), control)
            .await
            .map(TaskResult::Snapshot),
        TaskRequest::WorldSnapshotRestore {
            game_id,
            folder_name,
            snapshot_id,
            operation_id,
        } => world_snapshots::perform_restore(
            app,
            game_id,
            folder_name,
            snapshot_id,
            operation_id,
            control,
        )
        .await
        .map(TaskResult::Worlds),
        TaskRequest::WorldRollback {
            game_id,
            backup_id,
            operation_id,
        } => {
            let (game, _operation) = management::begin(app, game_id, true)?;
            update_game(app, game_id, |_| {}, false);
            let outcome = cobble_core::restore_world_replacement_backup(
                &game.directory,
                backup_id,
                operation_id,
                control,
            )
            .await
            .map_err(|e| e.to_string())?;
            worlds::restore_result(game.directory, outcome)
                .await
                .map(TaskResult::Worlds)
        }
    }
}

#[tauri::command]
pub fn list_tasks(app: AppHandle) -> Result<Vec<TaskSnapshot>, String> {
    let state = app.state::<TaskLibrary>();
    let store = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(error) = &store.load_error {
        return Err(error.clone());
    }
    Ok(store
        .records
        .iter()
        .rev()
        .map(|record| record.snapshot.clone())
        .collect())
}

#[tauri::command]
pub fn cancel_task(app: AppHandle, task_id: String) -> Result<TaskSnapshot, String> {
    let state = app.state::<TaskLibrary>();
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    let record = store
        .records
        .iter_mut()
        .find(|record| record.snapshot.id == task_id)
        .ok_or("找不到这个任务")?;
    let external = matches!(
        record.request,
        TaskRequest::TrialCopy { .. }
            | TaskRequest::GameTransfer { .. }
            | TaskRequest::SharedChanges { .. }
            | TaskRequest::StorageMove { .. }
            | TaskRequest::PackUpdateCopy { .. }
    );
    if record.snapshot.state == TaskState::Queued {
        // External workers may own a sidecar or other cleanup even before admission.
        record.snapshot.state = if external {
            TaskState::Cancelling
        } else {
            TaskState::Cancelled
        };
        record.snapshot.can_retry = matches!(record.request, TaskRequest::GameInstall { .. });
        record.snapshot.updated_at = advance(record.snapshot.updated_at);
    } else if record.snapshot.state == TaskState::Running {
        record.snapshot.state = TaskState::Cancelling;
        record.snapshot.updated_at = advance(record.snapshot.updated_at);
    }
    let snapshot = record.snapshot.clone();
    if let Some(control) = store.controls.get(&task_id) {
        control.cancel();
    }
    if let Some(control) = store.external_controls.get(&task_id) {
        control.store(true, std::sync::atomic::Ordering::Release);
    }
    persist(&store)?;
    drop(store);
    emit(&app, &snapshot);
    state.1.notify_waiters();
    Ok(snapshot)
}

#[tauri::command]
pub fn retry_task(app: AppHandle, task_id: String) -> Result<TaskSnapshot, String> {
    let state = app.state::<TaskLibrary>();
    let mut store = state.0.lock().map_err(|e| e.to_string())?;
    let index = store
        .records
        .iter()
        .position(|record| record.snapshot.id == task_id)
        .ok_or("找不到这个任务")?;
    if !store.records[index].snapshot.can_retry || store.records[index].snapshot.state.active() {
        return Err("这个任务无需重试".into());
    }
    if store
        .records
        .iter()
        .filter(|r| r.snapshot.state.active())
        .count()
        >= 32
    {
        return Err("正在进行的任务已达到上限".into());
    }
    let game_id = store.records[index].request.game_id().map(str::to_owned);
    let same_game_busy = game_id.as_deref().is_some_and(|id| {
        store
            .records
            .iter()
            .any(|r| r.snapshot.game_id.as_deref() == Some(id) && r.snapshot.state.active())
    });
    let migration = matches!(
        store.records[index].request,
        TaskRequest::DirectoryMigration { .. }
    );
    if migration
        && store.records.iter().enumerate().any(|(i, r)| {
            i != index
                && matches!(r.request, TaskRequest::DirectoryMigration { .. })
                && r.snapshot.state.active()
        })
    {
        return Err("DIRECTORY_MIGRATION_BUSY".into());
    }
    let queued = should_queue(&store.records, game_id.as_deref())
        || migration && store.records.iter().any(|r| r.snapshot.state.active());
    if let Some(game_id) = &game_id {
        let library = app.state::<GameLibrary>();
        let library = library.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| &game.id == game_id)
            .ok_or("找不到这个游戏")?;
        if game.launch_state != "idle"
            || !same_game_busy && (game.content_installing || game.status == "installing")
        {
            return Err("请先关闭游戏或等待当前操作完成".into());
        }
    }
    let game_install = matches!(
        store.records[index].request,
        TaskRequest::GameInstall { .. }
    );
    let previous = store.records[index].snapshot.clone();
    let resume_java = store.records[index].request.clone();
    let snapshot = &mut store.records[index].snapshot;
    snapshot.state = if queued {
        TaskState::Queued
    } else {
        TaskState::Running
    };
    snapshot.error = None;
    snapshot.can_retry = false;
    snapshot.progress = None;
    snapshot.updated_at = advance(snapshot.updated_at);
    let snapshot = snapshot.clone();
    if let Err(error) = persist(&store) {
        store.records[index].snapshot = previous;
        return Err(error);
    }
    drop(store);
    if game_install && let Some(game_id) = &game_id {
        update_game(
            &app,
            game_id,
            |game| {
                game.status = "installing".into();
                game.error = None;
                game.progress = None;
            },
            true,
        );
    }
    emit(&app, &snapshot);
    tauri::async_runtime::spawn(async move {
        let result = execute(app.clone(), task_id).await;
        if let (
            Ok(TaskResult::Java(runtime)),
            TaskRequest::GameJava {
                game_id,
                generation,
                profile_id,
                server_id,
                ..
            },
        ) = (result, resume_java)
        {
            let library = app.state::<GameLibrary>();
            let _ = runtime::start_game_captured(
                app.clone(),
                library,
                game_id,
                server_id,
                profile_id,
                runtime.executable,
                generation,
            );
        }
    });
    Ok(snapshot)
}

#[tauri::command]
pub fn get_task_pack_preview(app: AppHandle, task_id: String) -> Result<PackPreview, String> {
    let request = {
        let state = app.state::<TaskLibrary>();
        let store = state.0.lock().map_err(|e| e.to_string())?;
        let record = store
            .records
            .iter()
            .find(|record| record.snapshot.id == task_id)
            .ok_or("找不到这个任务")?;
        if record.snapshot.state != TaskState::Succeeded || !record.snapshot.has_result {
            return Err("整合包尚未下载完成".into());
        }
        record.request.clone()
    };
    let origin = request.pack_origin();
    let TaskRequest::ModpackDownload { archive_id, .. } = request else {
        return Err("这个任务没有整合包预览".into());
    };
    let pack =
        cobble_content::read_mrpack(&pack_archive_path(&archive_id)?).map_err(|e| e.to_string())?;
    store_pack_preview(&app, archive_id, pack, origin)
}

pub(super) fn curseforge_pack_result_id(
    app: &AppHandle,
    task_id: &str,
) -> Result<(String, pack_origin::PackOrigin), String> {
    let state = app.state::<TaskLibrary>();
    let store = state.0.lock().map_err(|_| "任务记录不可用")?;
    let record = store
        .records
        .iter()
        .find(|record| record.snapshot.id == task_id)
        .ok_or("找不到这个任务")?;
    if record.snapshot.state != TaskState::Succeeded || !record.snapshot.has_result {
        return Err("整合包尚未下载完成".into());
    }
    match &record.request {
        TaskRequest::CurseForgePackDownload {
            archive_id,
            project_id,
            file_id,
        } => Ok((
            archive_id.clone(),
            pack_origin::PackOrigin::curseforge(*project_id, *file_id),
        )),
        _ => Err("这个任务没有 CurseForge 整合包预览".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initial_plan() -> cobble_content::ModrinthPlan {
        use sha1::Digest;
        cobble_content::ModrinthPlan {
            project_id: "fixture".into(),
            version_id: "selected".into(),
            mods: vec![cobble_content::ResolvedMod {
                project_id: "fixture".into(),
                version_id: "selected".into(),
                name: "Fixture".into(),
                version_number: "release".into(),
                required_by: None,
                file: cobble_content::ModrinthFile {
                    url: "https://cdn.modrinth.com/data/fixture/versions/selected/fixture.jar"
                        .into(),
                    filename: "fixture.jar".into(),
                    size: 7,
                    sha1: format!("{:x}", sha1::Sha1::digest(b"fixture")),
                    sha512: None,
                },
            }],
        }
    }

    #[test]
    fn initial_mod_plan_survives_disk_history_restart_and_legacy_requests_decode() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.json");
        let mut task = record(TaskState::Running);
        task.request = TaskRequest::GameInstall {
            game_id: "game".into(),
            initial_mod_plan: Some(initial_plan()),
            initial_environment_plan: None,
            initial_shared_configs: Vec::new(),
            initial_personal_options: None,
        };
        let request_before = serde_json::to_value(&task.request).unwrap();
        persist_records(&path, &[task]).unwrap();
        let mut loaded: Vec<StoredTask> = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        recover_records(&mut loaded);
        assert_eq!(loaded[0].snapshot.state, TaskState::Interrupted);
        assert!(loaded[0].snapshot.can_retry);
        assert_eq!(
            serde_json::to_value(&loaded[0].request).unwrap(),
            request_before
        );
        let public = serde_json::to_value(&loaded[0].snapshot).unwrap();
        assert!(public.get("initial_mod_plan").is_none());
        let legacy: TaskRequest = serde_json::from_value(serde_json::json!({
            "operation":"game_install", "game_id":"old"
        }))
        .unwrap();
        assert!(matches!(
            legacy,
            TaskRequest::GameInstall {
                initial_mod_plan: None,
                ..
            }
        ));
    }

    #[test]
    fn initial_environment_plan_survives_restart_retry_and_history_pruning() {
        let graph = initial_plan();
        let environment = cobble_content::EnvironmentPlan {
            game_version: "1.21.1".into(),
            loader: "fabric".into(),
            roots: vec![cobble_content::EnvironmentSelection {
                source: cobble_content::EnvironmentSource::Modrinth,
                project_id: graph.project_id.clone(),
                version_id: graph.version_id.clone(),
            }],
            modrinth: Some(graph),
            curseforge: None,
            dependency_edges: vec![],
        };
        let mut task = record(TaskState::Running);
        task.request = TaskRequest::GameInstall {
            game_id: "game".into(),
            initial_mod_plan: None,
            initial_environment_plan: Some(environment),
            initial_shared_configs: Vec::new(),
            initial_personal_options: None,
        };
        let expected = serde_json::to_value(&task.request).unwrap();
        let mut loaded: Vec<StoredTask> =
            serde_json::from_slice(&serde_json::to_vec(&vec![task]).unwrap()).unwrap();
        recover_records(&mut loaded);
        assert_eq!(loaded[0].snapshot.state, TaskState::Interrupted);
        assert!(loaded[0].snapshot.can_retry);
        assert!(!removable_history(&loaded[0]));
        assert_eq!(serde_json::to_value(&loaded[0].request).unwrap(), expected);
        loaded[0].snapshot.state = TaskState::Running;
        assert_eq!(serde_json::to_value(&loaded[0].request).unwrap(), expected);
        loaded[0].snapshot.state = TaskState::Succeeded;
        assert!(removable_history(&loaded[0]));
    }

    #[test]
    fn builtin_copy_restart_keeps_host_parent_target_identity_and_world_choice() {
        let parent = tempfile::tempdir().unwrap();
        let mut task = record(TaskState::Running);
        task.request = TaskRequest::GameCopy {
            game_id: "game".into(),
            target_id: "a".repeat(32),
            target_parent: parent.path().to_owned(),
            name: "Independent copy".into(),
            include_worlds: false,
        };
        let expected = serde_json::to_value(&task.request).unwrap();
        let mut loaded: Vec<StoredTask> =
            serde_json::from_slice(&serde_json::to_vec(&vec![task]).unwrap()).unwrap();
        recover_records(&mut loaded);
        assert!(loaded[0].snapshot.can_retry);
        assert_eq!(loaded[0].snapshot.state, TaskState::Interrupted);
        assert!(!removable_history(&loaded[0]));
        assert_eq!(serde_json::to_value(&loaded[0].request).unwrap(), expected);
        assert!(
            serde_json::to_value(&loaded[0].snapshot)
                .unwrap()
                .get("target_parent")
                .is_none()
        );
    }

    #[test]
    fn shared_configuration_only_creation_is_durable_and_retained_after_interruption() {
        use sha2::Digest;
        let mut task = record(TaskState::Running);
        let configs = vec![cobble_core::environment_install::SharedConfig {
            path: "config/selected.json".into(),
            text: "{}".into(),
            sha256: format!("{:x}", sha2::Sha256::digest(b"{}")),
        }];
        task.request = TaskRequest::GameInstall {
            game_id: "game".into(),
            initial_mod_plan: None,
            initial_environment_plan: None,
            initial_shared_configs: configs,
            initial_personal_options: None,
        };
        let expected = serde_json::to_value(&task.request).unwrap();
        let mut loaded: Vec<StoredTask> =
            serde_json::from_slice(&serde_json::to_vec(&vec![task]).unwrap()).unwrap();
        recover_records(&mut loaded);
        assert!(loaded[0].snapshot.can_retry);
        assert!(!removable_history(&loaded[0]));
        assert_eq!(serde_json::to_value(&loaded[0].request).unwrap(), expected);
    }

    #[test]
    fn personal_options_only_creation_survives_restart_cancel_failure_and_history_pruning() {
        let options = serde_json::from_value(serde_json::json!({
            "schema_version":1,"source_version":"1.21.1","target_version":"1.21.1",
            "options_version":3955,"fields":{"renderDistance":"12"}
        }))
        .unwrap();
        let mut task = record(TaskState::Running);
        task.request = TaskRequest::GameInstall {
            game_id: "game".into(),
            initial_mod_plan: None,
            initial_environment_plan: None,
            initial_shared_configs: vec![],
            initial_personal_options: Some(options),
        };
        let expected = serde_json::to_value(&task.request).unwrap();
        let mut loaded: Vec<StoredTask> =
            serde_json::from_slice(&serde_json::to_vec(&vec![task]).unwrap()).unwrap();
        recover_records(&mut loaded);
        assert_eq!(loaded[0].snapshot.state, TaskState::Interrupted);
        assert!(loaded[0].snapshot.can_retry);
        assert_eq!(serde_json::to_value(&loaded[0].request).unwrap(), expected);
        for state in [
            TaskState::Failed,
            TaskState::Cancelled,
            TaskState::Interrupted,
        ] {
            loaded[0].snapshot.state = state;
            assert!(!removable_history(&loaded[0]));
        }
        loaded[0].snapshot.state = TaskState::Succeeded;
        assert!(removable_history(&loaded[0]));
    }

    #[test]
    fn unfinished_initial_mod_install_is_retained_when_history_is_pruned() {
        for state in [
            TaskState::Failed,
            TaskState::Cancelled,
            TaskState::Interrupted,
        ] {
            let mut task = record(state);
            task.request = TaskRequest::GameInstall {
                game_id: "game".into(),
                initial_mod_plan: Some(initial_plan()),
                initial_environment_plan: None,
                initial_shared_configs: Vec::new(),
                initial_personal_options: None,
            };
            assert!(!removable_history(&task));
            task.snapshot.state = TaskState::Succeeded;
            assert!(removable_history(&task));
        }
        assert!(removable_history(&record(TaskState::Succeeded)));
    }

    #[tokio::test]
    async fn initial_mod_stage_never_runs_after_base_failure() {
        let directory = tempfile::tempdir().unwrap();
        let result = initial_mods_after_base(
            async { Err("base failed".into()) },
            Some(&initial_plan()),
            directory.path(),
            OperationControl::new(Arc::new(|_| {})),
        )
        .await;
        assert!(matches!(result, Err(error) if error == "base failed"));
        assert!(!directory.path().join("mods").exists());
    }

    #[tokio::test]
    async fn initial_mod_stage_failure_or_cancellation_cannot_complete_game_install() {
        let directory = tempfile::tempdir().unwrap();
        let control = OperationControl::new(Arc::new(|_| {}));
        let cancelled = control.clone();
        let result = initial_mods_after_base(
            async move {
                cancelled.cancel();
                Ok(())
            },
            Some(&initial_plan()),
            directory.path(),
            control,
        )
        .await;
        assert!(matches!(result, Err(error) if error.contains("取消")));
        assert!(!directory.path().join("mods").exists());
        let mut plan = initial_plan();
        plan.mods[0].file.sha1 = "invalid".into();
        let result = initial_mods_after_base(
            async { Ok(()) },
            Some(&plan),
            directory.path(),
            OperationControl::new(Arc::new(|_| {})),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(terminal_state(result.is_ok(), false), TaskState::Failed);
    }

    #[tokio::test]
    async fn initial_mod_stage_completes_only_after_exact_plan_is_committed_and_can_resume() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("mods")).unwrap();
        fs::write(directory.path().join("mods/fixture.jar"), b"fixture").unwrap();
        let plan = initial_plan();
        for _ in 0..2 {
            let result = initial_mods_after_base(
                async { Ok(()) },
                Some(&plan),
                directory.path(),
                OperationControl::new(Arc::new(|_| {})),
            )
            .await;
            assert!(matches!(result, Ok(TaskResult::Game)));
            let managed = cobble_core::list_managed_modrinth_mods(directory.path()).unwrap();
            assert_eq!(managed.len(), 1);
            assert_eq!(managed[0].version_id, "selected");
        }
    }

    fn record(state: TaskState) -> StoredTask {
        StoredTask {
            snapshot: TaskSnapshot {
                id: "fixture".into(),
                game_id: Some("game".into()),
                world_folder: None,
                game_name: "Fixture".into(),
                title: "安装游戏".into(),
                kind: "game_install".into(),
                state,
                progress: Some(InstallProgress {
                    detail: None,
                    phase: "assets".into(),
                    completed: 2,
                    total: 10,
                    reused: 1,
                }),
                error: None,
                can_retry: false,
                created_at: 1,
                updated_at: 2,
                has_result: false,
            },
            request: TaskRequest::GameInstall {
                game_id: "game".into(),
                initial_mod_plan: None,
                initial_environment_plan: None,
                initial_shared_configs: Vec::new(),
                initial_personal_options: None,
            },
        }
    }

    #[test]
    fn same_game_queue_is_fifo_and_cancelled_head_releases_next() {
        let mut first = record(TaskState::Running);
        first.snapshot.id = "first".into();
        let mut second = record(TaskState::Queued);
        second.snapshot.id = "second".into();
        let mut third = record(TaskState::Queued);
        third.snapshot.id = "third".into();
        let mut records = vec![first, second, third];
        assert!(TaskState::Queued.active());
        assert!(should_queue(&records[..1], Some("game")));
        assert!(!can_start(&records, 1));
        records[0].snapshot.state = TaskState::Succeeded;
        assert!(can_start(&records, 1));
        assert!(!can_start(&records, 2));
        records[1].snapshot.state = TaskState::Cancelled;
        assert!(can_start(&records, 2));
    }

    #[test]
    fn directory_migration_is_an_exclusive_fifo_barrier_and_recovers_for_retry() {
        let plan = serde_json::from_value(serde_json::json!({"id":"abc123","source":"C:/old/games","target":"D:/new/games","entries":[]})).unwrap();
        let mut migration = record(TaskState::Queued);
        migration.snapshot.game_id = None;
        migration.request = TaskRequest::DirectoryMigration { plan };
        let mut later = record(TaskState::Queued);
        later.snapshot.game_id = Some("other".into());
        let mut records = vec![record(TaskState::Running), migration, later];
        assert!(should_queue(&records, Some("new")));
        assert!(!can_start(&records, 1));
        assert!(!can_start(&records, 2));
        records[0].snapshot.state = TaskState::Succeeded;
        assert!(can_start(&records, 1));
        records[1].snapshot.state = TaskState::Running;
        assert!(!can_start(&records, 2));
        let serialized = serde_json::to_vec(&records).unwrap();
        let mut recovered: Vec<StoredTask> = serde_json::from_slice(&serialized).unwrap();
        recover_records(&mut recovered);
        assert_eq!(recovered[1].snapshot.state, TaskState::Interrupted);
        assert!(recovered[1].snapshot.can_retry);
        assert!(!removable_history(&recovered[1]));
        records[1].snapshot.state = TaskState::Cancelled;
        assert!(can_start(&records, 2));
    }

    #[test]
    fn four_running_slots_allow_a_different_game_after_one_finishes() {
        let mut records = (0..4)
            .map(|index| {
                let mut task = record(TaskState::Running);
                task.snapshot.id = format!("running-{index}");
                task.snapshot.game_id = Some(format!("game-{index}"));
                task
            })
            .collect::<Vec<_>>();
        let mut next = record(TaskState::Queued);
        next.snapshot.id = "next".into();
        next.snapshot.game_id = Some("other".into());
        assert!(should_queue(&records, Some("other")));
        records.push(next);
        assert!(!can_start(&records, 4));
        records[0].snapshot.state = TaskState::Succeeded;
        assert!(can_start(&records, 4));
    }

    #[test]
    fn snapshot_restore_history_keeps_world_scope_and_journal_identity() {
        let mut value = record(TaskState::Running);
        value.request = TaskRequest::WorldSnapshotRestore {
            game_id: "game".into(),
            folder_name: "世界 1".into(),
            snapshot_id: "a".repeat(64),
            operation_id: "owned-operation".into(),
        };
        value.snapshot.kind = value.request.kind().into();
        value.snapshot.world_folder = Some("世界 1".into());
        let mut loaded: Vec<StoredTask> =
            serde_json::from_slice(&serde_json::to_vec(&vec![value]).unwrap()).unwrap();
        recover_records(&mut loaded);
        assert_eq!(loaded[0].snapshot.state, TaskState::Interrupted);
        assert_eq!(loaded[0].snapshot.world_folder.as_deref(), Some("世界 1"));
        assert_eq!(loaded[0].snapshot.kind, "world_snapshot_restore");
        assert!(
            matches!(&loaded[0].request, TaskRequest::WorldSnapshotRestore { operation_id, .. } if operation_id == "owned-operation")
        );
        let public = serde_json::to_value(&loaded[0].snapshot).unwrap();
        assert!(public.get("snapshot_id").is_none());
        assert!(public.get("password").is_none());
    }

    #[test]
    fn history_replacement_and_restart_keep_requests_and_do_not_restart_jobs() {
        let directory = std::env::temp_dir().join(format!(
            "cobble-task-history-{}-{}",
            std::process::id(),
            next_pack_id().unwrap()
        ));
        fs::create_dir(&directory).unwrap();
        let file = directory.join("tasks.json");
        persist_records(&file, &[record(TaskState::Succeeded)]).unwrap();
        persist_records(
            &file,
            &[record(TaskState::Cancelling), record(TaskState::Succeeded)],
        )
        .unwrap();
        // An incomplete future write does not replace the last complete history.
        fs::write(file.with_extension("json.tmp"), b"incomplete").unwrap();
        let mut loaded: Vec<StoredTask> =
            serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        recover_records(&mut loaded);
        assert_eq!(loaded[0].snapshot.state, TaskState::Interrupted);
        assert!(loaded[0].snapshot.can_retry);
        assert_eq!(loaded[0].request.game_id(), Some("game"));
        assert_eq!(loaded[0].snapshot.progress.as_ref().unwrap().completed, 2);
        assert_eq!(loaded[1].snapshot.state, TaskState::Succeeded);
        let public = serde_json::to_value(&loaded[0].snapshot).unwrap();
        assert!(public.get("request").is_none());
        assert!(public.get("directory").is_none());
        fs::remove_file(file.with_extension("json.tmp")).unwrap();
        fs::remove_file(file).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn internal_worker_stop_is_failure_and_completed_commit_wins_late_cancel() {
        assert_eq!(terminal_state(false, false), TaskState::Failed);
        assert_eq!(terminal_state(false, true), TaskState::Cancelled);
        assert_eq!(terminal_state(true, true), TaskState::Succeeded);
        let stamp = now();
        assert!(advance(stamp) > stamp);
    }

    #[test]
    fn interrupted_external_copy_requires_inspection_not_automatic_retry() {
        let mut copy = record(TaskState::Running);
        copy.request = TaskRequest::TrialCopy {
            game_id: "game".into(),
        };
        copy.snapshot.kind = copy.request.kind().into();
        let mut move_task = record(TaskState::Cancelling);
        move_task.request = TaskRequest::StorageMove {
            game_id: "game".into(),
        };
        move_task.snapshot.kind = move_task.request.kind().into();
        let mut pack_copy = record(TaskState::Running);
        pack_copy.request = TaskRequest::PackUpdateCopy {
            game_id: "game".into(),
            target_id: "a".repeat(32),
            archive_id: "b".repeat(32),
        };
        pack_copy.snapshot.kind = pack_copy.request.kind().into();
        let mut transfer = record(TaskState::Running);
        transfer.request = TaskRequest::GameTransfer {
            game_id: "game".into(),
        };
        transfer.snapshot.kind = transfer.request.kind().into();
        let mut changes = record(TaskState::Running);
        changes.request = TaskRequest::SharedChanges {
            game_id: "game".into(),
        };
        changes.snapshot.kind = changes.request.kind().into();
        let mut records = [copy, move_task, pack_copy, transfer, changes];
        recover_records(&mut records);
        for record in &records {
            assert_eq!(record.snapshot.state, TaskState::Interrupted);
            assert!(!record.snapshot.can_retry);
            assert!(record.snapshot.error.as_deref().unwrap().contains("检查"));
        }
    }

    #[test]
    fn interrupted_pack_cleanup_requires_exact_unpublished_owned_marker() {
        let base = std::env::temp_dir().join(format!(
            "cobble-pack-cleanup-{}-{}",
            std::process::id(),
            now()
        ));
        fs::create_dir(&base).unwrap();
        let target_id = "a".repeat(32);
        let archive_id = "b".repeat(32);
        let target = base.join(&target_id);
        fs::create_dir(&target).unwrap();
        fs::write(target.join("synthetic.txt"), b"fixture").unwrap();
        let marker = base.join(format!(".cobble-pack-update-{target_id}.json"));
        let receipt = serde_json::json!({"taskId":"task","sourceGameId":"source","targetId":target_id,"archiveId":archive_id});
        fs::write(&marker, serde_json::to_vec(&receipt).unwrap()).unwrap();
        let registered = vec![(target_id.clone(), target.clone())];
        assert!(
            !cleanup_pack_copy_target(
                &base,
                &registered,
                "task",
                "source",
                &target_id,
                &archive_id
            )
            .unwrap()
        );
        assert!(target.exists());
        assert!(
            !cleanup_pack_copy_target(&base, &[], "other-task", "source", &target_id, &archive_id)
                .unwrap()
        );
        assert!(target.exists());
        assert!(
            cleanup_pack_copy_target(&base, &[], "task", "source", &target_id, &archive_id)
                .unwrap()
        );
        assert!(!target.exists());
        assert!(!marker.exists());
        fs::remove_dir(base).unwrap();
    }
}
