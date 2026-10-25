// SPDX-License-Identifier: MIT
//! Host-owned confirmation for native workspace writes, never a plugin-supplied approval flag.
use super::plugin_views::ViewAuthorization;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager};
#[cfg(debug_assertions)]
static PROBE_APPROVAL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub fn enable_isolated_probe() -> Result<(), String> {
    let root = super::root("plugins")?
        .canonicalize()
        .map_err(|_| "PROBE_REGISTRY_INVALID")?;
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| "PROBE_REGISTRY_INVALID")?;
    if !root.starts_with(temp)
        || !root.ancestors().any(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with("cobble-plugin-webview-probe-"))
        })
    {
        return Err("PROBE_REGISTRY_INVALID".into());
    }
    PROBE_APPROVAL.store(true, Ordering::Release);
    Ok(())
}
pub async fn ask(
    app: &AppHandle,
    auth: &ViewAuthorization,
    title: &str,
    details: &str,
) -> Result<bool, String> {
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    #[cfg(debug_assertions)]
    if PROBE_APPROVAL.load(Ordering::Acquire) {
        return Ok(true);
    }
    let window = app
        .get_webview_window("main")
        .ok_or("PLUGIN_CONFIRM_UNAVAILABLE")?;
    let accepted = rfd::AsyncMessageDialog::new()
        .set_parent(&window)
        .set_title(title)
        .set_description(details)
        .set_level(rfd::MessageLevel::Warning)
        .set_buttons(rfd::MessageButtons::OkCancel)
        .show()
        .await
        == rfd::MessageDialogResult::Ok;
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    Ok(accepted)
}
