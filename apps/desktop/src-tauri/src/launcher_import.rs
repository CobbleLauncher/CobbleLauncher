// SPDX-License-Identifier: MIT
use super::*;
use cobble_core::LauncherImportCandidate;
#[path = "launcher_import/migration.rs"]
mod migration;
pub use migration::ImportResult;

#[tauri::command]
pub fn get_launcher_import_result(
    library: State<'_, GameLibrary>,
    id: String,
) -> Result<Option<ImportResult>, String> {
    migration::get_launcher_import_result(library, id)
}

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
    source_display: Option<String>,
}
#[derive(Serialize)]
struct ContentRoot {
    content_id: String,
    label: String,
    has_saves: bool,
    source_display: String,
}

#[derive(Serialize, Deserialize)]
struct ImportPlan {
    candidate: LauncherImportCandidate,
    content_id: String,
    include_saves: bool,
    #[serde(default)]
    move_source: bool,
    #[serde(default)]
    result: Option<ImportResult>,
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
        store_preview(&app, directory.display().to_string(), candidates).map(Some)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn discover_launcher_import(app: AppHandle) -> Result<ImportPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut roots = vec![];
        if let Some(data) = dirs::data_dir() {
            roots.push(data.join(".minecraft"));
        }
        roots.extend(
            [
                dirs::desktop_dir(),
                dirs::document_dir(),
                dirs::download_dir(),
            ]
            .into_iter()
            .flatten(),
        );
        if let Some(desktop) = dirs::desktop_dir()
            && let Ok(entries) = fs::read_dir(desktop)
        {
            for entry in entries.take(256).flatten() {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if (name.contains("pcl") || name.contains("plain craft launcher"))
                    && name.ends_with(".lnk")
                    && let Ok(target) = cobble_platform::read_shortcut_target(&entry.path())
                    && let Some(parent) = target.parent()
                {
                    roots.push(parent.to_owned());
                }
            }
        }
        let candidates =
            cobble_core::discover_launcher_imports(&roots).map_err(|error| error.to_string())?;
        store_preview(&app, "PCL / Minecraft".into(), candidates)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn store_preview(
    app: &AppHandle,
    source_display: String,
    candidates: Vec<LauncherImportCandidate>,
) -> Result<ImportPreview, String> {
    let id = next_pack_id()?;
    let preview = ImportPreview {
        preview_id: id.clone(),
        source_display,
        candidates: candidates
            .iter()
            .map(|item| ImportCandidate {
                candidate_id: item.id.clone(),
                name: item.name.clone(),
                minecraft_version: item.minecraft_version.clone(),
                loader_kind: item.loader_kind.clone(),
                loader_version: item.loader_version.clone(),
                supported: item.unsupported_reason.is_none() && item.minecraft_version.is_some(),
                unsupported_reason: item.unsupported_reason.clone(),
                warnings: item.warnings.clone(),
                source_display: item
                    .runtime_source
                    .as_ref()
                    .map(|source| source.directory.display().to_string()),
                content_roots: item
                    .content_roots
                    .iter()
                    .map(|root| ContentRoot {
                        content_id: root.id.clone(),
                        label: root.label.clone(),
                        has_saves: root.has_saves,
                        source_display: root.directory.display().to_string(),
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
    Ok(preview)
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
    move_source: Option<bool>,
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
    if move_source.unwrap_or(false) {
        if content_id != "isolated" {
            return Err("仅支持迁移版本独立目录；共享游戏目录请使用复制导入".into());
        }
        migration::ensure_stopped(&candidate, &content_id)?;
    }
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
        move_source: move_source.unwrap_or(false),
        result: None,
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
        last_launched_at: None,
        appearance_preset: game_appearance::default_preset(),
        icon_ref: None,
        cover_ref: None,
        pinned: false,
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
        pack_origin: None,
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
    native_readiness_space::workers::launcher_import(
        game,
        plan.candidate.clone(),
        plan.content_id.clone(),
        plan.include_saves,
        control.clone(),
    )
    .await?;
    cobble_core::import_launcher_game_controlled(
        &plan.candidate,
        &plan.content_id,
        &game.directory,
        source,
        plan.include_saves,
        control.clone(),
    )
    .await
    .map_err(|error| error.to_string())?;
    migration::finish(game, &plan, control).await;
    Ok(())
}

/// Recover the core-complete/native-result gap without recopying a source game.
pub(super) async fn reconcile_completed(
    game: &Game,
    control: cobble_core::OperationControl,
) -> Result<(), String> {
    let Some(id) = game.launcher_import_id.as_deref() else {
        return Ok(());
    };
    let plan: ImportPlan =
        serde_json::from_slice(&fs::read(plan_path(id)?).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if plan.result.is_none() {
        migration::finish(game, &plan, control).await;
    } else {
        migration::reconcile_report(game, &plan)?;
    }
    Ok(())
}
