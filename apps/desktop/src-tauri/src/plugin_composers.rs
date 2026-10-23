// SPDX-License-Identifier: MIT
//! Pure drafting adapter. Public Mesh snapshots are supplied by the native broker only.
use cobble_plugin_api::{InstalledPlugin, MessageComposerRequest, MessageDraft, PluginManifest};
use serde::Serialize;
use std::sync::{Arc, atomic::AtomicBool};
#[derive(Serialize)]
pub struct ComposerChoice {
    pub plugin_id: String,
    pub name: String,
}
pub(crate) fn selected(plugin_id: &str) -> Result<InstalledPlugin, String> {
    super::valid_id(plugin_id)?;
    let plugin = super::registry()?
        .resolve_enabled(plugin_id)
        .map_err(|error| error.to_string())?;
    if plugin.manifest.api_version != cobble_plugin_api::MESSAGE_COMPOSER_API_VERSION
        || plugin.manifest.capabilities != ["message-composer"]
    {
        return Err("PLUGIN_UNSUPPORTED_API".into());
    }
    Ok(plugin)
}
pub fn choices() -> Result<Vec<ComposerChoice>, String> {
    Ok(cobble_plugin_host::interfaces::ready_plugins(
        &super::registry()?
            .list()
            .map_err(|error| error.to_string())?,
    )
    .into_iter()
    .filter(|plugin| {
        plugin.manifest.api_version == cobble_plugin_api::MESSAGE_COMPOSER_API_VERSION
            && plugin.manifest.capabilities == ["message-composer"]
    })
    .map(|plugin| ComposerChoice {
        plugin_id: plugin.manifest.id.clone(),
        name: plugin.manifest.name.clone(),
    })
    .collect())
}
fn verify(manifest: &PluginManifest) -> Result<(), String> {
    if selected(&manifest.id)?.manifest != *manifest {
        return Err("PLUGIN_CHANGED".into());
    }
    Ok(())
}
/// Caller owns selected recipient authorization and must revalidate it after await.
/// This is deliberately not a renderer command accepting arbitrary public snapshots.
pub(crate) async fn compose(
    plugin_id: String,
    request: MessageComposerRequest,
    cancel: Arc<AtomicBool>,
) -> Result<MessageDraft, String> {
    let plugin = selected(&plugin_id)?;
    request
        .validate(&plugin.manifest)
        .map_err(|error| error.code.clone())?;
    let component = super::plugin_reports::package(&plugin)?;
    let manifest = plugin.manifest;
    tauri::async_runtime::spawn_blocking(move || {
        verify(&manifest)?;
        let host = std::env::current_exe()
            .map_err(|_| "PLUGIN_IO")?
            .with_file_name(if cfg!(windows) {
                "cobble-plugin-host.exe"
            } else {
                "cobble-plugin-host"
            });
        let result = cobble_plugin_host::run_message_composer(
            &host, &component, &manifest, &request, cancel,
        )
        .map_err(|error| error.code.clone())?;
        verify(&manifest)?;
        Ok(result)
    })
    .await
    .map_err(|_| "PLUGIN_IO".to_owned())?
}
