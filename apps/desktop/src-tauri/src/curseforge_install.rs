// SPDX-License-Identifier: MIT
//! Native-selected artifacts and opaque plan handles; no renderer filesystem paths.
use super::*;
use cobble_content::curseforge::{InstallPlan, RestrictedFile};

#[derive(Serialize)]
pub struct Preview {
    plan_id: String,
    project_id: String,
    version_id: String,
    mods: Vec<ModInstallItem>,
    missing: Vec<RestrictedFile>,
    ready: bool,
}
fn stored(app: &AppHandle, id: &str, plan_id: &str) -> Result<InstallPlan, String> {
    let library = app.state::<GameLibrary>();
    let library = library.0.lock().map_err(|_| "游戏库不可用")?;
    let (current, plan) = library
        .curseforge_plans
        .get(id)
        .ok_or("请先选择 CurseForge 文件")?;
    if current != plan_id {
        return Err("安装方案已变更，请重新预览".into());
    }
    Ok(plan.clone())
}
async fn preview(plan_id: String, plan: InstallPlan) -> Result<Preview, String> {
    let key = crate::curseforge::credential().ok_or("CurseForge 服务尚未配置")?;
    let plan_for_check = plan.clone();
    let missing = tauri::async_runtime::spawn_blocking(move || {
        cobble_core::missing_curseforge_manual_files(&plan_for_check, key)
    })
    .await
    .map_err(|_| "文件检查任务意外退出")?
    .map_err(|_| "无法验证 CurseForge 文件")?;
    Ok(Preview {
        plan_id,
        project_id: plan.graph.project_id,
        version_id: plan.graph.version_id,
        mods: plan
            .graph
            .mods
            .into_iter()
            .map(|m| ModInstallItem {
                project_id: m.project_id,
                version_id: m.version_id,
                name: m.name,
                version_number: m.version_number,
                required_by: m.required_by,
            })
            .collect(),
        ready: missing.is_empty(),
        missing,
    })
}
#[tauri::command]
pub async fn preview_curseforge_mod_install(
    app: AppHandle,
    id: String,
    project_id: String,
    file_id: String,
) -> Result<Preview, String> {
    let (version, loader) = modrinth_target(&app, &id)?;
    let project = project_id
        .parse::<u32>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or("CurseForge 项目编号无效")?;
    let file = file_id
        .parse::<u32>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or("CurseForge 文件编号无效")?;
    let plan_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间不可用")?
        .as_nanos()
        .to_string();
    {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|_| "游戏库不可用")?;
        library
            .curseforge_preview_ids
            .insert(id.clone(), plan_id.clone());
        library.curseforge_plans.remove(&id);
    }
    let plan = crate::curseforge::client()?
        .resolve_install(project, file, &version, &loader)
        .await
        .map_err(|error| error.to_string())?;
    let directory = game_directory(&app, &id)?;
    cobble_core::validate_curseforge_mod_pins(&directory, &plan.graph)
        .map_err(|error| error.to_string())?;
    {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|_| "游戏库不可用")?;
        if library.curseforge_preview_ids.get(&id) != Some(&plan_id)
            || !library.games.iter().any(|g| g.id == id)
        {
            return Err("安装方案已变更，请重新预览".into());
        }
        library
            .curseforge_plans
            .insert(id, (plan_id.clone(), plan.clone()));
    }
    preview(plan_id, plan).await
}
#[tauri::command]
pub async fn select_curseforge_manual_file(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    plan_id: String,
    project_id: String,
    file_id: String,
) -> Result<Option<Preview>, String> {
    let plan = stored(&app, &id, &plan_id)?;
    if !plan
        .restricted
        .iter()
        .any(|item| item.project_id == project_id && item.file_id == file_id)
    {
        return Err("这个文件不在当前安装方案中".into());
    }
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择 CurseForge 文件")
        .add_filter("Minecraft Mod", &["jar"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let key = crate::curseforge::credential().ok_or("CurseForge 服务尚未配置")?;
    let plan_for_check = plan.clone();
    let path = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        if std::fs::symlink_metadata(&path)
            .map_err(|_| "cannot read selection")?
            .file_type()
            .is_symlink()
        {
            return Err("invalid selection");
        }
        let input = std::fs::File::open(path).map_err(|_| "cannot open selection")?;
        if !input
            .metadata()
            .map_err(|_| "cannot read selection")?
            .is_file()
        {
            return Err("invalid selection");
        }
        cobble_core::import_curseforge_manual_file(
            &plan_for_check,
            &project_id,
            &file_id,
            input,
            key,
        )
        .map_err(|_| "manual_file_mismatch")
    })
    .await
    .map_err(|_| "文件验证任务意外退出")?
    .map_err(str::to_owned)?;
    stored(&app, &id, &plan_id)?; // Dialogs cannot authorize an obsolete plan.
    preview(plan_id, plan).await.map(Some)
}
#[tauri::command]
pub async fn install_curseforge_mod_plan(
    app: AppHandle,
    id: String,
    plan_id: String,
) -> Result<cobble_core::ModrinthInstallOutcome, String> {
    if plan_id.starts_with("update-") {
        return Err("Use the reviewed update plan to apply these changes".into());
    }
    let started = Instant::now();
    let plan = stored(&app, &id, &plan_id)?;
    modrinth_target(&app, &id)?;
    let result = match tasks::run(
        app,
        tasks::TaskRequest::CurseForgeModInstall { game_id: id, plan },
        "安装 CurseForge Mod",
    )
    .await
    {
        Ok(tasks::TaskResult::ModInstall(outcome)) => Ok(outcome),
        Ok(_) => Err("任务类型不匹配".into()),
        Err(error) => Err(error),
    };
    tracing::info!(
        operation = "curseforge_mod_install",
        outcome = if result.is_ok() { "success" } else { "failed" },
        duration_ms = started.elapsed().as_millis() as u64
    );
    result
}
#[tauri::command]
pub async fn open_curseforge_project(project_id: String) -> Result<(), String> {
    let id = project_id
        .parse::<u32>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or("CurseForge 项目编号无效")?;
    let url = crate::curseforge::client()?
        .project(id)
        .await
        .map_err(|error| error.to_string())?
        .website_url
        .ok_or("项目页面不可用")?;
    tauri::async_runtime::spawn_blocking(move || {
        open::that(url).map_err(|_| "无法打开项目页面".to_owned())
    })
    .await
    .map_err(|_| "浏览器任务意外退出")?
}
#[tauri::command]
pub async fn list_game_curseforge_mods(
    app: AppHandle,
    id: String,
) -> Result<Vec<cobble_core::ManagedModSummary>, String> {
    let directory = game_directory(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        cobble_core::list_managed_curseforge_mods(&directory)
            .map_err(|_| "无法读取 CurseForge Mod 记录".to_owned())
    })
    .await
    .map_err(|_| "Mod 检查任务意外退出")?
}
#[tauri::command]
pub async fn check_game_curseforge_updates(
    app: AppHandle,
    id: String,
) -> Result<Vec<cobble_core::ModUpdateCandidate>, String> {
    let (version, loader) = modrinth_target(&app, &id)?;
    let directory = game_directory(&app, &id)?;
    let client = crate::curseforge::client()?;
    cobble_core::check_curseforge_updates(&directory, &client, &version, &loader)
        .await
        .map_err(|error| error.to_string())
}

#[derive(Serialize)]
pub struct UpdatePreview {
    plan_id: String,
    changes: Vec<cobble_core::ModUpdateChange>,
    missing: Vec<RestrictedFile>,
    ready: bool,
}
#[tauri::command]
pub async fn preview_game_curseforge_updates(
    app: AppHandle,
    id: String,
    project_ids: Vec<String>,
) -> Result<UpdatePreview, String> {
    let (version, loader) = modrinth_target(&app, &id)?;
    let directory = game_directory(&app, &id)?;
    let plan_id = format!(
        "update-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    );
    {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|error| error.to_string())?;
        library
            .curseforge_preview_ids
            .insert(id.clone(), plan_id.clone());
        library.curseforge_plans.remove(&id);
        library.update_plans.remove(&id);
    }
    let client = crate::curseforge::client()?;
    let (plan, downloads) = cobble_core::prepare_curseforge_updates(
        &directory,
        &client,
        &version,
        &loader,
        &project_ids,
    )
    .await
    .map_err(|error| error.to_string())?;
    let file_preview = preview(plan_id.clone(), downloads.clone()).await?;
    let response = UpdatePreview {
        plan_id: plan_id.clone(),
        changes: plan.changes.clone(),
        missing: file_preview.missing,
        ready: file_preview.ready,
    };
    {
        let state = app.state::<GameLibrary>();
        let mut library = state.0.lock().map_err(|error| error.to_string())?;
        if library.curseforge_preview_ids.get(&id) != Some(&plan_id)
            || !library.games.iter().any(|game| game.id == id)
        {
            return Err("Update plan changed; preview the updates again".into());
        }
        library
            .curseforge_plans
            .insert(id.clone(), (plan_id.clone(), downloads));
        library.update_plans.insert(id, (plan_id, plan));
    }
    Ok(response)
}
