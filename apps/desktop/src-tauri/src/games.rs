// SPDX-License-Identifier: MIT
#[path = "blueprint_import.rs"]
pub mod blueprint_import;
#[path = "native_builtin_copy.rs"]
pub(crate) mod builtin_copy;
#[path = "curseforge_install.rs"]
pub mod curseforge_install;
#[path = "curseforge_pack.rs"]
pub mod curseforge_pack;
#[path = "game_directory_migration.rs"]
pub mod directory_migration;
#[path = "file_inbox_games.rs"]
pub(crate) mod file_inbox_games;
#[path = "game_appearance.rs"]
pub mod game_appearance;
#[path = "game_config.rs"]
pub mod game_config;
#[path = "game_diagnostics.rs"]
pub mod game_diagnostics;
#[path = "game_inspection.rs"]
pub mod game_inspection;
#[path = "game_servers.rs"]
pub mod game_servers;
#[path = "launch_recovery.rs"]
pub mod launch_recovery;
#[path = "launcher_import.rs"]
pub mod launcher_import;
#[path = "local_packs.rs"]
pub mod local_packs;
#[path = "management.rs"]
pub mod management;
#[path = "marketplace.rs"]
pub mod marketplace;
#[path = "mesh.rs"]
pub mod mesh;
#[path = "mod_updates.rs"]
pub mod mod_updates;
#[path = "native_readiness.rs"]
pub mod native_readiness;
#[path = "native_readiness_space.rs"]
mod native_readiness_space;
#[path = "native_reverse.rs"]
pub mod native_reverse;
#[path = "native_reverse_install.rs"]
pub mod native_reverse_install;
#[path = "pack_origin.rs"]
pub mod pack_origin;
#[path = "pack_update_copy.rs"]
pub mod pack_update_copy;
#[path = "native_pack_update_preview.rs"]
pub mod pack_update_preview;
#[path = "quick_launch.rs"]
pub mod quick_launch;
#[path = "reverse_config.rs"]
pub mod reverse_config;
#[path = "runtime.rs"]
pub mod runtime;
#[path = "screenshot_sheet.rs"]
pub mod screenshot_sheet;
#[path = "screenshots.rs"]
pub mod screenshots;
#[path = "sessions.rs"]
pub mod sessions;
#[path = "storage_workspace.rs"]
pub mod storage_workspace;
#[path = "tasks.rs"]
pub mod tasks;
#[path = "trial_copy.rs"]
pub(crate) mod trial_copy;
#[path = "game_workspace.rs"]
pub(crate) mod workspace;
#[path = "world_datapacks.rs"]
pub mod world_datapacks;
#[path = "worlds.rs"]
pub mod worlds;
use cobble_core::InstallProgress;
use cobble_minecraft::{DownloadSettings, VersionCatalog, VersionKind};
#[path = "world_snapshots.rs"]
pub mod world_snapshots;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

#[derive(Clone, Deserialize, Serialize)]
pub struct Game {
    id: String,
    name: String,
    #[serde(default)]
    last_launched_at: Option<u64>,
    #[serde(default = "game_appearance::default_preset")]
    appearance_preset: String,
    #[serde(default)]
    icon_ref: Option<String>,
    #[serde(default)]
    cover_ref: Option<String>,
    #[serde(default)]
    pinned: bool,
    version: String,
    #[serde(default)]
    loader_version: Option<String>,
    #[serde(default)]
    loader_kind: Option<String>,
    directory: PathBuf,
    #[serde(default)]
    profile_id: Option<String>,
    #[serde(default)]
    runtime_overrides: cobble_minecraft::LaunchOverrides,
    #[serde(default)]
    download_source: Option<DownloadSettings>,
    status: String,
    error: Option<String>,
    #[serde(skip_deserializing, default)]
    progress: Option<InstallProgress>,
    #[serde(skip_deserializing, default = "idle")]
    launch_state: String,
    #[serde(default)]
    launch_error: Option<String>,
    #[serde(default)]
    log_path: Option<PathBuf>,
    #[serde(skip_deserializing, default)]
    content_installing: bool,
    #[serde(default)]
    pack_name: Option<String>,
    #[serde(default)]
    pack_version: Option<String>,
    #[serde(default)]
    pack_source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pack_origin: Option<pack_origin::PackOrigin>,
    #[serde(default)]
    pack_optional_files: Vec<String>,
    #[serde(default)]
    pack_mod_files: Vec<String>,
    #[serde(default)]
    launcher_import_id: Option<String>,
}

fn idle() -> String {
    "idle".into()
}

struct PackSelection {
    id: String,
    pack: cobble_content::Mrpack,
    origin: Option<pack_origin::PackOrigin>,
}

fn pack_archive_path(id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("无效的整合包文件记录".into());
    }
    Ok(dirs::config_dir()
        .ok_or("无法找到配置目录")?
        .join("CobbleLauncher/pack-sources")
        .join(format!("{id}.mrpack")))
}

#[derive(Serialize)]
pub struct PackPreview {
    preview_id: String,
    name: String,
    version_id: String,
    summary: Option<String>,
    minecraft_version: String,
    loader_version: Option<String>,
    loader_kind: Option<String>,
    required_files: usize,
    optional_files: Vec<PackOptionalFile>,
    overrides: usize,
}
#[derive(Serialize)]
pub struct PackOptionalFile {
    path: String,
    size: u64,
}

fn next_pack_id() -> Result<String, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    Ok(format!(
        "{:x}{:016x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn store_pack_preview(
    app: &AppHandle,
    id: String,
    pack: cobble_content::Mrpack,
    origin: Option<pack_origin::PackOrigin>,
) -> Result<PackPreview, String> {
    let preview = PackPreview {
        preview_id: id.clone(),
        name: pack.name.clone(),
        version_id: pack.version_id.clone(),
        summary: pack.summary.clone(),
        minecraft_version: pack.minecraft_version.clone(),
        loader_version: pack.loader_version.clone(),
        loader_kind: pack.loader_kind.clone(),
        required_files: pack.files.iter().filter(|file| !file.optional).count(),
        optional_files: pack
            .files
            .iter()
            .filter(|file| file.optional)
            .map(|file| PackOptionalFile {
                path: file.path.clone(),
                size: file.file_size,
            })
            .collect(),
        overrides: pack.overrides.len(),
    };
    app.state::<GameLibrary>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .picked_pack = Some(PackSelection { id, pack, origin });
    Ok(preview)
}

#[tauri::command]
pub async fn preview_mrpack(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<Option<PackPreview>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("选择整合包")
        .add_filter("Modrinth 整合包", &["mrpack"])
        .set_parent(&window)
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let source = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let id = next_pack_id()?;
        let archive = pack_archive_path(&id)?;
        fs::create_dir_all(archive.parent().expect("archive parent")).map_err(|e| e.to_string())?;
        let mut created = false;
        let result = (|| {
            let mut input = fs::File::open(&source).map_err(|e| e.to_string())?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&archive)
                .map_err(|e| e.to_string())?;
            created = true;
            std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
            drop(output);
            cobble_content::read_mrpack(&archive).map_err(|e| e.to_string())
        })();
        let pack = match result {
            Ok(pack) => pack,
            Err(error) => {
                if created {
                    let _ = fs::remove_file(&archive);
                }
                return Err(error);
            }
        };
        store_pack_preview(&app, id, pack, None).map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn create_mrpack_game(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    name: String,
    preview_id: String,
    source: DownloadSettings,
    use_default_directory: bool,
    optional_files: Vec<String>,
) -> Result<Game, String> {
    if name.trim().is_empty() {
        return Err("请填写游戏名称".into());
    }
    let resolved_source = source.download_source().map_err(|e| e.to_string())?;
    let mut library = library.0.lock().map_err(|e| e.to_string())?;
    if let Some(error) = &library.load_error {
        return Err(error.clone());
    }
    if library.games.iter().any(|g| g.status == "installing") {
        return Err("已有游戏正在安装，请稍候".into());
    }
    let selection = library.picked_pack.as_ref().ok_or("请先选择整合包")?;
    if selection.id != preview_id {
        return Err("整合包已变更，请重新查看预览".into());
    }
    if optional_files.iter().any(|path| {
        !selection
            .pack
            .files
            .iter()
            .any(|file| file.optional && &file.path == path)
    }) {
        return Err("请选择整合包中提供的可选内容".into());
    }
    let base = if use_default_directory {
        default_directory()?
    } else {
        library.picked_directory.clone().ok_or("请先选择保存位置")?
    };
    let id = format!(
        "{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let game = Game {
        directory: base.join(&id),
        id,
        name: name.trim().into(),
        last_launched_at: None,
        appearance_preset: game_appearance::default_preset(),
        icon_ref: None,
        cover_ref: None,
        pinned: false,
        version: selection.pack.minecraft_version.clone(),
        loader_version: selection.pack.loader_version.clone(),
        loader_kind: selection.pack.loader_kind.clone(),
        profile_id: None,
        runtime_overrides: cobble_minecraft::LaunchOverrides::default(),
        download_source: Some(source.clone()),
        status: "installing".into(),
        error: None,
        progress: None,
        launch_state: idle(),
        launch_error: None,
        log_path: None,
        content_installing: false,
        pack_name: Some(selection.pack.name.clone()),
        pack_version: Some(selection.pack.version_id.clone()),
        pack_source_id: Some(selection.id.clone()),
        pack_origin: selection.origin.clone(),
        pack_optional_files: optional_files,
        pack_mod_files: Vec::new(),
        launcher_import_id: None,
    };
    library.games.push(game.clone());
    if let Err(error) = save(&library.games) {
        library.games.pop();
        return Err(error);
    }
    drop(library);
    start_install(app, game.clone(), resolved_source);
    Ok(game)
}

#[derive(Default)]
struct Library {
    games: Vec<Game>,
    picked_directory: Option<PathBuf>,
    load_error: Option<String>,
    mod_plans: HashMap<String, (String, cobble_content::ModrinthPlan)>,
    mod_preview_ids: HashMap<String, String>,
    curseforge_plans: HashMap<String, (String, cobble_content::curseforge::InstallPlan)>,
    curseforge_preview_ids: HashMap<String, String>,
    update_plans: HashMap<String, (String, cobble_core::ModUpdatePlan)>,
    picked_pack: Option<PackSelection>,
    picked_curseforge_pack: Option<curseforge_pack::Selection>,
    picked_import: Option<launcher_import::ImportSelection>,
    world_previews: HashMap<String, worlds::WorldSelection>,
}
#[derive(Default)]
pub struct GameLibrary(Mutex<Library>);

fn library_file() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("无法找到配置目录")?
        .join("CobbleLauncher/games.json"))
}
fn default_directory() -> Result<PathBuf, String> {
    directory_migration::require_idle()?;
    crate::preferences::game_directory()
}
fn save(games: &[Game]) -> Result<(), String> {
    directory_migration::require_idle()?;
    let path = library_file()?;
    fs::create_dir_all(path.parent().expect("library has a parent")).map_err(|e| e.to_string())?;
    fs::write(
        path,
        serde_json::to_vec_pretty(games).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
impl GameLibrary {
    pub fn load() -> Self {
        let started = Instant::now();
        let result = (|| {
            let bytes = match fs::read(library_file()?) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Vec::new());
                }
                Err(e) => return Err(e.to_string()),
            };
            load_summaries(&bytes)
        })();
        let (games, load_error) = match result {
            Ok(games) => (games, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        tracing::info!(
            event = "startup_library_summaries_loaded",
            elapsed_ms = started.elapsed().as_millis() as u64,
            instances = games.len(),
            failed = load_error.is_some()
        );
        Self(Mutex::new(Library {
            games,
            load_error,
            ..Library::default()
        }))
    }
}

fn load_summaries(bytes: &[u8]) -> Result<Vec<Game>, String> {
    let mut games: Vec<Game> = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    for game in &mut games {
        if game.status == "installing" {
            game.status = "failed".into();
            game.error = Some("安装已中断，请重试".into());
        }
        // No instance filesystem or process access on the summary render path.
        game.launch_state = "verifying".into();
    }
    Ok(games)
}

#[cfg(test)]
mod startup_summary_tests {
    use super::*;
    #[test]
    fn summaries_do_not_access_instance_directories_and_keep_each_game_locked_until_verified() {
        let games = load_summaries(br#"[{"id":"ab","name":"Offline drive","version":"1.21.1","directory":"unavailable:/synthetic","status":"installed"},{"id":"cd","name":"Interrupted","version":"1.21.1","directory":"unavailable:/other","status":"installing"}]"#).unwrap();
        assert_eq!(games.len(), 2);
        assert!(games.iter().all(|game| game.launch_state == "verifying"));
        assert_eq!(games[0].status, "installed");
        assert_eq!(games[1].status, "failed");
        assert!(games[1].error.is_some());
    }
}

#[derive(Serialize)]
pub struct GameVersion {
    id: String,
    kind: String,
}

#[tauri::command]
pub async fn list_game_versions(source: DownloadSettings) -> Result<Vec<GameVersion>, String> {
    let source = source.download_source().map_err(|e| e.to_string())?;
    let broker =
        cobble_net::Broker::new(cobble_net::BrokerConfig::default()).map_err(|e| e.to_string())?;
    let catalog =
        VersionCatalog::fetch_from(&broker, &source)
            .await
            .map_err(|error| match error {
                cobble_minecraft::CatalogError::Network(cobble_net::NetError::Timeout(_)) => {
                    "版本列表请求超时，请重试或切换下载源".into()
                }
                cobble_minecraft::CatalogError::Network(_) => {
                    "无法连接版本服务，请重试或切换下载源".into()
                }
                other => other.to_string(),
            })?;
    Ok(catalog
        .versions
        .into_iter()
        .filter(|v| v.kind == VersionKind::Release && modern_version(&v.id))
        .map(|v| GameVersion {
            id: v.id,
            kind: "release".into(),
        })
        .collect())
}
fn modern_version(id: &str) -> bool {
    let mut parts = id.split('.').filter_map(|p| p.parse::<u32>().ok());
    match (parts.next(), parts.next()) {
        (Some(1), Some(minor)) => minor >= 17,
        (Some(major), _) => major >= 26,
        _ => false,
    }
}
#[tauri::command]
pub fn list_games(library: State<'_, GameLibrary>) -> Result<Vec<Game>, String> {
    let library = library.0.lock().map_err(|e| e.to_string())?;
    if let Some(error) = &library.load_error {
        return Err(error.clone());
    }
    Ok(library.games.clone())
}

pub(crate) fn launcher_update_safe(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|_| "UPDATE_BUSY")?;
    if library.load_error.is_some()
        || library.games.iter().any(|game| {
            game.launch_state != "idle" || game.content_installing || game.status == "installing"
        })
    {
        return Err("UPDATE_BUSY".into());
    }
    drop(library);
    if tasks::active_count(app) > 0 {
        return Err("UPDATE_BUSY".into());
    }
    Ok(())
}

pub(crate) fn ensure_registered_game(app: &AppHandle, id: &str) -> Result<(), String> {
    registered_game_stopped(app, id).map(|_| ())
}
pub(crate) fn registered_game_version(app: &AppHandle, id: &str) -> Result<String, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|_| "游戏库不可用")?;
    if let Some(error) = &library.load_error {
        return Err(error.clone());
    }
    library
        .games
        .iter()
        .find(|game| game.id == id && game.status == "installed")
        .map(|game| game.version.clone())
        .ok_or_else(|| "找不到已安装的游戏".into())
}
pub(crate) fn registered_game_stopped(app: &AppHandle, id: &str) -> Result<bool, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|_| "游戏库不可用")?;
    if let Some(error) = &library.load_error {
        return Err(error.clone());
    }
    let game = library
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    Ok(game.launch_state == "idle" && !game.content_installing && game.status != "installing")
}
#[tauri::command]
pub fn default_game_directory() -> Result<String, String> {
    Ok(default_directory()?.display().to_string())
}
#[tauri::command]
pub async fn pick_game_directory(
    window: WebviewWindow,
    library: State<'_, GameLibrary>,
) -> Result<Option<String>, String> {
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_title("选择游戏保存位置")
        .set_parent(&window)
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let directory = folder.path().to_owned();
    let display = directory.display().to_string();
    library
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .picked_directory = Some(directory);
    Ok(Some(display))
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn create_game(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    name: String,
    version: String,
    source: DownloadSettings,
    use_default_directory: bool,
    loader_version: Option<String>,
    loader_kind: Option<String>,
    automatic_name: Option<bool>,
) -> Result<Game, String> {
    create_game_inner(
        app,
        library,
        name,
        version,
        source,
        use_default_directory,
        loader_version,
        loader_kind,
        automatic_name,
        None,
        None,
        Vec::new(),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn create_with_mod_plan(
    app: AppHandle,
    name: String,
    version: String,
    source: DownloadSettings,
    use_default_directory: bool,
    loader_version: String,
    loader_kind: String,
    automatic_name: bool,
    plan: cobble_content::ModrinthPlan,
) -> Result<Game, String> {
    create_game_inner(
        app.clone(),
        app.state::<GameLibrary>(),
        name,
        version,
        source,
        use_default_directory,
        Some(loader_version),
        Some(loader_kind),
        Some(automatic_name),
        Some(plan),
        None,
        Vec::new(),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_with_environment_plan(
    app: AppHandle,
    name: String,
    version: String,
    source: DownloadSettings,
    use_default_directory: bool,
    loader_version: String,
    loader_kind: String,
    automatic_name: bool,
    plan: cobble_content::EnvironmentPlan,
) -> Result<Game, String> {
    create_with_environment_plan_and_configs(
        app,
        name,
        version,
        source,
        use_default_directory,
        loader_version,
        loader_kind,
        automatic_name,
        plan,
        Vec::new(),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_with_environment_plan_and_configs(
    app: AppHandle,
    name: String,
    version: String,
    source: DownloadSettings,
    use_default_directory: bool,
    loader_version: String,
    loader_kind: String,
    automatic_name: bool,
    plan: cobble_content::EnvironmentPlan,
    configs: Vec<cobble_core::environment_install::SharedConfig>,
    personal_options: Option<cobble_core::personal_options::PersonalOptions>,
) -> Result<Game, String> {
    cobble_core::environment_install::validate_environment_plan(&plan)
        .map_err(|e| e.to_string())?;
    cobble_core::environment_install::validate_shared_configs(&configs)
        .map_err(|e| e.to_string())?;
    if version != plan.game_version || loader_kind != plan.loader {
        return Err("ENVIRONMENT_GAME_CHANGED".into());
    }
    create_game_inner(
        app.clone(),
        app.state::<GameLibrary>(),
        name,
        version,
        source,
        use_default_directory,
        Some(loader_version),
        Some(loader_kind),
        Some(automatic_name),
        None,
        Some(plan),
        configs,
        personal_options,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_with_shared_setup(
    app: AppHandle,
    name: String,
    version: String,
    source: DownloadSettings,
    use_default_directory: bool,
    loader_version: Option<String>,
    loader_kind: Option<String>,
    configs: Vec<cobble_core::environment_install::SharedConfig>,
    personal_options: Option<cobble_core::personal_options::PersonalOptions>,
) -> Result<Game, String> {
    cobble_core::environment_install::validate_shared_configs(&configs)
        .map_err(|e| e.to_string())?;
    create_game_inner(
        app.clone(),
        app.state::<GameLibrary>(),
        name,
        version,
        source,
        use_default_directory,
        loader_version,
        loader_kind,
        Some(false),
        None,
        None,
        configs,
        personal_options,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_game_inner(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    name: String,
    version: String,
    source: DownloadSettings,
    use_default_directory: bool,
    loader_version: Option<String>,
    loader_kind: Option<String>,
    automatic_name: Option<bool>,
    initial_mod_plan: Option<cobble_content::ModrinthPlan>,
    initial_environment_plan: Option<cobble_content::EnvironmentPlan>,
    initial_shared_configs: Vec<cobble_core::environment_install::SharedConfig>,
    initial_personal_options: Option<cobble_core::personal_options::PersonalOptions>,
) -> Result<Game, String> {
    if let Some(options) = &initial_personal_options {
        cobble_core::personal_options::validate(options)?;
        if options.target_version != version {
            return Err("PERSONAL_OPTIONS_TARGET_CHANGED".into());
        }
    }
    if name.trim().is_empty() {
        return Err("请填写游戏名称".into());
    }
    let loader_kind = if loader_version.is_some() {
        let kind = loader_kind.unwrap_or_else(|| "fabric".into());
        if !matches!(kind.as_str(), "fabric" | "forge" | "neoforge") {
            return Err("不支持这个模组加载器".into());
        }
        Some(kind)
    } else {
        None
    };
    source.download_source().map_err(|e| e.to_string())?;
    let mut library = library.0.lock().map_err(|e| e.to_string())?;
    if let Some(error) = &library.load_error {
        return Err(error.clone());
    }
    if library.games.iter().any(|g| g.status == "installing") {
        return Err("已有游戏正在安装，请稍候".into());
    }
    let base = if use_default_directory {
        default_directory()?
    } else {
        library.picked_directory.clone().ok_or("请先选择保存位置")?
    };
    let id = format!(
        "{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let name = if automatic_name.unwrap_or(false) {
        unique_game_name(
            name.trim(),
            library.games.iter().map(|game| game.name.as_str()),
        )
    } else {
        name.trim().to_owned()
    };
    let game = Game {
        directory: base.join(&id),
        id,
        name,
        last_launched_at: None,
        appearance_preset: game_appearance::default_preset(),
        icon_ref: None,
        cover_ref: None,
        pinned: false,
        version,
        loader_version,
        loader_kind,
        profile_id: None,
        runtime_overrides: cobble_minecraft::LaunchOverrides::default(),
        download_source: Some(source.clone()),
        status: "installing".into(),
        error: None,
        progress: None,
        launch_state: idle(),
        launch_error: None,
        log_path: None,
        content_installing: false,
        pack_name: None,
        pack_version: None,
        pack_source_id: None,
        pack_origin: None,
        pack_optional_files: Vec::new(),
        pack_mod_files: Vec::new(),
        launcher_import_id: None,
    };
    library.games.push(game.clone());
    drop(library);
    // Persist the request first: a recovered game must never lose its initial
    // Mod selection if the process exits between these separate durable writes.
    let task = register_install(
        &app,
        &game,
        initial_mod_plan,
        initial_environment_plan,
        initial_shared_configs,
        initial_personal_options,
    );
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|e| e.to_string())?;
    let task_id = match task {
        Ok(id) => id,
        Err(error) => {
            library.games.retain(|item| item.id != game.id);
            return Err(error);
        }
    };
    if let Err(error) = save(&library.games) {
        library.games.retain(|item| item.id != game.id);
        drop(library);
        tasks::discard_unstarted_install(&app, &task_id)?;
        return Err(error);
    }
    drop(library);
    tauri::async_runtime::spawn(async move {
        let _ = tasks::execute(app, task_id).await;
    });
    Ok(game)
}
#[tauri::command]
pub fn retry_game(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<Game, String> {
    let install_task = tasks::game_install_task(&app, &id)?;
    let mut library = library.0.lock().map_err(|e| e.to_string())?;
    if library.games.iter().any(|g| g.status == "installing") {
        return Err("已有游戏正在安装，请稍候".into());
    }
    let game = library
        .games
        .iter_mut()
        .find(|g| g.id == id)
        .ok_or("找不到这个游戏")?;
    if game.status != "failed" {
        return Err("这个游戏无需重试".into());
    }
    if let Some(task_id) = install_task {
        drop(library);
        tasks::retry_task(app.clone(), task_id)?;
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|e| e.to_string())?;
        return library
            .games
            .iter()
            .find(|game| game.id == id)
            .cloned()
            .ok_or("找不到这个游戏".into());
    }
    let source = match &game.download_source {
        Some(source) => source.clone(),
        None => cobble_minecraft::read_download_settings().map_err(|e| e.to_string())?,
    };
    let resolved_source = source.download_source().map_err(|e| e.to_string())?;
    game.download_source = Some(source);
    game.status = "installing".into();
    game.error = None;
    game.progress = None;
    let game = game.clone();
    save(&library.games)?;
    drop(library);
    start_install(app, game.clone(), resolved_source);
    Ok(game)
}
fn update_game(app: &AppHandle, id: &str, change: impl FnOnce(&mut Game), persist: bool) {
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().expect("game library mutex");
    let Some(game) = library.games.iter_mut().find(|g| g.id == id) else {
        return;
    };
    let previous_phase = game
        .progress
        .as_ref()
        .map(|progress| progress.phase.clone());
    let previous_status = game.status.clone();
    change(game);
    if previous_phase.as_deref()
        != game
            .progress
            .as_ref()
            .map(|progress| progress.phase.as_str())
    {
        tracing::info!(event="install_phase", game_id=%game.id, phase=game.progress.as_ref().map(|progress|progress.phase.as_str()).unwrap_or("idle"));
    }
    if previous_status != game.status {
        tracing::info!(event="install_state", game_id=%game.id, state=%game.status);
    }
    let snapshot = game.clone();
    if persist && let Err(error) = save(&library.games) {
        tracing::error!(event="library_save_failed", error=%error);
    }
    drop(library);
    let _ = app.emit("game-install", snapshot);
}
fn start_install(app: AppHandle, game: Game, _source: cobble_minecraft::DownloadSource) {
    if let Err(error) = schedule_install(app.clone(), &game, None) {
        update_game(
            &app,
            &game.id,
            |game| {
                game.status = "failed".into();
                game.error = Some(error);
            },
            true,
        );
    }
}

fn schedule_install(
    app: AppHandle,
    game: &Game,
    initial_mod_plan: Option<cobble_content::ModrinthPlan>,
) -> Result<(), String> {
    let id = register_install(&app, game, initial_mod_plan, None, Vec::new(), None)?;
    tauri::async_runtime::spawn(async move {
        let _ = tasks::execute(app, id).await;
    });
    Ok(())
}

fn register_install(
    app: &AppHandle,
    game: &Game,
    initial_mod_plan: Option<cobble_content::ModrinthPlan>,
    initial_environment_plan: Option<cobble_content::EnvironmentPlan>,
    initial_shared_configs: Vec<cobble_core::environment_install::SharedConfig>,
    initial_personal_options: Option<cobble_core::personal_options::PersonalOptions>,
) -> Result<String, String> {
    let request = tasks::TaskRequest::GameInstall {
        game_id: game.id.clone(),
        initial_mod_plan,
        initial_environment_plan,
        initial_shared_configs,
        initial_personal_options,
    };
    tasks::register(app, request, "安装游戏")
}

#[tauri::command]
pub async fn list_fabric_versions(
    version: String,
    source: DownloadSettings,
) -> Result<Vec<cobble_core::FabricVersion>, String> {
    let source = source.download_source().map_err(|e| e.to_string())?;
    cobble_core::list_fabric_versions_from(&version, &source)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct GameMod {
    #[serde(flatten)]
    item: cobble_content::LocalMod,
    pack_owned: bool,
}

fn with_game_mods(
    app: &AppHandle,
    id: &str,
    writable: bool,
    operation: impl FnOnce(
        &std::path::Path,
    ) -> Result<Vec<cobble_content::LocalMod>, cobble_content::BoxError>,
) -> Result<Vec<GameMod>, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter()
        .find(|g| g.id == id)
        .ok_or("找不到这个游戏")?;
    if game.status != "installed" {
        return Err("请先完成游戏安装".into());
    }
    if writable && (game.launch_state != "idle" || game.content_installing) {
        return Err("请先关闭游戏，再修改 Mod".into());
    }
    if writable {
        cobble_core::recover_modrinth_update(&game.directory).map_err(|e| e.to_string())?;
    }
    let items = operation(&game.directory).map_err(|e| e.to_string())?;
    let mut owned: std::collections::HashSet<_> = game.pack_mod_files.iter().cloned().collect();
    if let Some(source_id) = &game.pack_source_id
        && let Ok(path) = pack_archive_path(source_id)
        && let Ok(pack) = cobble_content::read_mrpack(&path)
    {
        for file in pack.files {
            if !file.optional || game.pack_optional_files.contains(&file.path) {
                owned.insert(file.path);
            }
        }
        owned.extend(pack.overrides.into_iter().map(|entry| entry.path));
    }
    Ok(items
        .into_iter()
        .map(|item| {
            let file = item
                .file_name
                .strip_suffix(".disabled")
                .unwrap_or(&item.file_name);
            let pack_owned = owned.contains(&format!("mods/{file}"));
            GameMod { item, pack_owned }
        })
        .collect())
}

#[tauri::command]
pub async fn list_game_mods(app: AppHandle, id: String) -> Result<Vec<GameMod>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_game_mods(&app, &id, false, cobble_content::list_mods)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn import_game_mods(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
) -> Result<Option<Vec<GameMod>>, String> {
    let Some(files) = rfd::AsyncFileDialog::new()
        .set_title("导入 Mod")
        .add_filter("Minecraft Mod", &["jar"])
        .set_parent(&window)
        .pick_files()
        .await
    else {
        return Ok(None);
    };
    let paths: Vec<PathBuf> = files.iter().map(|file| file.path().to_owned()).collect();
    tauri::async_runtime::spawn_blocking(move || {
        with_game_mods(&app, &id, true, |directory| {
            cobble_content::import_mods(directory, &paths)
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map(Some)
}

#[tauri::command]
pub async fn set_game_mod_enabled(
    app: AppHandle,
    id: String,
    file_name: String,
    enabled: bool,
) -> Result<Vec<GameMod>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_game_mods(&app, &id, true, |directory| {
            cobble_content::set_mod_enabled(directory, &file_name, enabled)
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn remove_game_mod(
    app: AppHandle,
    id: String,
    file_name: String,
) -> Result<Vec<GameMod>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_game_mods(&app, &id, true, |directory| {
            cobble_content::remove_mod(directory, &file_name)
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn modrinth_target(app: &AppHandle, id: &str) -> Result<(String, String), String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter()
        .find(|g| g.id == id)
        .ok_or("找不到这个游戏")?;
    if game.status != "installed" {
        return Err("请先完成游戏安装".into());
    }
    if game.loader_version.is_none() {
        return Err("请先创建带模组加载器的游戏，再安装 Mod".into());
    }
    Ok((
        game.version.clone(),
        game.loader_kind.clone().unwrap_or_else(|| "fabric".into()),
    ))
}

#[tauri::command]
pub async fn search_game_mods(
    app: AppHandle,
    id: String,
    query: String,
    offset: u32,
) -> Result<cobble_content::SearchResults, String> {
    let (version, kind) = modrinth_target(&app, &id)?;
    cobble_content::search_modrinth(&query, &version, &kind, offset)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct ModInstallPreview {
    plan_id: String,
    project_id: String,
    version_id: String,
    mods: Vec<ModInstallItem>,
}
#[derive(Serialize)]
pub struct ModInstallItem {
    project_id: String,
    version_id: String,
    name: String,
    version_number: String,
    required_by: Option<String>,
}

#[tauri::command]
pub async fn preview_game_mod_install(
    app: AppHandle,
    id: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<ModInstallPreview, String> {
    let (version, kind) = modrinth_target(&app, &id)?;
    let plan_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos()
        .to_string();
    {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|e| e.to_string())?;
        library.mod_preview_ids.insert(id.clone(), plan_id.clone());
        library.mod_plans.remove(&id);
    }
    let plan = match version_id {
        Some(version_id) => {
            cobble_content::resolve_modrinth_selected(
                &[cobble_content::ModVersionSelection {
                    project_id,
                    version_id,
                }],
                &version,
                &kind,
            )
            .await
        }
        None => cobble_content::resolve_modrinth(&project_id, &version, &kind).await,
    }
    .map_err(|e| e.to_string())?;
    let preview = ModInstallPreview {
        plan_id: plan_id.clone(),
        project_id: plan.project_id.clone(),
        version_id: plan.version_id.clone(),
        mods: plan
            .mods
            .iter()
            .map(|m| ModInstallItem {
                project_id: m.project_id.clone(),
                version_id: m.version_id.clone(),
                name: m.name.clone(),
                version_number: m.version_number.clone(),
                required_by: m.required_by.clone(),
            })
            .collect(),
    };
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|e| e.to_string())?;
    save_mod_preview(&mut library, id, plan_id, plan)?;
    Ok(preview)
}

fn save_mod_preview(
    library: &mut Library,
    id: String,
    plan_id: String,
    plan: cobble_content::ModrinthPlan,
) -> Result<(), String> {
    if library.mod_preview_ids.get(&id) != Some(&plan_id)
        || !library.games.iter().any(|game| game.id == id)
    {
        return Err("版本选择已变更，请使用当前安装预览".into());
    }
    library.mod_plans.insert(id, (plan_id, plan));
    Ok(())
}

fn unique_game_name<'a>(suggestion: &str, names: impl Iterator<Item = &'a str>) -> String {
    let names: std::collections::HashSet<_> = names.map(str::to_lowercase).collect();
    if !names.contains(&suggestion.to_lowercase()) {
        return suggestion.to_owned();
    }
    for number in 2.. {
        let candidate = format!("{suggestion} ({number})");
        if !names.contains(&candidate.to_lowercase()) {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod naming_tests {
    use super::unique_game_name;
    #[test]
    fn suggested_names_skip_existing_suffixes_and_ignore_case() {
        assert_eq!(
            unique_game_name(
                "1.21.1 Fabric",
                ["1.21.1 fabric", "1.21.1 Fabric (2)"].into_iter()
            ),
            "1.21.1 Fabric (3)"
        );
        assert_eq!(unique_game_name("1.21.1", [].into_iter()), "1.21.1");
    }
}

#[cfg(test)]
mod mod_preview_tests {
    use super::*;

    #[test]
    fn slow_previous_resolution_cannot_replace_current_version_plan() {
        let game = serde_json::from_value(serde_json::json!({
            "id":"game", "name":"Fixture", "version":"1.21.1",
            "directory":"synthetic", "status":"installed", "error":null
        }))
        .unwrap();
        let mut library = Library {
            games: vec![game],
            ..Library::default()
        };
        library
            .mod_preview_ids
            .insert("game".into(), "new-request".into());
        let plan = |version: &str| cobble_content::ModrinthPlan {
            project_id: "project".into(),
            version_id: version.into(),
            mods: Vec::new(),
        };
        save_mod_preview(
            &mut library,
            "game".into(),
            "new-request".into(),
            plan("new-version"),
        )
        .unwrap();
        assert!(
            save_mod_preview(
                &mut library,
                "game".into(),
                "old-request".into(),
                plan("old-version")
            )
            .is_err()
        );
        assert_eq!(library.mod_plans["game"].1.version_id, "new-version");
        library.games.clear();
        assert!(
            save_mod_preview(
                &mut library,
                "game".into(),
                "new-request".into(),
                plan("new-version")
            )
            .is_err()
        );
    }
}

#[tauri::command]
pub async fn install_game_mod_plan(
    app: AppHandle,
    id: String,
    plan_id: String,
) -> Result<cobble_core::ModrinthInstallOutcome, String> {
    let plan = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| game.id == id)
            .ok_or("找不到这个游戏")?;
        if game.status != "installed" || game.loader_version.is_none() {
            return Err("请先完成游戏和模组加载器安装".into());
        }
        if game.launch_state != "idle" || game.content_installing {
            return Err("请先关闭游戏或等待安装完成".into());
        }
        let stored = library.mod_plans.get(&id).ok_or("请先选择要安装的 Mod")?;
        if stored.0 != plan_id {
            return Err("安装方案已变更，请重新选择 Mod".into());
        }
        stored.1.clone()
    };
    let title = format!(
        "安装 {}",
        plan.mods
            .iter()
            .find(|item| item.project_id == plan.project_id)
            .map(|item| item.name.as_str())
            .unwrap_or("Mod")
    );
    match tasks::run(
        app,
        tasks::TaskRequest::ModInstall { game_id: id, plan },
        &title,
    )
    .await?
    {
        tasks::TaskResult::ModInstall(outcome) => Ok(outcome),
        _ => Err("任务类型不匹配".into()),
    }
}

#[tauri::command]
pub fn rename_game(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
    name: String,
) -> Result<Game, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("游戏名称应为 1–80 个字符".into());
    }
    let mut library = library.0.lock().map_err(|e| e.to_string())?;
    if let Some(error) = &library.load_error {
        return Err(error.clone());
    }
    let mut games = library.games.clone();
    let game = games
        .iter_mut()
        .find(|g| g.id == id)
        .ok_or("找不到这个游戏")?;
    game.name = name.into();
    let snapshot = game.clone();
    save(&games)?;
    library.games = games;
    drop(library);
    let _ = app.emit("game-install", &snapshot);
    Ok(snapshot)
}

pub(crate) fn game_directory(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|e| e.to_string())?;
    Ok(library
        .games
        .iter()
        .find(|g| g.id == id)
        .ok_or("找不到这个游戏")?
        .directory
        .clone())
}

#[tauri::command]
pub async fn open_game_directory(app: AppHandle, id: String) -> Result<(), String> {
    let directory = game_directory(&app, &id)?;
    if !directory.is_dir() {
        return Err("游戏目录尚未创建，请先完成安装".into());
    }
    tauri::async_runtime::spawn_blocking(move || open::that(directory).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn open_game_log(app: AppHandle, id: String) -> Result<(), String> {
    let path = game_directory(&app, &id)?.join(".cobble-logs/launcher.log");
    if !path.is_file() {
        return Err("还没有游戏运行日志".into());
    }
    tauri::async_runtime::spawn_blocking(move || open::that(path).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_loader_versions(
    version: String,
    kind: String,
    source: DownloadSettings,
) -> Result<Vec<cobble_core::ForgeVersion>, String> {
    let source = source.download_source().map_err(|e| e.to_string())?;
    let kind = match kind.as_str() {
        "forge" => cobble_core::ForgeKind::Forge,
        "neoforge" => cobble_core::ForgeKind::NeoForge,
        _ => return Err("不支持这个模组加载器".into()),
    };
    cobble_core::list_forge_versions_from(&version, kind, &source)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_game_profile(
    library: State<'_, GameLibrary>,
    id: String,
    profile_id: Option<String>,
) -> Result<Game, String> {
    if let Some(profile_id) = profile_id.as_deref() {
        cobble_auth::get_player_profile(Some(profile_id)).map_err(|e| e.to_string())?;
    }
    let mut library = library.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter_mut()
        .find(|g| g.id == id)
        .ok_or("找不到这个游戏")?;
    let previous = game.profile_id.clone();
    game.profile_id = profile_id;
    let snapshot = game.clone();
    if let Err(error) = save(&library.games) {
        library
            .games
            .iter_mut()
            .find(|g| g.id == id)
            .unwrap()
            .profile_id = previous;
        return Err(error);
    }
    Ok(snapshot)
}

pub(crate) fn frontend_game_summaries(app: &AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let mut games =
        list_games(app.state::<GameLibrary>()).map_err(|_| "PLUGIN_CONTEXT_UNAVAILABLE")?;
    games.sort_by(|a, b| {
        b.last_launched_at
            .cmp(&a.last_launched_at)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(games
        .into_iter()
        .take(1024)
        .map(|game| serde_json::json!({"id":game.id,"name":game.name,"version":game.version}))
        .collect())
}
