// SPDX-License-Identifier: MIT
use super::*;
#[tauri::command]
pub fn mesh_group_snapshot(game_id: String) -> Result<View, String> {
    Ok(state()
        .lock()
        .map_err(|_| "MESH_GROUP_UNAVAILABLE")?
        .view(Some(&game_id)))
}
#[tauri::command]
pub async fn mesh_group_create(
    app: AppHandle,
    game_id: String,
    local_port: Option<u16>,
    relay: Option<String>,
    approve_local_port: bool,
) -> Result<InvitationResult, String> {
    if !approve_local_port {
        return Err("MESH_GROUP_PORT_APPROVAL_REQUIRED".into());
    }
    let game = running_game(&app, &game_id).map_err(|_| "MESH_GROUP_GAME_EXITED")?;
    tauri::async_runtime::spawn_blocking(move || {
        let port = local_port
            .or_else(|| lan::discover(&game).port)
            .ok_or("MESH_GROUP_LAN_NOT_OPEN")?;
        if port < 1024 {
            return Err("MESH_GROUP_INVALID_PORT".into());
        }
        port::approved_listener(port, game.pid).map_err(|error| match error.as_str() {
            "该端口不属于选定的游戏，请先在游戏中对局域网开放" => {
                "MESH_GROUP_PORT_NOT_OWNED"
            }
            "游戏局域网端口尚未开放" => "MESH_GROUP_LAN_NOT_OPEN",
            _ => "MESH_GROUP_UNAVAILABLE",
        })?;
        let relay = relay
            .filter(|value| !value.trim().is_empty())
            .or_else(|| overview::saved_relay().address)
            .or_else(|| {
                network::mesh_network_snapshot()
                    .ok()
                    .filter(|snapshot| snapshot.enabled)
                    .and_then(|snapshot| authenticated_relay(&snapshot.services))
            })
            .ok_or("MESH_GROUP_RELAY_REQUIRED")?;
        if relay.len() > 1024 {
            return Err("MESH_GROUP_INVALID_RELAY".into());
        }
        let _pause=app_route::pause_for_game()?;
        let (context, directory) = reserve(&app, game)?;
        let result = (|| {
            request(
                &context,
                json!({"op":"app_initialize","directory":directory,"display_name":"Player"}),
            )?;
            let value = request(
                &context,
                json!({"op":"group_create","approved_loopback_port":port,"relay":relay,"route_policy":overview::saved_relay().route_policy}),
            )?;
            let wire = types::decode(&value)?;
            let invitation = types::invitation(&value)?;
            if wire.role != "host" || wire.state != "hosting" {
                return Err("MESH_GROUP_INVALID_RESPONSE".into());
            }
            if !sessions::alive(&context.game) {
                let _ = context
                    .client
                    .request(json!({"op":"group_stop","group_id":wire.group_id}));
                return Err("MESH_GROUP_GAME_EXITED".into());
            }
            let view = finish(&app, &context, wire)?;
            watch(app.clone(), context.clone());
            Ok(InvitationResult { view, invitation })
        })();
        if result.as_ref().is_err_and(|error| {
            error == "MESH_GROUP_INVALID_RESPONSE" || error == "MESH_GROUP_IO_FAILED"
        }) {
            context.client.shutdown();
        }
        if result.is_err() {
            failed(&context);
        }
        result
    })
    .await
    .map_err(|_| "MESH_GROUP_UNAVAILABLE")?
}
#[tauri::command]
pub async fn mesh_group_join(
    app: AppHandle,
    game_id: String,
    invitation: String,
) -> Result<JoinResult, String> {
    if invitation.is_empty() || invitation.len() > 6000 || invitation.chars().any(char::is_control)
    {
        return Err("MESH_GROUP_INVALID_INVITATION".into());
    }
    let game = running_game(&app, &game_id).map_err(|_| "MESH_GROUP_GAME_EXITED")?;
    tauri::async_runtime::spawn_blocking(move || {
        let _pause=app_route::pause_for_game()?;
        let (context, directory) = reserve(&app, game)?;
        let result = (|| {
            request(
                &context,
                json!({"op":"app_initialize","directory":directory,"display_name":"Player"}),
            )?;
            let value = request(&context, json!({"op":"group_join","invitation":invitation,"route_policy":overview::saved_relay().route_policy}))?;
            let mut wire = types::decode(&value)?;
            let bridge = value
                .get("bridge_address")
                .and_then(Value::as_str)
                .or(wire.bridge_address.as_deref())
                .ok_or("MESH_GROUP_INVALID_RESPONSE")?
                .to_owned();
            types::validate_bridge(&bridge)?;
            wire.bridge_address = Some(bridge.clone());
            if wire.role != "guest" || wire.state != "bridge_ready" {
                return Err("MESH_GROUP_INVALID_RESPONSE".into());
            }
            if !sessions::alive(&context.game) {
                let _ = context
                    .client
                    .request(json!({"op":"group_stop","group_id":wire.group_id}));
                return Err("MESH_GROUP_GAME_EXITED".into());
            }
            let view = finish(&app, &context, wire)?;
            watch(app.clone(), context.clone());
            Ok(JoinResult {
                view,
                bridge_address: bridge,
            })
        })();
        if result.as_ref().is_err_and(|error| {
            error == "MESH_GROUP_INVALID_RESPONSE" || error == "MESH_GROUP_IO_FAILED"
        }) {
            context.client.shutdown();
        }
        if result.is_err() {
            failed(&context);
        }
        result
    })
    .await
    .map_err(|_| "MESH_GROUP_UNAVAILABLE")?
}
async fn mutate(
    app: AppHandle,
    game_id: String,
    group_id: u64,
    operation: &'static str,
    peer: Option<String>,
) -> Result<(View, Value), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (context, id) = action(&game_id, group_id, operation != "group_stop")?;
        let mut command = json!({"op":operation,"group_id":id});
        if let Some(peer) = peer {
            command["peer_id"] = json!(peer);
        }
        let result = request(&context, command).and_then(|value| {
            let wire = types::decode(&value)?;
            if (operation == "group_stop" && wire.state != "ended")
                || (operation == "group_revoke" && wire.admission_open)
                || (operation == "group_invite" && !wire.admission_open)
            {
                return Err("MESH_GROUP_INVALID_RESPONSE".into());
            }
            let view = finish(&app, &context, wire)?;
            Ok((view, value))
        });
        if result.is_err() {
            failed(&context);
        }
        result
    })
    .await
    .map_err(|_| "MESH_GROUP_UNAVAILABLE")?
}
#[tauri::command]
pub async fn mesh_group_revoke(
    app: AppHandle,
    game_id: String,
    group_id: u64,
) -> Result<View, String> {
    mutate(app, game_id, group_id, "group_revoke", None)
        .await
        .map(|(view, _)| view)
}
#[tauri::command]
pub async fn mesh_group_invite(
    app: AppHandle,
    game_id: String,
    group_id: u64,
) -> Result<InvitationResult, String> {
    let (view, value) = mutate(app, game_id, group_id, "group_invite", None).await?;
    Ok(InvitationResult {
        view,
        invitation: types::invitation(&value)?,
    })
}
#[tauri::command]
pub async fn mesh_group_kick(
    app: AppHandle,
    game_id: String,
    group_id: u64,
    peer_id: String,
) -> Result<View, String> {
    if peer_id.is_empty()
        || peer_id.len() > 128
        || !peer_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err("MESH_GROUP_UNKNOWN_MEMBER".into());
    }
    mutate(app, game_id, group_id, "group_kick", Some(peer_id))
        .await
        .map(|(view, _)| view)
}
#[tauri::command]
pub async fn mesh_group_stop(
    app: AppHandle,
    game_id: String,
    group_id: u64,
) -> Result<View, String> {
    let result = mutate(app, game_id, group_id, "group_stop", None)
        .await
        .map(|(view, _)| view);
    app_route::schedule();
    result
}
fn authenticated_relay(services: &[overview::ConfiguredService]) -> Option<String> {
    services
        .iter()
        .find(|service| {
            service.kind == "relay"
                && service.state == "authenticated"
                && service.game_relay
                && !service.endpoint.is_empty()
                && service.endpoint.len() <= 1024
        })
        .map(|service| service.endpoint.clone())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_relay_requires_actual_authenticated_game_service() {
        let mut service = overview::ConfiguredService {
            id: "official-relay".into(),
            kind: "relay".into(),
            endpoint: "/dns4/relay.example.invalid/tcp/443/wss/p2p/fixture".into(),
            state: "connecting".into(),
            discovery: "unsupported".into(),
            game_relay: true,
        };
        assert!(authenticated_relay(&[service.clone()]).is_none());
        service.state = "authenticated".into();
        service.game_relay = false;
        assert!(authenticated_relay(&[service.clone()]).is_none());
        service.game_relay = true;
        assert_eq!(
            authenticated_relay(&[service.clone()]),
            Some(service.endpoint.clone())
        );
        service.kind = "discovery".into();
        assert!(authenticated_relay(&[service]).is_none());
    }
}
