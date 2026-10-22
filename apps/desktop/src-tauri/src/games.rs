// SPDX-License-Identifier: MIT
#[path = "launcher_import.rs"]
pub mod launcher_import;
#[path = "local_packs.rs"]
pub mod local_packs;
#[path = "management.rs"]
pub mod management;
#[path = "marketplace.rs"]
pub mod marketplace;
#[path = "mod_updates.rs"]
pub mod mod_updates;
#[path = "worlds.rs"]
pub mod worlds;
use cobble_core::InstallProgress;
use cobble_minecraft::{DownloadSettings, VersionCatalog, VersionKind};
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
        .picked_pack = Some(PackSelection { id, pack });
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
        store_pack_preview(&app, id, pack).map(Some)
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
    update_plans: HashMap<String, (String, cobble_core::ModUpdatePlan)>,
    picked_pack: Option<PackSelection>,
    picked_import: Option<launcher_import::ImportSelection>,
}
#[derive(Default)]
pub struct GameLibrary(Mutex<Library>);

fn library_file() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("无法找到配置目录")?
        .join("CobbleLauncher/games.json"))
}
fn default_directory() -> Result<PathBuf, String> {
    crate::preferences::game_directory()
}
fn save(games: &[Game]) -> Result<(), String> {
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
        let result = (|| {
            let bytes = match fs::read(library_file()?) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
                Err(e) => return Err(e.to_string()),
            };
            let mut games: Vec<Game> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            for game in &mut games {
                if game.status == "installing" {
                    game.status = "failed".into();
                    game.error = Some("安装已中断，请重试".into());
                }
            }
            Ok::<_, String>(games)
        })();
        let (games, load_error) = match result {
            Ok(games) => (games, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        Self(Mutex::new(Library {
            games,
            load_error,
            ..Library::default()
        }))
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
) -> Result<Game, String> {
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
    let resolved_source = source.download_source().map_err(|e| e.to_string())?;
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
    let game = Game {
        directory: base.join(&id),
        id,
        name: name.trim().into(),
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
        pack_optional_files: Vec::new(),
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
#[tauri::command]
pub fn retry_game(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<Game, String> {
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
    change(game);
    let snapshot = game.clone();
    if persist && let Err(error) = save(&library.games) {
        eprintln!("Could not save game library: {error}");
    }
    drop(library);
    let _ = app.emit("game-install", snapshot);
}
fn start_install(app: AppHandle, game: Game, source: cobble_minecraft::DownloadSource) {
    tauri::async_runtime::spawn(async move {
        let event_app = app.clone();
        let event_id = game.id.clone();
        let mut last_phase = String::new();
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        let throttle = Mutex::new(move |progress: &InstallProgress| {
            let emit = progress.phase != last_phase
                || progress.completed == progress.total
                || last_emit.elapsed() >= Duration::from_millis(100);
            if emit {
                last_emit = Instant::now();
                last_phase.clone_from(&progress.phase);
            }
            emit
        });
        let progress = Arc::new(move |progress: InstallProgress| {
            if throttle.lock().expect("progress mutex")(&progress) {
                update_game(
                    &event_app,
                    &event_id,
                    |g| g.progress = Some(progress),
                    false,
                );
            }
        });
        let result = if game.launcher_import_id.is_some() {
            launcher_import::install(&game, source, progress).await
        } else if let Some(source_id) = &game.pack_source_id {
            match pack_archive_path(source_id) {
                Ok(archive) => cobble_core::install_mrpack(
                    &archive,
                    &game.directory,
                    source,
                    &game.pack_optional_files,
                    progress,
                )
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
                Err(error) => Err(error),
            }
        } else {
            match &game.loader_version {
                Some(loader) => match game.loader_kind.as_deref().unwrap_or("fabric") {
                    "fabric" => {
                        cobble_core::install_fabric(
                            &game.version,
                            loader,
                            &game.directory,
                            source,
                            progress,
                        )
                        .await
                    }
                    "forge" => {
                        cobble_core::install_forge(
                            &game.version,
                            loader,
                            cobble_core::ForgeKind::Forge,
                            &game.directory,
                            source,
                            progress,
                        )
                        .await
                    }
                    "neoforge" => {
                        cobble_core::install_forge(
                            &game.version,
                            loader,
                            cobble_core::ForgeKind::NeoForge,
                            &game.directory,
                            source,
                            progress,
                        )
                        .await
                    }
                    _ => Err("unsupported mod loader".into()),
                },
                None => {
                    cobble_core::install_game(&game.version, &game.directory, source, progress)
                        .await
                }
            }
            .map(|_| ())
            .map_err(|e| e.to_string())
        };
        update_game(
            &app,
            &game.id,
            |g| match result {
                Ok(_) => {
                    g.status = "installed".into();
                    g.error = None;
                }
                Err(error) => {
                    g.status = "failed".into();
                    g.error = Some(error.to_string());
                }
            },
            true,
        );
    });
}

#[derive(Serialize)]
pub struct LaunchReadiness {
    java: crate::java::JavaRuntimeInfo,
    required_java: u32,
    main_class: String,
    classpath_entries: usize,
}

#[tauri::command]
pub async fn prepare_game_launch(
    id: String,
    library: State<'_, GameLibrary>,
) -> Result<LaunchReadiness, String> {
    let (version, directory, java_override) = {
        let library = library.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| game.id == id)
            .ok_or("找不到这个游戏")?;
        if game.status != "installed" {
            return Err("请先完成游戏安装".into());
        }
        (
            game.version.clone(),
            game.directory.clone(),
            game.runtime_overrides.java.clone(),
        )
    };
    let launch = cobble_core::prepare_launch(&version, &directory)
        .await
        .map_err(|e| e.to_string())?;
    let java = cobble_minecraft::resolve_java_override(launch.required_java, &java_override)
        .await
        .map_err(|e| e.to_string())?;
    Ok(LaunchReadiness {
        java: java.into(),
        required_java: launch.required_java,
        main_class: launch.main_class,
        classpath_entries: launch.classpath_entries,
    })
}

#[tauri::command]
pub fn start_game(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<Game, String> {
    let mut library = library.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter_mut()
        .find(|g| g.id == id)
        .ok_or("找不到这个游戏")?;
    if game.status != "installed" {
        return Err("请先完成游戏安装".into());
    }
    if game.launch_state != "idle" {
        return Err("这个游戏已经在启动或运行".into());
    }
    if game.content_installing {
        return Err("Mod 正在安装，请稍候".into());
    }
    // Capture the effective identity before spawning; later selection changes
    // only affect future launches.
    let profile =
        cobble_auth::get_player_profile(game.profile_id.as_deref()).map_err(|e| e.to_string())?;
    game.launch_state = "preparing".into();
    game.launch_error = None;
    let snapshot = game.clone();
    drop(library);
    let launch = snapshot.clone();
    tauri::async_runtime::spawn(async move {
        let result = async {
            let identity = cobble_auth::launch_identity(Some(&profile.id)).await?;
            cobble_core::start_game_with_identity(
                &launch.version,
                &launch.directory,
                &identity,
                &launch.runtime_overrides,
            )
            .await
        }
        .await;
        match result {
            Ok(mut running) => {
                update_game(
                    &app,
                    &launch.id,
                    |g| {
                        g.launch_state = "running".into();
                        g.log_path = Some(running.log_path);
                    },
                    false,
                );
                let result = running.child.wait().await;
                update_game(
                    &app,
                    &launch.id,
                    |g| {
                        g.launch_state = idle();
                        g.launch_error = match result {
                            Ok(status) if status.success() => None,
                            Ok(status) => Some(format!("游戏异常退出（{status}），请查看游戏日志")),
                            Err(error) => Some(error.to_string()),
                        };
                    },
                    true,
                );
            }
            Err(error) => update_game(
                &app,
                &launch.id,
                |g| {
                    g.launch_state = idle();
                    g.launch_error = Some(error.to_string());
                },
                true,
            ),
        }
    });
    Ok(snapshot)
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
    let (plan, directory) = {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|g| g.id == id)
            .ok_or("找不到这个游戏")?;
        if game.status != "installed" || game.loader_version.is_none() {
            return Err("请先完成游戏和模组加载器安装".into());
        }
        if game.launch_state != "idle" || game.content_installing {
            return Err("请先关闭游戏或等待安装完成".into());
        }
        let directory = game.directory.clone();
        let stored = library.mod_plans.get(&id).ok_or("请先选择要安装的 Mod")?;
        if stored.0 != plan_id {
            return Err("安装方案已变更，请重新选择 Mod".into());
        }
        let plan = stored.1.clone();
        library
            .games
            .iter_mut()
            .find(|g| g.id == id)
            .expect("checked game")
            .content_installing = true;
        (plan, directory)
    };
    update_game(&app, &id, |_| {}, false);
    let result = cobble_core::install_modrinth_plan(&plan, &directory, Arc::new(|_| {}))
        .await
        .map_err(|e| e.to_string());
    update_game(&app, &id, |g| g.content_installing = false, false);
    result
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

fn game_directory(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
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

fn change_runtime(
    library: &GameLibrary,
    id: &str,
    change: impl FnOnce(&mut cobble_minecraft::LaunchOverrides),
) -> Result<Game, String> {
    let mut library = library.0.lock().map_err(|e| e.to_string())?;
    let game = library
        .games
        .iter_mut()
        .find(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    let previous = game.runtime_overrides.clone();
    change(&mut game.runtime_overrides);
    let snapshot = game.clone();
    if let Err(error) = save(&library.games) {
        library
            .games
            .iter_mut()
            .find(|game| game.id == id)
            .unwrap()
            .runtime_overrides = previous;
        return Err(error);
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn set_game_memory(
    library: State<'_, GameLibrary>,
    id: String,
    memory_mb: Option<u32>,
) -> Result<Game, String> {
    if let Some(memory) = memory_mb {
        cobble_minecraft::validate_memory_mb(memory).map_err(|e| e.to_string())?;
    }
    change_runtime(&library, &id, |settings| settings.memory_mb = memory_mb)
}

#[tauri::command]
pub fn set_game_java_mode(
    library: State<'_, GameLibrary>,
    id: String,
    mode: String,
) -> Result<Game, String> {
    let java = match mode.as_str() {
        "inherit" => cobble_minecraft::JavaOverride::Inherit,
        "automatic" => cobble_minecraft::JavaOverride::Automatic,
        _ => return Err("请选择自动选择或跟随默认 Java".into()),
    };
    change_runtime(&library, &id, |settings| settings.java = java)
}

#[tauri::command]
pub async fn pick_game_java_executable(
    window: WebviewWindow,
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<Option<Game>, String> {
    let (version, directory) = {
        let library = library.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| game.id == id)
            .ok_or("找不到这个游戏")?;
        (game.version.clone(), game.directory.clone())
    };
    let bytes = fs::read(directory.join(format!("versions/{version}/{version}.json")))
        .map_err(|e| e.to_string())?;
    let required = cobble_minecraft::VersionMetadata::parse_installed(&bytes, &version)
        .map_err(|e| e.to_string())?
        .java_version
        .major_version;
    let dialog = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择此游戏使用的 Java");
    #[cfg(windows)]
    let dialog = dialog.add_filter("Java (java.exe / javaw.exe)", &["exe"]);
    let Some(file) = dialog.pick_file().await else {
        return Ok(None);
    };
    let java = cobble_minecraft::probe_java(file.path())
        .await
        .map_err(|e| format!("无法使用这个 Java：{e}"))?;
    if java.major != required {
        return Err(format!(
            "这个游戏需要 Java {required}，所选版本为 Java {}",
            java.major
        ));
    }
    change_runtime(&library, &id, |settings| {
        settings.java = cobble_minecraft::JavaOverride::Custom {
            executable: java.executable,
        }
    })
    .map(Some)
}
