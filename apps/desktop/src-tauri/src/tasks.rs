// SPDX-License-Identifier: MIT
//! Local task history and controls. Requests and paths remain in the host.
use super::*;
use cobble_core::OperationControl;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}
impl TaskState {
    fn active(self) -> bool {
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
    },
    GameRepair {
        game_id: String,
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
}
impl TaskRequest {
    fn game_id(&self) -> Option<&str> {
        match self {
            Self::GameInstall { game_id }
            | Self::GameRepair { game_id }
            | Self::ModInstall { game_id, .. }
            | Self::CurseForgeModInstall { game_id, .. }
            | Self::ModUpdate { game_id, .. }
            | Self::ContentPack { game_id, .. }
            | Self::WorldImport { game_id, .. }
            | Self::WorldRollback { game_id, .. }
            | Self::WorldAutomaticBackup { game_id, .. }
            | Self::WorldSnapshot { game_id, .. }
            | Self::WorldSnapshotRestore { game_id, .. } => Some(game_id),
            Self::ModpackDownload { .. } | Self::CurseForgePackDownload { .. } => None,
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::GameInstall { .. } => "game_install",
            Self::GameRepair { .. } => "game_repair",
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
    load_error: Option<String>,
}
pub struct TaskLibrary(Mutex<TaskStore>);

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
            record.snapshot.can_retry = true;
            record.snapshot.error = Some("上次操作已中断，可以继续".into());
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
        Self(Mutex::new(store))
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
    if game_id.is_some()
        && store
            .records
            .iter()
            .any(|record| record.snapshot.game_id == game_id && record.snapshot.state.active())
    {
        return Err("这个游戏已有任务正在进行".into());
    }
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
        state: TaskState::Running,
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
        let Some(index) = store
            .records
            .iter()
            .position(|record| !record.snapshot.state.active())
        else {
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
    execute(app, id).await
}

pub(super) async fn execute(app: AppHandle, id: String) -> Result<TaskResult, String> {
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
    if let TaskRequest::GameInstall { game_id } = &request {
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
    result
}

async fn perform(
    app: &AppHandle,
    request: &TaskRequest,
    control: OperationControl,
) -> Result<TaskResult, String> {
    match request {
        TaskRequest::GameRepair { game_id } => {
            let (game, _operation) = management::begin(app, game_id, true)?;
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
        TaskRequest::GameInstall { game_id } => {
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
            if let Some(marker) = marker
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
                return Ok(TaskResult::Game);
            }
            let settings = game
                .download_source
                .clone()
                .map(Ok)
                .unwrap_or_else(cobble_minecraft::read_download_settings)
                .map_err(|e| e.to_string())?;
            let source = settings.download_source().map_err(|e| e.to_string())?;
            if game.launcher_import_id.is_some() {
                launcher_import::install(&game, source, control).await?;
            } else if let Some(source_id) = &game.pack_source_id {
                if curseforge_pack::is_source(source_id)? {
                    curseforge_pack::install(&game, source, control).await?;
                } else {
                    cobble_core::install_mrpack_controlled(
                        &pack_archive_path(source_id)?,
                        &game.directory,
                        source,
                        &game.pack_optional_files,
                        control,
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
                                control,
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
                                control,
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
                            control,
                        )
                        .await
                    }
                }
                .map_err(|e| e.to_string())?;
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
            let outcome =
                cobble_core::apply_modrinth_updates_controlled(&game.directory, plan, control)
                    .await
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
    if record.snapshot.state == TaskState::Running {
        record.snapshot.state = TaskState::Cancelling;
        record.snapshot.updated_at = advance(record.snapshot.updated_at);
    }
    let snapshot = record.snapshot.clone();
    if let Some(control) = store.controls.get(&task_id) {
        control.cancel();
    }
    persist(&store)?;
    drop(store);
    emit(&app, &snapshot);
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
    let game_id = store.records[index].request.game_id().map(str::to_owned);
    if game_id.is_some()
        && store
            .records
            .iter()
            .any(|record| record.snapshot.game_id == game_id && record.snapshot.state.active())
    {
        return Err("这个游戏已有任务正在进行".into());
    }
    if let Some(game_id) = &game_id {
        let library = app.state::<GameLibrary>();
        let library = library.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| &game.id == game_id)
            .ok_or("找不到这个游戏")?;
        if game.launch_state != "idle" || game.content_installing || game.status == "installing" {
            return Err("请先关闭游戏或等待当前操作完成".into());
        }
    }
    let game_install = matches!(
        store.records[index].request,
        TaskRequest::GameInstall { .. }
    );
    let previous = store.records[index].snapshot.clone();
    let snapshot = &mut store.records[index].snapshot;
    snapshot.state = TaskState::Running;
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
        let _ = execute(app, task_id).await;
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
            },
        }
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
}
