// SPDX-License-Identifier: MIT
//! Restricted config recovery adapter for the optional Config Doctor frontend.
use cobble_plugin_api::{PermissionNode, PluginManifest};
use serde::Deserialize;
use serde_json::{Value, to_value};
use tauri::AppHandle;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameTarget {
    pub game_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigTarget {
    pub game_id: String,
    pub config_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupTarget {
    pub game_id: String,
    pub config_id: String,
    pub backup_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteRequest {
    pub game_id: String,
    pub config_id: String,
    pub expected_revision: String,
    pub text: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreRequest {
    pub game_id: String,
    pub config_id: String,
    pub backup_id: String,
    pub expected_revision: String,
}

fn permitted(manifest: &PluginManifest, write: bool) -> Result<(), String> {
    let permissions = manifest.permissions.as_deref().unwrap_or_default();
    if !permissions.contains(&PermissionNode::GameConfigRead {})
        || (write && !permissions.contains(&PermissionNode::GameConfigWrite {}))
    {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    Ok(())
}
fn target(
    auth: &super::plugin_views::ViewAuthorization,
    game_id: &str,
    write: bool,
) -> Result<(), String> {
    permitted(&auth.manifest, write)?;
    if game_id.is_empty()
        || game_id.len() > 128
        || !game_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || auth
            .instance_id
            .as_deref()
            .is_some_and(|bound| bound != game_id)
    {
        return Err("PLUGIN_GAME_SCOPE".into());
    }
    Ok(())
}
fn config_id(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Err("CONFIG_NOT_FOUND".into())
    } else {
        Ok(())
    }
}
fn backup_id(value: &str) -> Result<(), String> {
    let Some((timestamp, digest)) = value.split_once('-') else {
        return Err("CONFIG_BACKUP_NOT_FOUND".into());
    };
    if timestamp.is_empty()
        || timestamp.len() > 39
        || !timestamp.bytes().all(|byte| byte.is_ascii_digit())
        || digest.len() != 64
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        Err("CONFIG_BACKUP_NOT_FOUND".into())
    } else {
        Ok(())
    }
}
fn revision(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Err("CONFIG_CHANGED".into())
    } else {
        Ok(())
    }
}
fn text(value: &str) -> Result<(), String> {
    if value.len() > 1024 * 1024 || value.contains('\0') {
        Err("CONFIG_TOO_LARGE".into())
    } else {
        Ok(())
    }
}
fn supported(manifest: &PluginManifest, name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        manifest
            .file_extensions
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(extension))
    })
}
fn safe_document(value: Value, manifest: &PluginManifest) -> Result<Value, String> {
    if !supported(
        manifest,
        value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    ) {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    Ok(value)
}
fn display_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control())
        .take(160)
        .collect()
}

pub async fn list(
    app: AppHandle,
    auth: super::plugin_views::ViewAuthorization,
    args: GameTarget,
) -> Result<Value, String> {
    target(&auth, &args.game_id, false)?;
    let files = crate::games::game_config::list_game_configs(app, args.game_id).await?;
    let value = to_value(files).map_err(|_| "PLUGIN_FRONTEND_FAILED")?;
    let Some(files) = value.as_array() else {
        return Err("PLUGIN_FRONTEND_FAILED".into());
    };
    Ok(Value::Array(
        files
            .iter()
            .filter(|file| {
                supported(
                    &auth.manifest,
                    file.get("name").and_then(Value::as_str).unwrap_or_default(),
                )
            })
            .cloned()
            .collect(),
    ))
}
pub async fn read(
    app: AppHandle,
    auth: super::plugin_views::ViewAuthorization,
    args: ConfigTarget,
) -> Result<Value, String> {
    target(&auth, &args.game_id, false)?;
    config_id(&args.config_id)?;
    let document =
        crate::games::game_config::read_game_config(app, args.game_id, args.config_id).await?;
    safe_document(
        to_value(document).map_err(|_| "PLUGIN_FRONTEND_FAILED")?,
        &auth.manifest,
    )
}
pub async fn read_backup(
    app: AppHandle,
    auth: super::plugin_views::ViewAuthorization,
    args: BackupTarget,
) -> Result<Value, String> {
    target(&auth, &args.game_id, false)?;
    config_id(&args.config_id)?;
    backup_id(&args.backup_id)?;
    let current = crate::games::game_config::read_game_config(
        app.clone(),
        args.game_id.clone(),
        args.config_id.clone(),
    )
    .await?;
    safe_document(
        to_value(current).map_err(|_| "PLUGIN_FRONTEND_FAILED")?,
        &auth.manifest,
    )?;
    let document = crate::games::game_config::read_game_config_backup(
        app,
        args.game_id,
        args.config_id,
        args.backup_id,
    )
    .await?;
    to_value(document).map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
}
pub async fn write(
    app: AppHandle,
    auth: super::plugin_views::ViewAuthorization,
    args: WriteRequest,
) -> Result<Value, String> {
    target(&auth, &args.game_id, true)?;
    config_id(&args.config_id)?;
    revision(&args.expected_revision)?;
    text(&args.text)?;
    let current = crate::games::game_config::read_game_config(
        app.clone(),
        args.game_id.clone(),
        args.config_id.clone(),
    )
    .await?;
    let current = safe_document(
        to_value(current).map_err(|_| "PLUGIN_FRONTEND_FAILED")?,
        &auth.manifest,
    )?;
    if current["revision"].as_str() != Some(&args.expected_revision) {
        return Err("CONFIG_CHANGED".into());
    }
    let name = display_name(current["name"].as_str().ok_or("PLUGIN_FRONTEND_FAILED")?);
    let (title, details) = if auth.locale == "zh-CN" {
        (
            "保存配置",
            format!("将保存 {name}。CobbleLauncher 会先创建备份。"),
        )
    } else {
        (
            "Save config",
            format!("Save {name}. CobbleLauncher will create a backup first."),
        )
    };
    if !super::plugin_confirm::ask(&app, &auth, title, &details).await? {
        return Err("PLUGIN_ACTION_CANCELLED".into());
    }
    let latest = crate::games::game_config::read_game_config(
        app.clone(),
        args.game_id.clone(),
        args.config_id.clone(),
    )
    .await?;
    if to_value(latest).map_err(|_| "PLUGIN_FRONTEND_FAILED")?["revision"].as_str()
        != Some(&args.expected_revision)
    {
        return Err("CONFIG_CHANGED".into());
    }
    let document = crate::games::game_config::save_game_config(
        app,
        args.game_id,
        args.config_id,
        args.text,
        args.expected_revision,
    )
    .await?;
    safe_document(
        to_value(document).map_err(|_| "PLUGIN_FRONTEND_FAILED")?,
        &auth.manifest,
    )
}
pub async fn restore(
    app: AppHandle,
    auth: super::plugin_views::ViewAuthorization,
    args: RestoreRequest,
) -> Result<Value, String> {
    target(&auth, &args.game_id, true)?;
    config_id(&args.config_id)?;
    backup_id(&args.backup_id)?;
    revision(&args.expected_revision)?;
    let current = crate::games::game_config::read_game_config(
        app.clone(),
        args.game_id.clone(),
        args.config_id.clone(),
    )
    .await?;
    let current = safe_document(
        to_value(current).map_err(|_| "PLUGIN_FRONTEND_FAILED")?,
        &auth.manifest,
    )?;
    if current["revision"].as_str() != Some(&args.expected_revision) {
        return Err("CONFIG_CHANGED".into());
    }
    let name = display_name(current["name"].as_str().ok_or("PLUGIN_FRONTEND_FAILED")?);
    let (title, details) = if auth.locale == "zh-CN" {
        (
            "恢复配置",
            format!("将用所选备份替换 {name}。CobbleLauncher 会先保留当前版本。"),
        )
    } else {
        (
            "Restore config",
            format!(
                "Replace {name} with the selected backup. CobbleLauncher will keep the current version first."
            ),
        )
    };
    if !super::plugin_confirm::ask(&app, &auth, title, &details).await? {
        return Err("PLUGIN_ACTION_CANCELLED".into());
    }
    let latest = crate::games::game_config::read_game_config(
        app.clone(),
        args.game_id.clone(),
        args.config_id.clone(),
    )
    .await?;
    if to_value(latest).map_err(|_| "PLUGIN_FRONTEND_FAILED")?["revision"].as_str()
        != Some(&args.expected_revision)
    {
        return Err("CONFIG_CHANGED".into());
    }
    let document = crate::games::game_config::restore_game_config_backup(
        app,
        args.game_id,
        args.config_id,
        args.backup_id,
        args.expected_revision,
    )
    .await?;
    safe_document(
        to_value(document).map_err(|_| "PLUGIN_FRONTEND_FAILED")?,
        &auth.manifest,
    )
}
