// SPDX-License-Identifier: MIT
//! Opaque pinned archive/plan workflow. Renderer supplies IDs, never paths or URLs.
use super::*;
use cobble_content::curseforge::{PackInstallPlan, RestrictedFile};
use cobble_core::{CurseForgePackInspection, CurseForgePackPlan};
use std::io::{Read, Write};

#[derive(Clone)]
pub(super) struct Selection {
    id: String,
    pack: cobble_content::CurseForgePack,
    archive_sha256: String,
    origin: Option<pack_origin::PackOrigin>,
    plan_id: Option<String>,
    plan: Option<CurseForgePackPlan>,
}
#[derive(Serialize)]
pub struct Preview {
    pub preview_id: String,
    pub name: String,
    version_id: String,
    minecraft_version: String,
    loader_kind: Option<String>,
    loader_version: Option<String>,
    required_files: usize,
    optional_files: Vec<OptionalFile>,
    overrides: usize,
    selected_optional_files: Vec<String>,
    plan_id: Option<String>,
    files: Vec<ResolvedFile>,
    missing: Vec<RestrictedFile>,
    ready: bool,
}
#[derive(Serialize)]
struct OptionalFile {
    id: String,
    project_id: String,
    file_id: String,
}
#[derive(Serialize)]
struct ResolvedFile {
    project_id: String,
    file_id: String,
    name: String,
    file_name: String,
    destination: String,
    size_bytes: u64,
    required_by: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    schema_version: u32,
    plan: CurseForgePackPlan,
}
fn archive(id: &str) -> Result<PathBuf, String> {
    Ok(pack_archive_path(id)?.with_extension("curseforge.zip"))
}
fn receipt(id: &str) -> Result<PathBuf, String> {
    Ok(pack_archive_path(id)?.with_extension("curseforge.json"))
}
pub(super) fn is_source(id: &str) -> Result<bool, String> {
    receipt(id)?.try_exists().map_err(|error| error.to_string())
}
fn selection(app: &AppHandle, id: &str, plan_id: Option<&str>) -> Result<Selection, String> {
    let state = app.state::<GameLibrary>();
    let library = state.0.lock().map_err(|_| "游戏库不可用")?;
    let selected = library
        .picked_curseforge_pack
        .as_ref()
        .filter(|selected| selected.id == id)
        .ok_or("整合包已变更，请重新选择")?;
    if plan_id.is_some_and(|id| selected.plan_id.as_deref() != Some(id)) {
        return Err("安装方案已变更，请重新准备文件".into());
    }
    Ok(selected.clone())
}
fn missing(plan: &CurseForgePackPlan) -> Result<Vec<RestrictedFile>, String> {
    if plan.files.restricted.is_empty() {
        return Ok(vec![]);
    }
    let key = crate::curseforge::credential().ok_or("CurseForge 服务尚未配置")?;
    cobble_core::missing_curseforge_pack_manual_files(plan, key).map_err(|error| error.to_string())
}
fn preview(selected: &Selection) -> Result<Preview, String> {
    let missing = selected
        .plan
        .as_ref()
        .map(missing)
        .transpose()?
        .unwrap_or_default();
    let files = selected
        .plan
        .iter()
        .flat_map(|plan| &plan.files.files)
        .map(|file| {
            Ok(ResolvedFile {
                project_id: file.project_id.clone(),
                file_id: file.file_id.clone(),
                name: file.name.clone(),
                file_name: file.file.filename.clone(),
                destination: cobble_core::curseforge_pack_file_path(file)
                    .map_err(|error| error.to_string())?,
                size_bytes: file.file.size,
                required_by: file.required_by.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Preview {
        preview_id: selected.id.clone(),
        name: selected.pack.name.clone(),
        version_id: selected.pack.version.clone(),
        minecraft_version: selected.pack.minecraft_version.clone(),
        loader_kind: selected
            .pack
            .loader
            .as_ref()
            .map(|loader| loader.kind.clone()),
        loader_version: selected
            .pack
            .loader
            .as_ref()
            .map(|loader| loader.version.clone()),
        required_files: selected
            .pack
            .files
            .iter()
            .filter(|file| file.required)
            .count(),
        optional_files: selected
            .pack
            .files
            .iter()
            .filter(|file| !file.required)
            .map(|file| OptionalFile {
                id: format!("{}:{}", file.project_id, file.file_id),
                project_id: file.project_id.to_string(),
                file_id: file.file_id.to_string(),
            })
            .collect(),
        overrides: selected.pack.overrides.len(),
        selected_optional_files: selected
            .plan
            .as_ref()
            .map(|plan| plan.optional_files.clone())
            .unwrap_or_default(),
        plan_id: selected.plan_id.clone(),
        files,
        ready: selected.plan.is_some() && missing.is_empty(),
        missing,
    })
}
fn store(
    app: &AppHandle,
    id: String,
    inspected: CurseForgePackInspection,
    origin: Option<pack_origin::PackOrigin>,
) -> Result<Preview, String> {
    let selected = Selection {
        id,
        pack: inspected.pack,
        archive_sha256: inspected.archive_sha256,
        origin,
        plan_id: None,
        plan: None,
    };
    let output = preview(&selected)?;
    app.state::<GameLibrary>()
        .0
        .lock()
        .map_err(|_| "游戏库不可用")?
        .picked_curseforge_pack = Some(selected);
    Ok(output)
}
fn regular(path: &std::path::Path) -> Result<fs::File, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "无法读取所选文件")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("请选择普通文件".into());
    }
    fs::File::open(path).map_err(|_| "无法打开所选文件".into())
}
#[tauri::command]
pub async fn preview_curseforge_pack(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<Option<Preview>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("选择 CurseForge 整合包")
        .add_filter("CurseForge 整合包", &["zip"])
        .set_parent(&window)
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let id = next_pack_id()?;
        let destination = archive(&id)?;
        fs::create_dir_all(destination.parent().ok_or("整合包目录不可用")?)
            .map_err(|error| error.to_string())?;
        let mut input = regular(&path)?;
        if input.metadata().map_err(|_| "无法读取整合包")?.len() > 16 * 1024 * 1024 * 1024 {
            return Err("整合包超过 16 GiB".into());
        }
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|error| error.to_string())?;
        let copied = std::io::copy(
            &mut Read::by_ref(&mut input).take(16 * 1024 * 1024 * 1024 + 1),
            &mut output,
        )
        .map_err(|error| error.to_string());
        let result = copied.and_then(|size| {
            if size > 16 * 1024 * 1024 * 1024 {
                return Err("整合包超过 16 GiB".into());
            }
            output.sync_all().map_err(|error| error.to_string())?;
            drop(output);
            cobble_core::inspect_curseforge_pack(&destination).map_err(|error| error.to_string())
        });
        match result {
            Ok(inspected) => store(&app, id, inspected, None).map(Some),
            Err(error) => {
                let _ = fs::remove_file(destination);
                Err(error)
            }
        }
    })
    .await
    .map_err(|_| "整合包检查任务意外退出")?
}
#[tauri::command]
pub async fn preview_curseforge_online_pack(
    window: WebviewWindow,
    app: AppHandle,
    project_id: String,
    file_id: String,
) -> Result<Option<Preview>, String> {
    let project = number(&project_id)?;
    let file = number(&file_id)?;
    let selection = crate::curseforge::client()?
        .resolve_pack_archive(project, file)
        .await
        .map_err(|error| error.to_string())?;
    let origin = pack_origin::PackOrigin::curseforge(project, file);
    if selection.restricted.is_some() {
        let Some(file) = rfd::AsyncFileDialog::new()
            .set_parent(&window)
            .set_title("选择手动下载的 CurseForge 整合包")
            .add_filter("CurseForge 整合包", &["zip"])
            .pick_file()
            .await
        else {
            return Ok(None);
        };
        let path = file.path().to_owned();
        let key = crate::curseforge::credential().ok_or("CurseForge 服务尚未配置")?;
        return tauri::async_runtime::spawn_blocking(move || {
            let id = next_pack_id()?;
            let destination = archive(&id)?;
            fs::create_dir_all(destination.parent().ok_or("整合包目录不可用")?)
                .map_err(|error| error.to_string())?;
            let inspected = cobble_core::import_curseforge_pack_archive(
                &selection,
                regular(&path)?,
                &destination,
                key,
            )
            .map_err(|_| "manual_file_mismatch")?;
            store(&app, id, inspected, Some(origin)).map(Some)
        })
        .await
        .map_err(|_| "文件验证任务意外退出")?;
    }
    let id = next_pack_id()?;
    match tasks::run(
        app,
        tasks::TaskRequest::CurseForgePackDownload {
            archive_id: id,
            project_id: project,
            file_id: file,
        },
        "下载 CurseForge 整合包",
    )
    .await?
    {
        tasks::TaskResult::CurseForgePack(preview) => Ok(Some(preview)),
        _ => Err("任务类型不匹配".into()),
    }
}
pub(super) async fn download(
    app: &AppHandle,
    id: &str,
    project: u32,
    file: u32,
    control: cobble_core::OperationControl,
) -> Result<Preview, String> {
    let selected = crate::curseforge::client()?
        .resolve_pack_archive(project, file)
        .await
        .map_err(|error| error.to_string())?;
    let key = crate::curseforge::credential().ok_or("CurseForge 服务尚未配置")?;
    let path = archive(id)?;
    fs::create_dir_all(path.parent().ok_or("整合包目录不可用")?)
        .map_err(|error| error.to_string())?;
    let inspected =
        cobble_core::download_curseforge_pack_archive_controlled(&selected, &path, key, control)
            .await
            .map_err(|error| error.to_string())?;
    store(
        app,
        id.to_owned(),
        inspected,
        Some(pack_origin::PackOrigin::curseforge(project, file)),
    )
}
fn number(value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|number| *number > 0 && number.to_string() == value)
        .ok_or_else(|| "CurseForge 文件编号无效".into())
}
#[tauri::command]
pub async fn prepare_curseforge_pack(
    app: AppHandle,
    preview_id: String,
    optional_files: Vec<String>,
) -> Result<Preview, String> {
    let selected = selection(&app, &preview_id, None)?;
    let loader = selected
        .pack
        .loader
        .as_ref()
        .map(|loader| loader.kind.as_str());
    if !cobble_content::supported_pack_target(
        &selected.pack.minecraft_version,
        loader.unwrap_or("minecraft"),
    ) {
        return Err("此整合包的游戏版本或加载器暂不支持".into());
    }
    let pins = cobble_core::curseforge_pack_pins(&selected.pack, &optional_files)
        .map_err(|error| error.to_string())?;
    let plan_id = next_pack_id()?;
    {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|_| "游戏库不可用")?;
        let current = library
            .picked_curseforge_pack
            .as_mut()
            .filter(|current| current.id == preview_id)
            .ok_or("整合包已变更")?;
        current.plan_id = Some(plan_id.clone());
        current.plan = None;
    }
    let files = if pins.is_empty() {
        PackInstallPlan {
            files: vec![],
            restricted: vec![],
            incompatible: vec![],
        }
    } else {
        crate::curseforge::client()?
            .resolve_pack_files(&pins, &selected.pack.minecraft_version, loader)
            .await
            .map_err(|error| error.to_string())?
    };
    let plan = CurseForgePackPlan {
        archive_sha256: selected.archive_sha256,
        optional_files,
        files,
    };
    cobble_core::validate_curseforge_pack_plan(&selected.pack, &plan)
        .map_err(|error| error.to_string())?;
    let resolved = {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|_| "游戏库不可用")?;
        let current = library
            .picked_curseforge_pack
            .as_mut()
            .filter(|current| {
                current.id == preview_id && current.plan_id.as_ref() == Some(&plan_id)
            })
            .ok_or("整合包方案已变更")?;
        current.plan = Some(plan);
        current.clone()
    };
    tauri::async_runtime::spawn_blocking(move || preview(&resolved))
        .await
        .map_err(|_| "文件检查任务意外退出")?
}
#[tauri::command]
pub async fn select_curseforge_pack_manual_file(
    window: WebviewWindow,
    app: AppHandle,
    preview_id: String,
    plan_id: String,
    project_id: String,
    file_id: String,
) -> Result<Option<Preview>, String> {
    let selected = selection(&app, &preview_id, Some(&plan_id))?;
    let plan = selected.plan.ok_or("请先准备整合包文件")?;
    if !plan
        .files
        .restricted
        .iter()
        .any(|item| item.project_id == project_id && item.file_id == file_id)
    {
        return Err("这个文件不在当前安装方案中".into());
    }
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择手动下载的文件")
        .add_filter("CurseForge 文件", &["jar", "zip"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_owned();
    let key = crate::curseforge::credential().ok_or("CurseForge 服务尚未配置")?;
    tauri::async_runtime::spawn_blocking(move || {
        cobble_core::import_curseforge_pack_manual_file(
            &plan,
            &project_id,
            &file_id,
            regular(&path)?,
            key,
        )
        .map_err(|_| "manual_file_mismatch")?;
        let current = selection(&app, &preview_id, Some(&plan_id))?;
        preview(&current).map(Some)
    })
    .await
    .map_err(|_| "文件验证任务意外退出")?
}
fn read_source(id: &str) -> Result<Source, String> {
    let path = receipt(id)?;
    let file = regular(&path)?;
    if file.metadata().map_err(|_| "无法读取整合包记录")?.len() > 8 * 1024 * 1024 {
        return Err("整合包记录过大".into());
    }
    let source: Source =
        serde_json::from_reader(file.take(8 * 1024 * 1024 + 1)).map_err(|_| "整合包记录无效")?;
    if source.schema_version != 1 {
        return Err("不支持这个整合包记录版本".into());
    }
    Ok(source)
}
fn write_source(id: &str, plan: &CurseForgePackPlan) -> Result<(), String> {
    let path = receipt(id)?;
    let mut file = tempfile::NamedTempFile::new_in(path.parent().ok_or("整合包目录不可用")?)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(
        &mut file,
        &Source {
            schema_version: 1,
            plan: plan.clone(),
        },
    )
    .map_err(|error| error.to_string())?;
    file.write_all(b"\n").map_err(|error| error.to_string())?;
    file.as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    file.persist_noclobber(path)
        .map_err(|_| "整合包记录已存在，请重新预览")?;
    Ok(())
}
#[tauri::command]
pub async fn create_curseforge_pack_game(
    app: AppHandle,
    name: String,
    preview_id: String,
    plan_id: String,
    source: DownloadSettings,
    use_default_directory: bool,
) -> Result<Game, String> {
    if name.trim().is_empty() {
        return Err("请填写游戏名称".into());
    }
    let selected = selection(&app, &preview_id, Some(&plan_id))?;
    let plan = selected.plan.as_ref().ok_or("请先准备整合包文件")?;
    let checked = selected.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if !preview(&checked)?.ready {
            return Err("请先验证所有手动下载的文件".into());
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|_| "文件检查任务意外退出")??;
    let resolved_source = source
        .download_source()
        .map_err(|error| error.to_string())?;
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|_| "游戏库不可用")?;
    if let Some(error) = &library.load_error {
        return Err(error.clone());
    }
    if library
        .picked_curseforge_pack
        .as_ref()
        .is_none_or(|current| {
            current.id != preview_id || current.plan_id.as_deref() != Some(&plan_id)
        })
    {
        return Err("安装方案已变更，请重新预览".into());
    }
    if library.games.iter().any(|game| game.status == "installing") {
        return Err("已有游戏正在安装，请稍候".into());
    }
    let base = if use_default_directory {
        default_directory()?
    } else {
        library.picked_directory.clone().ok_or("请先选择保存位置")?
    };
    let id = next_pack_id()?;
    let game = Game {
        directory: base.join(&id),
        id,
        name: name.trim().into(),
        last_launched_at: None,
        appearance_preset: game_appearance::default_preset(),
        icon_ref: None,
        cover_ref: None,
        pinned: false,
        version: selected.pack.minecraft_version.clone(),
        loader_version: selected
            .pack
            .loader
            .as_ref()
            .map(|loader| loader.version.clone()),
        loader_kind: selected
            .pack
            .loader
            .as_ref()
            .map(|loader| loader.kind.clone()),
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
        pack_name: Some(selected.pack.name),
        pack_version: Some(selected.pack.version),
        pack_source_id: Some(preview_id.clone()),
        pack_origin: selected.origin.clone(),
        pack_optional_files: plan.optional_files.clone(),
        pack_mod_files: plan
            .files
            .files
            .iter()
            .map(cobble_core::curseforge_pack_file_path)
            .chain(
                selected
                    .pack
                    .overrides
                    .iter()
                    .map(|entry| Ok(entry.path.clone())),
            )
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?,
        launcher_import_id: None,
    };
    write_source(&preview_id, plan)?;
    library.games.push(game.clone());
    if let Err(error) = save(&library.games) {
        library.games.pop();
        let _ = fs::remove_file(receipt(&preview_id)?);
        return Err(error);
    }
    library.picked_curseforge_pack = None;
    drop(library);
    start_install(app, game.clone(), resolved_source);
    Ok(game)
}
pub(super) async fn install(
    game: &Game,
    source: cobble_minecraft::DownloadSource,
    control: cobble_core::OperationControl,
) -> Result<(), String> {
    let id = game.pack_source_id.as_deref().ok_or("缺少整合包来源")?;
    let selected = read_source(id)?;
    let key = if selected.plan.files.files.is_empty() {
        String::new()
    } else {
        crate::curseforge::credential().ok_or("CurseForge 服务尚未配置")?
    };
    cobble_core::install_curseforge_pack_controlled(
        &archive(id)?,
        &game.directory,
        source,
        &selected.plan,
        key,
        control,
    )
    .await
    .map_err(|error| error.to_string())
}
#[tauri::command]
pub async fn get_task_curseforge_pack_preview(
    app: AppHandle,
    task_id: String,
) -> Result<Preview, String> {
    let (id, origin) = tasks::curseforge_pack_result_id(&app, &task_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let inspected = cobble_core::inspect_curseforge_pack(&archive(&id)?)
            .map_err(|error| error.to_string())?;
        store(&app, id, inspected, Some(origin))
    })
    .await
    .map_err(|_| "整合包检查任务意外退出")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_requires_prepared_plan_and_preserves_optional_ids() {
        let mut selected = Selection {
            id: "fixture".into(),
            archive_sha256: "a".repeat(64),
            origin: None,
            plan_id: None,
            plan: None,
            pack: cobble_content::CurseForgePack {
                name: "Original fixture".into(),
                version: "1".into(),
                minecraft_version: "1.21.1".into(),
                loader: None,
                files: vec![cobble_content::CurseForgePackFile {
                    project_id: 1,
                    file_id: 10,
                    required: false,
                }],
                overrides: vec![],
            },
        };
        let unprepared = preview(&selected).unwrap();
        assert!(!unprepared.ready);
        assert_eq!(unprepared.optional_files[0].id, "1:10");
        selected.plan_id = Some("plan-fixture".into());
        selected.plan = Some(CurseForgePackPlan {
            archive_sha256: selected.archive_sha256.clone(),
            optional_files: vec![],
            files: PackInstallPlan {
                files: vec![],
                restricted: vec![],
                incompatible: vec![],
            },
        });
        let prepared = preview(&selected).unwrap();
        assert!(prepared.ready);
        let json = serde_json::to_value(prepared).unwrap();
        assert_eq!(json["plan_id"], "plan-fixture");
        assert!(json["files"].as_array().unwrap().is_empty());
        assert_eq!(json["optional_files"][0]["project_id"], "1");
    }
}
