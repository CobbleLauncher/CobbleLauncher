// SPDX-License-Identifier: MIT
//! A local root must be explicitly approved outside the plugin package.
use cobble_plugin_api::{PluginError, PluginManifest};
use std::path::{Path, PathBuf};
pub(crate) fn trusted_roots() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("PLUGIN_SIGNATURE_ROOT_STORE")?
        .join("CobbleLauncher/plugin-trust/roots"))
}
pub(super) fn read_package(path: &Path) -> Result<(PluginManifest, Vec<u8>), PluginError> {
    let roots = trusted_roots().map_err(|_| {
        PluginError::new(
            "PLUGIN_SIGNATURE_ROOT_STORE",
            "Trusted plugin roots are unavailable",
        )
    })?;
    cobble_plugin_host::read_package_with_trusted_roots(path, Some(&roots))
}
#[tauri::command]
pub fn get_pending_plugin_signature() -> Result<cobble_plugin_host::SignatureStatus, String> {
    let package = super::PACKAGE
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "PLUGIN_PREVIEW_UNAVAILABLE")?
        .pending
        .clone()
        .ok_or("PLUGIN_PREVIEW_REQUIRED")?;
    let roots = trusted_roots()?;
    let status = cobble_plugin_host::package_signature_status(&package.path, Some(&roots))
        .map_err(|error| error.code)?;
    if status != package.signature {
        return Err("PLUGIN_REVIEW_MISMATCH".into());
    }
    Ok(status)
}
