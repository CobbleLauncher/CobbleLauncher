// SPDX-License-Identifier: MIT
use super::*;
use cobble_core::LauncherImportCandidate;

pub(super) struct ImportSelection {
    pub id: String,
    pub candidates: Vec<LauncherImportCandidate>,
}

#[derive(Serialize)]
pub struct ImportPreview {
    preview_id: String,
    source_display: String,
    candidates: Vec<ImportCandidate>,
}
#[derive(Serialize)]
struct ImportCandidate {
    candidate_id: String,
    name: String,
    minecraft_version: Option<String>,
    loader_kind: Option<String>,
    loader_version: Option<String>,
    supported: bool,
    unsupported_reason: Option<String>,
    content_roots: Vec<ContentRoot>,
    warnings: Vec<String>,
}
#[derive(Serialize)]
struct ContentRoot {
    content_id: String,
    label: String,
    has_saves: bool,
}

#[derive(Serialize, Deserialize)]
struct ImportPlan {
    candidate: LauncherImportCandidate,
    content_id: String,
    include_saves: bool,
}

fn plan_path(id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("无效的游戏导入记录".into());
    }
    Ok(dirs::config_dir()
        .ok_or("无法找到配置目录")?
        .join("CobbleLauncher/import-sources")
        .join(format!("{id}.json")))
}

#[tauri::command]
pub async fn preview_launcher_import(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<Option<ImportPreview>, String> {
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择原启动器的游戏或实例文件夹")
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let directory = folder.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let candidates =
            cobble_core::inspect_launcher_import(&directory).map_err(|error| error.to_string())?;
        let id = next_pack_id()?;
        let preview = ImportPreview {
            preview_id: id.clone(),
            source_display: directory.display().to_string(),
            candidates: candidates
                .iter()
                .map(|item| ImportCandidate {
                    candidate_id: item.id.clone(),
                    name: item.name.clone(),
                    minecraft_version: item.minecraft_version.clone(),
                    loader_kind: item.loader_kind.clone(),
                    loader_version: item.loader_version.clone(),
                    supported: item.unsupported_reason.is_none()
                        && item.minecraft_version.is_some(),
                    unsupported_reason: item.unsupported_reason.clone(),
                    warnings: item.warnings.clone(),
                    content_roots: item
                        .content_roots
                        .iter()
                        .map(|root| ContentRoot {
                            content_id: root.id.clone(),
                            label: root.label.clone(),
                            has_saves: root.has_saves,
                        })
                        .collect(),
                })
                .collect(),
        };
        app.state::<GameLibrary>()
            .0
            .lock()
            .map_err(|error| error.to_string())?
            .picked_import = Some(ImportSelection { id, candidates });
        Ok(Some(preview))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn create_launcher_import(
    app: AppHandle,
    library: State<'_, GameLibrary>,
    preview_id: String,
    candidate_id: String,
    content_id: String,
    name: String,
    include_saves: bool,
    source: DownloadSettings,
    use_default_directory: bool,
) -> Result<Game, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("游戏名称需为 1–80 个字符".into());
    }
    let resolved_source = source
        .download_source()
        .map_err(|error| error.to_string())?;
    let mut library = library.0.lock().map_err(|error| error.to_string())?;
    if let Some(error) = &library.load_error {
        return Err(error.clone());
    }
    if library.games.iter().any(|game| game.status == "installing") {
        return Err("已有游戏正在安装，请稍候".into());
    }
    let selection = library
        .picked_import
        .as_ref()
        .filter(|selection| selection.id == preview_id)
        .ok_or("请重新选择来源并查看预览")?;
    let candidate = selection
        .candidates
        .iter()
        .find(|candidate| candidate.id == candidate_id)
        .ok_or("请选择预览中提供的版本")?
        .clone();
    if let Some(reason) = &candidate.unsupported_reason {
        return Err(reason.clone());
    }
    let version = candidate
        .minecraft_version
        .clone()
        .ok_or("无法识别 Minecraft 版本")?;
    let root = candidate
        .content_roots
        .iter()
        .find(|root| root.id == content_id)
        .ok_or("请选择来源数据文件夹")?;
    if include_saves && !root.has_saves {
        return Err("选择的数据文件夹中没有存档，请重新预览".into());
    }
    let base = if use_default_directory {
        default_directory()?
    } else {
        library.picked_directory.clone().ok_or("请先选择保存位置")?
    };
    fs::create_dir_all(&base).map_err(|error| error.to_string())?;
    let id = next_pack_id()?;
    let plan_file = plan_path(&id)?;
    fs::create_dir_all(plan_file.parent().ok_or("无法定位导入记录")?)
        .map_err(|error| error.to_string())?;
    let plan = ImportPlan {
        candidate: candidate.clone(),
        content_id,
        include_saves,
    };
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&plan_file)
        .map_err(|error| error.to_string())?;
    if let Err(error) = serde_json::to_writer(file, &plan) {
        let _ = fs::remove_file(&plan_file);
        return Err(error.to_string());
    }
    let game = Game {
        directory: base.join(&id),
        id: id.clone(),
        name: name.into(),
        version,
        loader_version: candidate.loader_version,
        loader_kind: candidate.loader_kind,
        profile_id: None,
        runtime_overrides: cobble_minecraft::LaunchOverrides::default(),
        download_source: Some(source),
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
        launcher_import_id: Some(id),
    };
    // The saved plan exists before publishing an installing game, so interrupted
    // imports always retry the import pipeline rather than a plain runtime install.
    library.games.push(game.clone());
    if let Err(error) = save(&library.games) {
        library.games.pop();
        let _ = fs::remove_file(plan_file);
        return Err(error);
    }
    drop(library);
    start_install(app, game.clone(), resolved_source);
    Ok(game)
}

pub(super) async fn install(
    game: &Game,
    source: cobble_minecraft::DownloadSource,
    control: cobble_core::OperationControl,
) -> Result<(), String> {
    let id = game.launcher_import_id.as_deref().ok_or("缺少导入记录")?;
    let path = plan_path(id)?;
    let plan: ImportPlan =
        serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    cobble_core::import_launcher_game_controlled(
        &plan.candidate,
        &plan.content_id,
        &game.directory,
        source,
        plan.include_saves,
        control,
    )
    .await
    .map(|_| ())
    .map_err(|error| error.to_string())
}
