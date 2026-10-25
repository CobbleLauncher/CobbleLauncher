// SPDX-License-Identifier: MIT
//! Selected Inbox resource to an existing Report worker; no new parser or file capability.
use super::*;
pub fn supports(manifest: &PluginManifest, name: &str) -> bool {
    let ext = name
        .rsplit_once('.')
        .map(|(_, v)| v.to_ascii_lowercase())
        .unwrap_or_default();
    manifest.capabilities==["report"]&&manifest.file_extensions.iter().any(|e|e==&ext)
 && manifest.permissions.as_deref().unwrap_or_default().iter().any(|p|matches!(p,cobble_plugin_api::PermissionNode::SelectedFileRead{extensions}if extensions.contains(&ext)))
 && (report_media_type(&ext).is_some()||(ext=="zip"&&pack_metadata_protocol(manifest)))
}
pub async fn analyze(
    app: AppHandle,
    path: std::path::PathBuf,
    plugin_id: String,
    locale: String,
    game: Option<String>,
) -> Result<serde_json::Value, String> {
    let (plugin, component, bytes, media) = tauri::async_runtime::spawn_blocking(move || {
        let plugin = selected_plugin(&plugin_id).map_err(|_| "FILE_INBOX_HANDLER_UNAVAILABLE")?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("FILE_INBOX_FORMAT_INVALID")?;
        if !supports(&plugin.manifest, name) {
            return Err("FILE_INBOX_HANDLER_UNAVAILABLE".into());
        }
        let (bytes, media) =
            selected_snapshot(&path, &plugin.manifest).map_err(|_| "FILE_INBOX_REPORT_FAILED")?;
        let component = package(&plugin)?;
        Ok::<_, String>((plugin, component, bytes, media))
    })
    .await
    .map_err(|_| "FILE_INBOX_REPORT_FAILED")??;
    if let Some(id) = &game {
        crate::games::ensure_registered_game(&app, id).map_err(|_| "PLUGIN_GAME_SCOPE")?;
    }
    let report = execute_report(
        plugin,
        component,
        bytes,
        media,
        Arc::new(AtomicBool::new(false)),
        locale,
        game,
    )
    .await
    .map_err(|_| "FILE_INBOX_REPORT_FAILED")?;
    serde_json::to_value(report.ok_or("PLUGIN_ACTION_CANCELLED")?)
        .map_err(|_| "FILE_INBOX_REPORT_FAILED".into())
}
