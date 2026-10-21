// SPDX-License-Identifier: MIT
use cobble_core::InstallProgress;
use cobble_minecraft::{DownloadSettings, VersionCatalog, VersionKind};
use serde::{Deserialize, Serialize};
use std::{
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
    directory: PathBuf,
    status: String,
    error: Option<String>,
    #[serde(skip_deserializing, default)]
    progress: Option<InstallProgress>,
}

#[derive(Default)]
struct Library {
    games: Vec<Game>,
    picked_directory: Option<PathBuf>,
    load_error: Option<String>,
}
#[derive(Default)]
pub struct GameLibrary(Mutex<Library>);

fn library_file() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("无法找到配置目录")?
        .join("CobbleLauncher/games.json"))
}
fn default_directory() -> Result<PathBuf, String> {
    Ok(dirs::data_local_dir()
        .ok_or("无法找到游戏目录")?
        .join("CobbleLauncher/games"))
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
    let catalog = VersionCatalog::fetch_from(&broker, &source)
        .await
        .map_err(|e| e.to_string())?;
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
pub fn create_game(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    name: String,
    version: String,
    source: DownloadSettings,
    use_default_directory: bool,
) -> Result<Game, String> {
    if name.trim().is_empty() {
        return Err("请填写游戏名称".into());
    }
    let resolved_source = source.download_source().map_err(|e| e.to_string())?;
    cobble_minecraft::save_download_settings(&source).map_err(|e| e.to_string())?;
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
        status: "installing".into(),
        error: None,
        progress: None,
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
    let source = cobble_minecraft::read_download_settings()
        .map_err(|e| e.to_string())?
        .download_source()
        .map_err(|e| e.to_string())?;
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
    game.status = "installing".into();
    game.error = None;
    game.progress = None;
    let game = game.clone();
    save(&library.games)?;
    drop(library);
    start_install(app, game.clone(), source);
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
        let result =
            cobble_core::install_game(&game.version, &game.directory, source, progress).await;
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
    let (version, directory) = {
        let library = library.0.lock().map_err(|e| e.to_string())?;
        let game = library
            .games
            .iter()
            .find(|game| game.id == id)
            .ok_or("找不到这个游戏")?;
        if game.status != "installed" {
            return Err("请先完成游戏安装".into());
        }
        (game.version.clone(), game.directory.clone())
    };
    let launch = cobble_core::prepare_launch(&version, &directory)
        .await
        .map_err(|e| e.to_string())?;
    let java = cobble_minecraft::resolve_java(launch.required_java)
        .await
        .map_err(|e| e.to_string())?;
    Ok(LaunchReadiness {
        java: java.into(),
        required_java: launch.required_java,
        main_class: launch.main_class,
        classpath_entries: launch.classpath_entries,
    })
}
