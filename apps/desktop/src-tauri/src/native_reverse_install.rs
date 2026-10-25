// SPDX-License-Identifier: MIT
use super::*;
use tauri::WebviewWindow;

pub(super) async fn preflight(
    game: &Game,
    plan: Option<&cobble_content::EnvironmentPlan>,
    configs: &[cobble_core::environment_install::SharedConfig],
    personal_options: Option<&cobble_core::personal_options::PersonalOptions>,
    control: cobble_core::OperationControl,
) -> Result<(), String> {
    let source = game
        .download_source
        .clone()
        .map(Ok)
        .unwrap_or_else(cobble_minecraft::read_download_settings)
        .map_err(|e| e.to_string())?;
    let java = cobble_core::capture_java_choice(&game.runtime_overrides.java)
        .map_err(|e| e.to_string())?;
    let mut space = native_readiness_space::estimate(
        &game.version,
        game.loader_kind.as_deref(),
        plan,
        &game.directory,
        source,
        &java,
        control,
    )
    .await?;
    native_readiness_space::include_shared_configs(&mut space, &game.directory, configs)?;
    native_readiness_space::include_personal_options(
        &mut space,
        &game.directory,
        personal_options,
    )?;
    if !space.can_proceed {
        return Err("SPACE_INSUFFICIENT".into());
    }
    Ok(())
}

pub(super) async fn install_initial(
    _app: &AppHandle,
    game: &Game,
    plan: &cobble_content::EnvironmentPlan,
    control: cobble_core::OperationControl,
) -> Result<(), String> {
    native_reverse::validate_target(game, plan)?;
    cobble_core::environment_install::install_environment_plan_controlled(
        plan,
        &game.directory,
        crate::curseforge::credential(),
        control,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize)]
pub struct Requirements {
    pub missing: Vec<cobble_content::curseforge::RestrictedFile>,
    pub ready: bool,
}
pub(super) async fn requirements(
    plan: cobble_content::EnvironmentPlan,
) -> Result<Requirements, String> {
    let missing = tauri::async_runtime::spawn_blocking(move || {
        cobble_core::environment_install::missing_environment_manual_files(&plan)
    })
    .await
    .map_err(|_| "ENVIRONMENT_FILE_CHECK_FAILED")?
    .map_err(|e| e.to_string())?;
    Ok(Requirements {
        ready: missing.is_empty(),
        missing,
    })
}
pub(super) async fn select_manual(
    window: &WebviewWindow,
    plan: cobble_content::EnvironmentPlan,
    project: String,
    version: String,
) -> Result<bool, String> {
    if !plan.curseforge.as_ref().is_some_and(|p| {
        p.restricted
            .iter()
            .any(|r| r.project_id == project && r.file_id == version)
    }) {
        return Err("ENVIRONMENT_MANUAL_FILE_INVALID".into());
    }
    let Some(selected) = rfd::AsyncFileDialog::new()
        .set_parent(window)
        .set_title("选择 CurseForge 文件")
        .add_filter("Minecraft Mod", &["jar"])
        .pick_file()
        .await
    else {
        return Ok(false);
    };
    let path = selected.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|_| "ENVIRONMENT_MANUAL_FILE_INVALID")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("ENVIRONMENT_MANUAL_FILE_INVALID".to_owned());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("ENVIRONMENT_MANUAL_FILE_INVALID".into());
            }
        }
        let file = std::fs::File::open(path).map_err(|_| "ENVIRONMENT_MANUAL_FILE_INVALID")?;
        cobble_core::environment_install::import_environment_manual_file(
            &plan, &project, &version, file,
        )
        .map_err(|_| "ENVIRONMENT_MANUAL_FILE_MISMATCH".into())
    })
    .await
    .map_err(|_| "ENVIRONMENT_FILE_CHECK_FAILED")??;
    Ok(true)
}

#[tauri::command]
pub async fn get_task_environment_requirements(
    app: AppHandle,
    task_id: String,
) -> Result<Requirements, String> {
    requirements(tasks::environment_plan_for_task(&app, &task_id)?).await
}
#[tauri::command]
pub async fn select_task_environment_manual_file(
    window: WebviewWindow,
    app: AppHandle,
    task_id: String,
    project_id: String,
    file_id: String,
) -> Result<Option<Requirements>, String> {
    let plan = tasks::environment_plan_for_task(&app, &task_id)?;
    if !select_manual(&window, plan.clone(), project_id, file_id).await? {
        return Ok(None);
    }
    // The record, not a renderer path or a page-owned plan, authorizes the file.
    tasks::environment_plan_for_task(&app, &task_id)?;
    requirements(plan).await.map(Some)
}
