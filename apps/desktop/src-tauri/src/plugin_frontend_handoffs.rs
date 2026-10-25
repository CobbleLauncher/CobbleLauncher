// SPDX-License-Identifier: MIT
//! Registry/context resolution for the concrete waypoint handoff.
use super::{
    plugin_handoffs as handoffs, plugin_material_handoffs as materials,
    plugin_views::ViewAuthorization,
};
use cobble_plugin_api::{InspectionScope, PermissionNode};
use serde::Deserialize;
use serde_json::Value;
use std::sync::atomic::Ordering;
use tauri::AppHandle;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterfaceRequest {
    pub interface_id: String,
    pub version: String,
}
impl InterfaceRequest {
    fn validate(&self) -> Result<(), String> {
        if (self.interface_id == handoffs::WAYPOINT_INTERFACE
            || self.interface_id == materials::INTERFACE)
            && self.version == handoffs::WAYPOINT_VERSION
        {
            Ok(())
        } else {
            Err("PLUGIN_HANDOFF_INTERFACE_UNSUPPORTED".into())
        }
    }
}
pub async fn receivers(auth: &ViewAuthorization, args: InterfaceRequest) -> Result<Value, String> {
    args.validate()?;
    let manifest = auth.manifest.clone();
    let cancelled = auth.cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if cancelled.load(Ordering::Acquire) {
            return Err("PLUGIN_VIEW_REVOKED".into());
        }
        let plugins = super::registry()
            .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
            .list()
            .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?;
        serde_json::to_value(if args.interface_id == materials::INTERFACE {
            materials::receivers(&manifest, &plugins)?
        } else {
            handoffs::receivers(&manifest, &plugins)?
        })
        .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE".into())
    })
    .await
    .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
}
async fn send_waypoint(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: handoffs::SendRequest,
) -> Result<Value, String> {
    if auth
        .instance_id
        .as_deref()
        .is_some_and(|id| id != args.game_id)
    {
        return Err("PLUGIN_GAME_SCOPE".into());
    }
    crate::games::ensure_registered_game(&app, &args.game_id).map_err(|_| "PLUGIN_GAME_SCOPE")?;
    let game = crate::games::frontend_game_summaries(&app)?
        .into_iter()
        .find(|game| game["id"].as_str() == Some(args.game_id.as_str()))
        .ok_or("PLUGIN_GAME_SCOPE")?;
    if let Some(world) = &args.world_id {
        if !auth.manifest.permissions.as_deref().unwrap_or_default().iter().any(|node| matches!(node, PermissionNode::GameInspectionRead { scopes } if scopes.contains(&InspectionScope::Worlds))) { return Err("PLUGIN_PERMISSION_DENIED".into()); }
        let worlds = crate::games::worlds::list_game_worlds(app.clone(), args.game_id.clone())
            .await
            .map_err(|_| "PLUGIN_WORLD_SCOPE")?;
        if !worlds.iter().any(|item| &item.folder_name == world) {
            return Err("PLUGIN_WORLD_SCOPE".into());
        }
    }
    let auth = auth.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if auth.cancel.load(Ordering::Acquire) {
            return Err("PLUGIN_VIEW_REVOKED".into());
        }
        let receiver = super::registry()
            .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
            .resolve_enabled(&args.target_plugin_id)
            .map_err(|_| "PLUGIN_HANDOFF_RECEIVER_UNAVAILABLE")?;
        let sender_game = handoffs::GameIdentity {
            id: args.game_id.clone(),
            name: game["name"].as_str().ok_or("PLUGIN_GAME_SCOPE")?.into(),
        };
        serde_json::to_value(handoffs::send(
            &app,
            &auth.manifest,
            auth.instance_id.as_deref(),
            sender_game,
            &receiver,
            args,
        )?)
        .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE".into())
    })
    .await
    .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
}
pub fn take(auth: &ViewAuthorization, args: InterfaceRequest) -> Result<Value, String> {
    args.validate()?;
    if args.interface_id == materials::INTERFACE {
        return serde_json::to_value(materials::take(
            &auth.manifest,
            auth.instance_id.as_deref(),
            auth.handoff_id.as_deref(),
        )?)
        .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE".into());
    }
    serde_json::to_value(handoffs::take(
        &auth.manifest,
        auth.instance_id.as_deref(),
        auth.handoff_id.as_deref(),
        &args.interface_id,
        &args.version,
    )?)
    .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE".into())
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum SendRequest {
    Waypoint(handoffs::SendRequest),
    Materials(materials::SendRequest),
}
impl SendRequest {
    pub fn game_id(&self) -> Option<&str> {
        match self {
            Self::Waypoint(r) => Some(&r.game_id),
            Self::Materials(r) => r.game_id.as_deref(),
        }
    }
}
pub async fn send(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: SendRequest,
) -> Result<Value, String> {
    let SendRequest::Materials(args) = args else {
        let SendRequest::Waypoint(args) = args else {
            unreachable!()
        };
        return send_waypoint(app, auth, args).await;
    };
    if auth
        .instance_id
        .as_deref()
        .is_some_and(|id| Some(id) != args.game_id.as_deref())
    {
        return Err("PLUGIN_GAME_SCOPE".into());
    }
    let game = resolve_game(&app, args.game_id.as_deref())?;
    if let Some(world) = &args.world_id {
        let id = args.game_id.as_ref().ok_or("PLUGIN_WORLD_SCOPE")?;
        if !auth.manifest.permissions.as_deref().unwrap_or_default().iter().any(|node|matches!(node,PermissionNode::GameInspectionRead{scopes} if scopes.contains(&InspectionScope::Worlds))){return Err("PLUGIN_PERMISSION_DENIED".into())}
        let worlds = crate::games::worlds::list_game_worlds(app.clone(), id.clone())
            .await
            .map_err(|_| "PLUGIN_WORLD_SCOPE")?;
        if !worlds.iter().any(|w| &w.folder_name == world) {
            return Err("PLUGIN_WORLD_SCOPE".into());
        }
    }
    let auth = auth.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if auth.cancel.load(Ordering::Acquire) {
            return Err("PLUGIN_VIEW_REVOKED".into());
        }
        let receiver = super::registry()
            .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
            .resolve_enabled(&args.target_plugin_id)
            .map_err(|_| "PLUGIN_HANDOFF_RECEIVER_UNAVAILABLE")?;
        serde_json::to_value(materials::send(
            &app,
            &auth.manifest,
            auth.instance_id.as_deref(),
            game,
            &receiver,
            args,
        )?)
        .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE".into())
    })
    .await
    .map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?
}
pub(super) fn resolve_game(
    app: &AppHandle,
    id: Option<&str>,
) -> Result<Option<handoffs::GameIdentity>, String> {
    let Some(id) = id else { return Ok(None) };
    crate::games::ensure_registered_game(app, id).map_err(|_| "PLUGIN_GAME_SCOPE")?;
    let game = crate::games::frontend_game_summaries(app)?
        .into_iter()
        .find(|g| g["id"].as_str() == Some(id))
        .ok_or("PLUGIN_GAME_SCOPE")?;
    Ok(Some(handoffs::GameIdentity {
        id: id.into(),
        name: game["name"].as_str().ok_or("PLUGIN_GAME_SCOPE")?.into(),
    }))
}
