// SPDX-License-Identifier: MIT
//! Scoped, host-confirmed play-configuration sharing; plugin never sees paths.
use super::plugin_views::ViewAuthorization;
use cobble_plugin_api::PermissionNode;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRequest {
    pub game_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportRequest {
    pub game_id: String,
    pub revision: String,
    pub config_ids: Vec<String>,
}
fn authorize(auth: &ViewAuthorization, game: &str) -> Result<(), String> {
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    if auth.instance_id.as_deref().is_some_and(|id| id != game)
        || game.is_empty()
        || game.len() > 128
    {
        return Err("PLAY_SHARE_SCOPE".into());
    }
    if !auth
        .manifest
        .permissions
        .as_deref()
        .unwrap_or_default()
        .contains(&PermissionNode::PlayManifestExport {})
    {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    Ok(())
}
async fn inspect(
    game: &crate::games::workspace::GameSnapshot,
) -> Result<cobble_core::play_manifest::Preview, String> {
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        cobble_core::play_manifest::preview(
            &game.directory,
            &game.name,
            &game.version,
            game.loader_kind.as_deref(),
            game.loader_version.as_deref(),
        ),
    )
    .await
    .map_err(|_| "PLAY_SHARE_PREVIEW_TIMEOUT")?
    .map_err(|error| match error.to_string().as_str() {
        "PLAY_SHARE_LIMIT" => "PLAY_SHARE_LIMIT".into(),
        "PLAY_SHARE_UNSAFE_FILE" => "PLAY_SHARE_UNSAFE_FILE".into(),
        _ => "PLAY_SHARE_PREVIEW_FAILED".into(),
    })
}
pub async fn preview(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: PreviewRequest,
) -> Result<Value, String> {
    authorize(auth, &args.game_id)?;
    let (game, _lease) = crate::games::workspace::acquire(&app, &args.game_id)?;
    let result = inspect(&game).await?;
    authorize(auth, &args.game_id)?;
    serde_json::to_value(result).map_err(|error| match error.to_string().as_str() {
        "PLAY_SHARE_LIMIT" => "PLAY_SHARE_LIMIT".into(),
        "PLAY_SHARE_UNSAFE_FILE" => "PLAY_SHARE_UNSAFE_FILE".into(),
        _ => "PLAY_SHARE_PREVIEW_FAILED".into(),
    })
}
pub async fn export(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: ExportRequest,
) -> Result<Value, String> {
    authorize(auth, &args.game_id)?;
    if args.revision.len() != 64 || args.config_ids.len() > 64 {
        return Err("PLAY_SHARE_REQUEST_INVALID".into());
    }
    let (game, _lease) = crate::games::workspace::acquire(&app, &args.game_id)?;
    let before = inspect(&game).await?;
    if before.revision != args.revision {
        return Err("PLAY_SHARE_STALE".into());
    }
    let window = app
        .get_webview_window("main")
        .ok_or("PLAY_SHARE_UNAVAILABLE")?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title(if auth.locale == "zh-CN" {
            "保存开玩配置"
        } else {
            "Save play configuration"
        })
        .set_file_name("play-configuration.mrpack")
        .add_filter("Modrinth pack", &["mrpack"])
        .save_file()
        .await
    else {
        return Ok(json!({"saved":false}));
    };
    authorize(auth, &args.game_id)?;
    let after = inspect(&game).await?;
    if after.revision != args.revision {
        return Err("PLAY_SHARE_STALE".into());
    }
    save_archive(
        game,
        auth.cancel.clone(),
        after,
        args.config_ids,
        file.path().to_owned(),
    )
    .await
}
async fn save_archive(
    game: crate::games::workspace::GameSnapshot,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    after: cobble_core::play_manifest::Preview,
    config_ids: Vec<String>,
    destination: std::path::PathBuf,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let parent = destination.parent().ok_or("PLAY_SHARE_UNAVAILABLE")?;
        let temporary = tempfile::Builder::new()
            .prefix(".cobble-play-share-")
            .tempdir_in(parent)
            .map_err(|_| "PLAY_SHARE_SAVE_FAILED")?;
        let archive = temporary.path().join("share.mrpack");
        if cancel.load(Ordering::Acquire) {
            return Err("PLUGIN_VIEW_REVOKED".into());
        }
        cobble_core::play_manifest::write(&game.directory, &archive, &after, &config_ids)
            .map_err(|_| "PLAY_SHARE_SAVE_FAILED")?;
        if cancel.load(Ordering::Acquire) {
            return Err("PLUGIN_VIEW_REVOKED".into());
        }
        let mut output =
            tempfile::NamedTempFile::new_in(parent).map_err(|_| "PLAY_SHARE_SAVE_FAILED")?;
        let mut input = std::fs::File::open(&archive).map_err(|_| "PLAY_SHARE_SAVE_FAILED")?;
        std::io::copy(&mut input, output.as_file_mut()).map_err(|_| "PLAY_SHARE_SAVE_FAILED")?;
        output
            .as_file()
            .sync_all()
            .map_err(|_| "PLAY_SHARE_SAVE_FAILED")?;
        output
            .persist(destination)
            .map_err(|_| "PLAY_SHARE_SAVE_FAILED")?;
        Ok(json!({"saved":true,"mods":after.mods.len(),"configs":config_ids.len()}))
    })
    .await
    .map_err(|_| "PLAY_SHARE_SAVE_FAILED")?
}
/// Internal debug probe only: no IPC and no native-picker acceptance claim.
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub async fn isolated_export(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: ExportRequest,
    destination: std::path::PathBuf,
) -> Result<Value, String> {
    authorize(auth, &args.game_id)?;
    let parent = destination.parent().ok_or("PLAY_SHARE_PROBE_SCOPE")?;
    let parent = std::fs::canonicalize(parent).map_err(|_| "PLAY_SHARE_PROBE_SCOPE")?;
    let temporary =
        std::fs::canonicalize(std::env::temp_dir()).map_err(|_| "PLAY_SHARE_PROBE_SCOPE")?;
    let relative = parent
        .strip_prefix(&temporary)
        .map_err(|_| "PLAY_SHARE_PROBE_SCOPE")?;
    if relative.components().count() != 1
        || !relative
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| {
                n.starts_with("cobble-plugin-webview-probe-")
                    && n.len() > "cobble-plugin-webview-probe-".len()
            })
        || destination.exists()
    {
        return Err("PLAY_SHARE_PROBE_SCOPE".into());
    }
    let filename = destination.file_name().ok_or("PLAY_SHARE_PROBE_SCOPE")?;
    let destination = parent.join(filename);
    if args.revision.len() != 64 || args.config_ids.len() > 64 {
        return Err("PLAY_SHARE_REQUEST_INVALID".into());
    }
    let (game, _lease) = crate::games::workspace::acquire(&app, &args.game_id)?;
    let source = std::fs::canonicalize(&game.directory).map_err(|_| "PLAY_SHARE_PROBE_SCOPE")?;
    if !source.starts_with(&parent) {
        return Err("PLAY_SHARE_PROBE_SCOPE".into());
    }
    let after = inspect(&game).await?;
    if after.revision != args.revision {
        return Err("PLAY_SHARE_STALE".into());
    }
    authorize(auth, &args.game_id)?;
    save_archive(
        game,
        auth.cancel.clone(),
        after,
        args.config_ids,
        destination,
    )
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_requests_reject_paths() {
        assert!(
            serde_json::from_value::<PreviewRequest>(json!({"gameId":"g","path":"C:/secret"}))
                .is_err()
        );
        assert!(
            serde_json::from_value::<ExportRequest>(
                json!({"gameId":"g","revision":"r","configIds":[],"destination":"C:/secret"})
            )
            .is_err()
        );
    }
    #[test]
    fn permission_binding_and_revocation_are_enforced() {
        let mut auth = ViewAuthorization {
            manifest: serde_json::from_str(include_str!(
                "../../../../examples/play-manifest/plugin.json"
            ))
            .unwrap(),
            instance_id: Some("owned-game".into()),
            keybinding_target_id: None,
            handoff_id: None,
            config_restore_target: None,
            locale: "en".into(),
            theme: "light".into(),
            appearance: None,
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            session_id: "synthetic-share-test".into(),
        };
        assert!(authorize(&auth, "owned-game").is_ok());
        assert_eq!(
            authorize(&auth, "other-game").unwrap_err(),
            "PLAY_SHARE_SCOPE"
        );
        auth.manifest.permissions = Some(vec![]);
        assert_eq!(
            authorize(&auth, "owned-game").unwrap_err(),
            "PLUGIN_PERMISSION_DENIED"
        );
        auth.cancel.store(true, Ordering::Release);
        assert_eq!(
            authorize(&auth, "owned-game").unwrap_err(),
            "PLUGIN_VIEW_REVOKED"
        );
    }
}
