// SPDX-License-Identifier: MIT
//! Renderer supplies only a live viewer resource ID and an optional explicit game.
use super::{plugin_frontend_handoffs::resolve_game, plugin_material_handoffs as materials};
use tauri::{AppHandle, WebviewWindow};
#[tauri::command]
pub async fn blueprint_material_receivers(
    resource_id: String,
) -> Result<Vec<super::plugin_handoffs::Receiver>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let resource = super::resource(&resource_id)?;
        let plugins = super::registry()
            .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
            .list()
            .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?;
        materials::receivers(&resource.viewer_manifest, &plugins)
    })
    .await
    .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
}
#[tauri::command]
pub async fn send_blueprint_materials(
    app: AppHandle,
    window: WebviewWindow,
    resource_id: String,
    target_plugin_id: String,
    game_id: Option<String>,
) -> Result<super::plugin_handoffs::SendResult, String> {
    if window.label() != "main" {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    let game = resolve_game(&app, game_id.as_deref())?;
    tauri::async_runtime::spawn_blocking(move || {
        let resource = super::resource(&resource_id)?;
        let rows = resource
            .session
            .lock()
            .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
            .summary()
            .materials
            .clone();
        let payload = materials::Payload {
            name: resource.name.chars().take(80).collect(),
            source: Some(resource.viewer_manifest.name.chars().take(120).collect()),
            materials: rows
                .into_iter()
                .map(|m| materials::Material {
                    id: m.state,
                    name: None,
                    required: u64::from(m.count),
                    stack_size: None,
                })
                .collect(),
        };
        materials::validate(&payload)?;
        let receiver = super::registry()
            .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
            .resolve_enabled(&target_plugin_id)
            .map_err(|_| "PLUGIN_HANDOFF_RECEIVER_UNAVAILABLE")?;
        let result = materials::send(
            &app,
            &resource.viewer_manifest,
            None,
            game,
            &receiver,
            materials::SendRequest {
                target_plugin_id,
                interface_id: materials::INTERFACE.into(),
                version: materials::VERSION.into(),
                game_id,
                world_id: None,
                payload,
            },
        );
        super::plugin_access_log::record(
            &resource.viewer_manifest.id,
            "plugin-handoff",
            "handoffs.send",
            super::plugin_access_log::Target::None,
            if result.is_ok() {
                super::plugin_access_log::Outcome::Completed
            } else {
                super::plugin_access_log::Outcome::Failed
            },
        );
        result
    })
    .await
    .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
}
