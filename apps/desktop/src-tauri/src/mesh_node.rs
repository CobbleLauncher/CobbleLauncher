// SPDX-License-Identifier: MIT
//! Explicit local volunteer/private relay process owned by the existing companion.
use super::*;

fn node_request(broker: &mut Broker, command: Value, initialize: bool) -> Result<Value, String> {
    enabled::require(broker)?;
    let client = broker.shared_client()?;
    if initialize {
        let directory = broker
            .temporary_identity
            .as_ref()
            .map(|root| root.path().join("identity"))
            .map(Ok)
            .unwrap_or_else(app::private_directory)?;
        client.request(
            json!({"op":"app_initialize","directory":directory,"display_name":"Player"}),
        )?;
    }
    client.request(command)
}
#[tauri::command]
pub async fn mesh_node_start(config: Value) -> Result<Value, String> {
    if serde_json::to_vec(&config)
        .map_err(|_| "MESH_INVALID_NODE_CONFIG")?
        .len()
        > 6000
    {
        return Err("MESH_INVALID_NODE_CONFIG".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut state = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        node_request(&mut state, json!({"op":"node_start","config":config}), true)
    })
    .await
    .map_err(|_| "MESH_STATE_UNAVAILABLE")?
}
#[tauri::command]
pub async fn mesh_node_status() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        if !enabled::is_enabled(&mut broker) || broker.client.is_none() {
            return Ok(json!({"node":null}));
        }
        let Some(client) = broker
            .client
            .as_ref()
            .filter(|client| client.alive())
            .cloned()
        else {
            return Ok(json!({"node":null}));
        };
        client.request(json!({"op":"node_status"}))
    })
    .await
    .map_err(|_| "MESH_STATE_UNAVAILABLE")?
}
#[tauri::command]
pub async fn mesh_node_stop() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let Some(client) = state
            .client
            .as_ref()
            .filter(|client| client.alive())
            .cloned()
        else {
            return Ok(json!({"node":null}));
        };
        client.request(json!({"op":"node_stop"}))
    })
    .await
    .map_err(|_| "MESH_STATE_UNAVAILABLE")?
}
