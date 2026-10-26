// SPDX-License-Identifier: MIT
//! Optional, user-started Mesh sessions for verified games or approved loopback servers.
use super::*;
#[path = "mesh_app.rs"]
pub mod app;
#[path = "mesh_app_route.rs"]
mod app_route;
#[path = "mesh_client.rs"]
mod client;
#[path = "mesh_composer.rs"]
pub mod composer;
#[path = "mesh_enabled.rs"]
mod enabled;
#[path = "mesh_environment.rs"]
pub mod environment;
#[path = "mesh_groups.rs"]
pub mod groups;
#[path = "mesh_lan.rs"]
mod lan;
#[path = "mesh_network.rs"]
pub mod network;
#[path = "mesh_node.rs"]
pub mod node;
#[path = "mesh_overview.rs"]
pub mod overview;
#[path = "mesh_port.rs"]
mod port;
use serde_json::{Value, json};
use std::sync::OnceLock;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CompanionSession {
    session_id: u64,
    role: String,
    state: String,
    route: String,
    scope: String,
    expires_at: u64,
    #[serde(default)]
    bridge_address: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct MeshSession {
    session_id: u64,
    game_id: String,
    target: &'static str,
    local_port: Option<u16>,
    role: String,
    state: String,
    route: String,
    scope: String,
    expires_at: u64,
}
#[derive(Serialize)]
pub struct Availability {
    available: bool,
    reason: Option<String>,
    can_host: bool,
}
#[derive(Serialize)]
pub struct HostResult {
    session: MeshSession,
    invitation: String,
}
#[derive(Serialize)]
pub struct JoinResult {
    session: MeshSession,
    bridge_address: String,
}
#[derive(Serialize)]
pub struct StatusResult {
    session: Option<MeshSession>,
}
struct Binding {
    game: Option<sessions::Session>,
    companion_id: u64,
    public: MeshSession,
}
#[derive(Default)]
struct Broker {
    enabled: Option<bool>,
    discovery_checked_at: Option<Instant>,
    client: Option<client::Client>,
    discovery_client: Option<client::Client>,
    social_client: Option<client::Client>,
    social_generation: u64,
    temporary_identity: Option<tempfile::TempDir>,
    binding: Option<Binding>,
    next_id: u64,
    discovery: overview::Discovery,
}
static BROKER: OnceLock<Mutex<Broker>> = OnceLock::new();
struct MeshActivity {
    operation: &'static str,
    started: Instant,
    outcome: &'static str,
}
impl MeshActivity {
    fn new(operation: &'static str) -> Self {
        Self {
            operation,
            started: Instant::now(),
            outcome: "failed",
        }
    }
    fn complete<T>(mut self, result: Result<T, String>) -> Result<T, String> {
        if result.is_ok() {
            self.outcome = "completed";
        }
        result
    }
}
impl Drop for MeshActivity {
    fn drop(&mut self) {
        tracing::info!(
            event = "mesh_operation_finished",
            operation = self.operation,
            outcome = self.outcome,
            elapsed_ms = self.started.elapsed().as_millis() as u64
        );
    }
}
fn broker() -> &'static Mutex<Broker> {
    BROKER.get_or_init(Default::default)
}
/// Private adapter for explicitly approved game handoffs, never a renderer/plugin IPC.
pub(crate) fn file_transfer_request(command: Value) -> Result<Value, String> {
    if !command
        .get("op")
        .and_then(Value::as_str)
        .is_some_and(|op| op.starts_with("app_file_"))
    {
        return Err("GAME_TRANSFER_COMMAND_DENIED".into());
    }
    let mut client = broker().lock().map_err(|_| "Mesh is unavailable")?;
    client.prepare()?;
    client.request(command)
}
/// Closed plugin adapter. It only uses the already-running social client.
pub(crate) fn plugin_event_request(
    expected_generation: Option<u64>,
    command: Value,
) -> Result<(u64, Value), String> {
    if app_route::changing() {
        return Err("MESH_EVENT_ROUTE_CHANGING".into());
    }
    if !plugin_event_command_allowed(expected_generation, &command) {
        return Err("MESH_EVENT_COMMAND_DENIED".into());
    }
    let (generation, client) = {
        let mut state = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let enabled = enabled::require(&mut state).is_ok();
        plugin_event_context(
            expected_generation,
            state.social_generation,
            enabled,
            state.social_client.is_some(),
        )?;
        (
            state.social_generation,
            state.social_client.as_ref().unwrap().clone(),
        )
    };
    // A remote ACK may take time. Offline and identity changes must never wait
    // behind this request's broker lock; old results are checked before return.
    let result = client.request(command);
    let mut state = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
    if app_route::changing() {
        return Err("MESH_EVENT_ROUTE_CHANGING".into());
    }
    let enabled = enabled::require(&mut state).is_ok();
    plugin_event_context(
        Some(generation),
        state.social_generation,
        enabled,
        state.social_client.is_some(),
    )?;
    Ok((generation, result?))
}
fn plugin_event_context(
    expected: Option<u64>,
    current: u64,
    enabled: bool,
    active: bool,
) -> Result<(), String> {
    if !enabled {
        Err("MESH_DISABLED".into())
    } else if expected.is_some_and(|generation| generation != current) {
        Err("MESH_IDENTITY_CHANGED".into())
    } else if !active {
        Err("MESH_APP_STOPPED".into())
    } else {
        Ok(())
    }
}
fn plugin_event_command_allowed(generation: Option<u64>, command: &Value) -> bool {
    let Some(fields) = command.as_object() else {
        return false;
    };
    let keys: &[&str] = match command["op"].as_str() {
        Some("app_status") => &["op"],
        Some("app_event_subscribe") if generation.is_some() => {
            &["op", "subscription_id", "namespace"]
        }
        Some("app_event_unsubscribe") if generation.is_some() => &["op", "subscription_id"],
        Some("app_event_read") if generation.is_some() => {
            &["op", "subscription_id", "after", "limit"]
        }
        Some("app_event_send") if generation.is_some() => {
            &["op", "peer_id", "namespace", "event", "payload"]
        }
        _ => return false,
    };
    fields.len() == keys.len() && keys.iter().all(|key| fields.contains_key(*key))
}
fn helper_path() -> Result<PathBuf, String> {
    crate::runtime_resources::mesh_companion().map_err(|_| "无法确定联机组件位置".into())
}
fn running_game(app: &AppHandle, id: &str) -> Result<sessions::Session, String> {
    let library = app.state::<GameLibrary>();
    let library = library.0.lock().map_err(|_| "游戏状态不可用")?;
    let game = library
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    if game.launch_state != "running" || game.content_installing {
        return Err("请先启动选定的游戏".into());
    }
    sessions::recover(game)?.ok_or_else(|| "无法确认选定的游戏进程".into())
}
fn active(state: &str) -> bool {
    matches!(state, "listening" | "bridge_ready")
}
fn snapshot(value: &Value) -> Result<CompanionSession, String> {
    let session: CompanionSession =
        serde_json::from_value(value.get("session").cloned().ok_or("联机会话数据无效")?)
            .map_err(|_| "联机会话数据无效")?;
    if session.session_id == 0
        || !matches!(session.role.as_str(), "host" | "guest")
        || !matches!(
            session.state.as_str(),
            "listening" | "bridge_ready" | "completed" | "failed" | "ended"
        )
        || !matches!(
            session.route.as_str(),
            "unknown" | "direct" | "relay" | "explicit_relay"
        )
        || session.scope != "public"
    {
        return Err("联机会话数据无效".into());
    }
    Ok(session)
}
impl Broker {
    fn shared_client(&mut self) -> Result<client::Client, String> {
        network::prepare_identity(self);
        enabled::require(self)?;
        let client = self
            .client
            .as_ref()
            .or(self.social_client.as_ref())
            .or(self.discovery_client.as_ref())
            .filter(|client| client.alive())
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| client::Client::spawn(&helper_path()?))?;
        self.client = Some(client.clone());
        self.social_client = Some(client.clone());
        self.discovery_client = Some(client.clone());
        Ok(client)
    }
    fn request(&mut self, command: Value) -> Result<Value, String> {
        let result = self
            .client
            .as_mut()
            .ok_or("联机会话尚未开始")?
            .request(command);
        if result.is_err() {
            self.abort();
        }
        result
    }
    fn prepare(&mut self) -> Result<(), String> {
        enabled::require(self)?;
        if groups::occupied() {
            return Err("MESH_GROUP_ACTIVE".into());
        }
        if self
            .binding
            .as_ref()
            .is_some_and(|binding| active(&binding.public.state))
        {
            return Err("已有联机会话正在进行".into());
        }
        if self.client.is_none() {
            self.shared_client()?;
        }
        Ok(())
    }
    fn bind(
        &mut self,
        game: sessions::Session,
        companion: CompanionSession,
    ) -> Result<MeshSession, String> {
        self.bind_target(Some(game), None, companion)
    }
    fn bind_target(
        &mut self,
        game: Option<sessions::Session>,
        local_port: Option<u16>,
        companion: CompanionSession,
    ) -> Result<MeshSession, String> {
        self.next_id = self.next_id.checked_add(1).ok_or("联机会话序号已用尽")?;
        let public = MeshSession {
            session_id: self.next_id,
            game_id: game
                .as_ref()
                .map(|game| game.game_id.clone())
                .unwrap_or_default(),
            target: if game.is_some() { "game" } else { "server" },
            local_port,
            role: companion.role,
            state: companion.state,
            route: companion.route,
            scope: companion.scope,
            expires_at: companion.expires_at,
        };
        self.binding = Some(Binding {
            game,
            companion_id: companion.session_id,
            public: public.clone(),
        });
        Ok(public)
    }
    fn abort(&mut self) {
        groups::clear();
        if let Some(client) = self.client.take() {
            client.shutdown();
        }
        self.social_client.take();
        self.discovery_client.take();
        self.social_generation = self.social_generation.wrapping_add(1);
        self.binding.take();
    }
    fn end(&mut self, public_id: u64) -> Result<StatusResult, String> {
        let binding = self
            .binding
            .as_ref()
            .filter(|binding| binding.public.session_id == public_id)
            .ok_or("联机会话已变化")?;
        let companion_id = binding.companion_id;
        let result = self.request(json!({"op":"end","session_id":companion_id}))?;
        let state = snapshot(&result)?;
        if state.session_id != companion_id || state.state != "ended" {
            self.abort();
            return Err("联机会话结束确认无效".into());
        }
        let binding = self.binding.as_mut().ok_or("联机会话已结束")?;
        binding.public.state = state.state;
        Ok(StatusResult {
            session: Some(binding.public.clone()),
        })
    }
    fn status(&mut self) -> Result<StatusResult, String> {
        let Some(binding) = &self.binding else {
            return Ok(StatusResult { session: None });
        };
        let id = binding.public.session_id;
        if active(&binding.public.state)
            && binding
                .game
                .as_ref()
                .is_some_and(|game| !sessions::alive(game))
        {
            return self.end(id);
        }
        if !active(&binding.public.state) {
            return Ok(StatusResult {
                session: Some(binding.public.clone()),
            });
        }
        let result = self.request(json!({"op":"status"}))?;
        if result.get("session").is_none_or(Value::is_null) {
            self.binding.take();
            return Ok(StatusResult { session: None });
        }
        let current = snapshot(&result)?;
        let binding = self.binding.as_mut().ok_or("联机会话已结束")?;
        if current.session_id != binding.companion_id {
            return Err("联机会话已变化".into());
        }
        binding.public.state = current.state;
        binding.public.route = current.route;
        Ok(StatusResult {
            session: Some(binding.public.clone()),
        })
    }
}
fn watch_game(public_id: u64) {
    std::thread::spawn(move || {
        let _restore = app_route::RestoreOnExit;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let Ok(mut broker) = broker().lock() else {
                break;
            };
            if !broker.binding.as_ref().is_some_and(|binding| {
                binding.public.session_id == public_id && active(&binding.public.state)
            }) {
                break;
            }
            if broker.status().is_err() {
                break;
            }
        }
    });
}

#[tauri::command]
pub fn mesh_availability() -> Availability {
    let available = helper_path().is_ok_and(|path| client::helper_file(&path));
    Availability {
        available,
        reason: (!available).then(|| "当前安装未包含联机组件".into()),
        can_host: available && cfg!(any(windows, target_os = "linux")),
    }
}
#[tauri::command]
pub async fn mesh_host(
    app: AppHandle,
    game_id: String,
    local_port: Option<u16>,
    relay: Option<String>,
    approve_local_port: bool,
    route_policy: Option<overview::RoutingPolicy>,
) -> Result<HostResult, String> {
    let activity = MeshActivity::new("create");
    if !approve_local_port {
        return Err("请确认共享选定游戏的局域网端口".into());
    }
    let game = running_game(&app, &game_id)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let route_policy = route_policy.unwrap_or_else(|| overview::saved_relay().route_policy);
        let local_port = local_port
            .or_else(|| lan::discover(&game).port)
            .ok_or("Open this world to LAN in Minecraft first")?;
        let relay = relay
            .filter(|s| !s.trim().is_empty())
            .or_else(|| overview::saved_relay().address)
            .ok_or("No relay is configured")?;
        port::approved_listener(local_port, game.pid)?;
        if !sessions::alive(&game) {
            return Err("选定的游戏已退出".into());
        }
        if relay.len() > 1024 || !(relay.starts_with("/ip4/") || relay.starts_with("/dns4/")) {
            return Err("请输入有效的公网中继地址".into());
        }
        let _pause = app_route::pause_for_game()?;
        let mut broker = broker().lock().map_err(|_| "联机状态不可用")?;
        broker.prepare()?;
        let value = broker.request(
            json!({"op":"create_game","approved_loopback_port":local_port,"relay":relay,"route_policy":route_policy}),
        )?;
        let result = (|| {
            let state = snapshot(&value)?;
            if state.role != "host" || state.state != "listening" {
                return Err("联机会话数据无效".into());
            }
            let invitation = value
                .get("invitation")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty() && text.len() <= 6000)
                .ok_or("联机邀请数据无效")?
                .to_owned();
            let session = broker.bind(game, state)?;
            if broker
                .binding
                .as_ref()
                .ok_or("联机会话已结束")?
                .game
                .as_ref()
                .is_some_and(|game| !sessions::alive(game))
            {
                broker.end(session.session_id)?;
                return Err("选定的游戏已退出".into());
            }
            watch_game(session.session_id);
            let _ = overview::save_relay_policy(&relay, &route_policy);
            Ok(HostResult {
                session,
                invitation,
            })
        })();
        if result.is_err() {
            broker.abort();
        }
        result
    })
    .await
    .map_err(|_| "联机操作意外退出")?;
    activity.complete(result)
}
#[tauri::command]
pub async fn mesh_host_server(
    local_port: Option<u16>,
    relay: Option<String>,
    approve_local_port: bool,
    route_policy: Option<overview::RoutingPolicy>,
) -> Result<HostResult, String> {
    let activity = MeshActivity::new("create_server");
    if !approve_local_port {
        return Err("请确认共享本机服务器端口".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let route_policy = route_policy.unwrap_or_else(|| overview::saved_relay().route_policy);
        let local_port = local_port.unwrap_or(25565);
        port::approved_server_listener(local_port)?;
        let relay = relay
            .filter(|value| !value.trim().is_empty())
            .or_else(|| overview::saved_relay().address)
            .filter(|value| overview::valid_relay(value))
            .ok_or("No relay is configured")?;
        let _pause = app_route::pause_for_game()?;
        let mut broker = broker().lock().map_err(|_| "联机状态不可用")?;
        broker.prepare()?;
        let value = broker.request(
            json!({"op":"create_game","approved_loopback_port":local_port,"relay":relay,"route_policy":route_policy}),
        )?;
        let result = (|| {
            let state = snapshot(&value)?;
            if state.role != "host" || state.state != "listening" {
                return Err("联机会话数据无效".into());
            }
            let invitation = value
                .get("invitation")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty() && text.len() <= 6000)
                .ok_or("联机邀请数据无效")?
                .to_owned();
            let session = broker.bind_target(None, Some(local_port), state)?;
            watch_game(session.session_id);
            let _ = overview::save_relay_policy(&relay, &route_policy);
            Ok(HostResult {
                session,
                invitation,
            })
        })();
        if result.is_err() {
            broker.abort();
        }
        result
    })
    .await
    .map_err(|_| "联机操作意外退出")?;
    activity.complete(result)
}

#[derive(Serialize)]
pub struct NetworkStatus {
    relay_available: bool,
    discovery_checked: bool,
    enabled: bool,
    connected: bool,
}
#[tauri::command]
pub async fn mesh_network_status() -> Result<NetworkStatus, String> {
    let snapshot = network::mesh_network_snapshot()?;
    Ok(NetworkStatus {
        enabled: snapshot.enabled,
        connected: snapshot.phase == "online",
        relay_available: snapshot.relay_available,
        discovery_checked: !snapshot.services.is_empty(),
    })
}

#[tauri::command]
pub async fn mesh_join(
    app: AppHandle,
    game_id: String,
    invitation: String,
    route_policy: Option<overview::RoutingPolicy>,
) -> Result<JoinResult, String> {
    let activity = MeshActivity::new("join");
    let game = running_game(&app, &game_id)?;
    if invitation.is_empty() || invitation.len() > 6000 {
        return Err("邀请无效或过长".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _pause = app_route::pause_for_game()?;
        let mut broker = broker().lock().map_err(|_| "联机状态不可用")?;
        broker.prepare()?;
        let route_policy = route_policy.unwrap_or_else(|| overview::saved_relay().route_policy);
        let value = broker.request(
            json!({"op":"join_game","invitation":invitation,"route_policy":route_policy}),
        )?;
        let result = (|| {
            let state = snapshot(&value)?;
            if state.role != "guest" || state.state != "bridge_ready" {
                return Err("联机会话数据无效".into());
            }
            let bridge_address = state
                .bridge_address
                .as_deref()
                .or_else(|| value.get("bridge_address").and_then(Value::as_str))
                .ok_or("联机地址数据无效")?
                .to_owned();
            let address: std::net::SocketAddr =
                bridge_address.parse().map_err(|_| "联机地址数据无效")?;
            if address.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                || address.port() < 1024
            {
                return Err("联机地址数据无效".into());
            }
            let session = broker.bind(game, state)?;
            if broker
                .binding
                .as_ref()
                .ok_or("联机会话已结束")?
                .game
                .as_ref()
                .is_some_and(|game| !sessions::alive(game))
            {
                broker.end(session.session_id)?;
                return Err("选定的游戏已退出".into());
            }
            watch_game(session.session_id);
            Ok(JoinResult {
                session,
                bridge_address,
            })
        })();
        if result.is_err() {
            broker.abort();
        }
        result
    })
    .await
    .map_err(|_| "联机操作意外退出")?;
    activity.complete(result)
}
#[tauri::command]
pub async fn mesh_status() -> Result<StatusResult, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _restore = app_route::RestoreOnExit;
        broker().lock().map_err(|_| "联机状态不可用")?.status()
    })
    .await
    .map_err(|_| "联机状态读取失败")?
}
#[tauri::command]
pub async fn mesh_end(session_id: u64) -> Result<StatusResult, String> {
    let activity = MeshActivity::new("end");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _restore = app_route::RestoreOnExit;
        broker()
            .lock()
            .map_err(|_| "联机状态不可用")?
            .end(session_id)
    })
    .await
    .map_err(|_| "联机结束失败")?;
    activity.complete(result)
}

pub(crate) fn shutdown() {
    groups::clear();
    composer::cancel();
    if let Some(state) = BROKER.get()
        && let Ok(mut broker) = state.try_lock()
    {
        if let Some(client) = &mut broker.client {
            client.shutdown();
        }
        broker.client.take();
        if let Some(client) = &mut broker.discovery_client {
            client.shutdown();
        }
        broker.discovery_client.take();
        if let Some(client) = &mut broker.social_client {
            client.shutdown();
        }
        broker.social_client.take();
        broker.temporary_identity.take();
        broker.binding.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standalone_server_binding_has_no_game_process_requirement() {
        let mut broker = Broker::default();
        let companion = CompanionSession {
            session_id: 1,
            role: "host".into(),
            state: "listening".into(),
            route: "explicit_relay".into(),
            scope: "public".into(),
            expires_at: 123,
            bridge_address: None,
        };
        let session = broker.bind_target(None, Some(25565), companion).unwrap();
        assert_eq!(session.target, "server");
        assert_eq!(session.local_port, Some(25565));
        assert!(session.game_id.is_empty());
        assert!(broker.binding.unwrap().game.is_none());
    }
    #[test]
    fn actual_companion_guest_shape_keeps_validated_bridge_inside_session() {
        let value = json!({"session":{"session_id":1,"role":"guest","state":"bridge_ready","route":"explicit_relay","scope":"public","expires_at":123,"bridge_address":"127.0.0.1:54321"}});
        let session = snapshot(&value).unwrap();
        assert_eq!(session.bridge_address.as_deref(), Some("127.0.0.1:54321"));
    }
    #[test]
    fn companion_metadata_is_allowlisted_and_session_ids_are_native_generations() {
        let valid = json!({"session":{"session_id":1,"role":"host","state":"listening","route":"explicit_relay","scope":"public","expires_at":123}});
        assert!(snapshot(&valid).is_ok());
        let mut invalid = valid;
        invalid["session"]["state"] = json!("arbitrary");
        assert!(snapshot(&invalid).is_err());
        let mut broker = Broker::default();
        let game = sessions::Session {
            game_id: "game".into(),
            directory: PathBuf::new(),
            natives: PathBuf::new(),
            pid: 0,
            started: 0,
            executable: PathBuf::new(),
        };
        let companion = CompanionSession {
            session_id: 1,
            role: "host".into(),
            state: "listening".into(),
            route: "explicit_relay".into(),
            scope: "public".into(),
            expires_at: 123,
            bridge_address: None,
        };
        let first = broker.bind(game.clone(), companion.clone()).unwrap();
        let second = broker.bind(game, companion).unwrap();
        assert_ne!(first.session_id, second.session_id);
        assert!(broker.end(first.session_id).is_err());
        assert_eq!(broker.binding.unwrap().public.session_id, second.session_id);
    }
}

#[tauri::command]
pub async fn mesh_set_enabled(app: AppHandle, enabled: bool) -> Result<NetworkStatus, String> {
    network::mesh_network_set_enabled(app, enabled)?;
    mesh_network_status().await
}

#[cfg(test)]
fn network_readiness(broker: &mut Broker, connected: bool) -> NetworkStatus {
    let enabled = enabled::is_enabled(broker);
    let fresh = broker
        .discovery_checked_at
        .is_some_and(|at| at.elapsed() <= Duration::from_secs(120));
    NetworkStatus {
        enabled,
        connected: enabled && connected,
        discovery_checked: fresh,
        relay_available: enabled
            && fresh
            && broker.discovery.configured_services.iter().any(|service| {
                service.kind == "relay" && service.state == "authenticated" && service.game_relay
            }),
    }
}
#[cfg(test)]
mod readiness_tests {
    use super::*;
    #[test]
    fn plugin_event_adapter_never_allows_startup_or_unbound_writes() {
        assert!(plugin_event_command_allowed(
            None,
            &json!({"op":"app_status"})
        ));
        assert!(!plugin_event_command_allowed(
            None,
            &json!({"op":"app_event_subscribe","subscription_id":"id","namespace":"org.example.ready"})
        ));
        assert!(plugin_event_command_allowed(
            Some(4),
            &json!({"op":"app_event_read","subscription_id":"id","after":0,"limit":4})
        ));
        for command in [
            json!({"op":"app_initialize"}),
            json!({"op":"network_start"}),
            json!({"op":"app_status","extra":true}),
            json!({"op":"app_event_send","peer_id":"peer","namespace":"org.example.ready","event":"ready","payload":null,"address":"raw"}),
        ] {
            assert!(!plugin_event_command_allowed(Some(4), &command));
        }
        assert!(plugin_event_context(Some(4), 4, true, true).is_ok());
        assert_eq!(
            plugin_event_context(Some(4), 5, true, true).unwrap_err(),
            "MESH_IDENTITY_CHANGED"
        );
        assert_eq!(
            plugin_event_context(Some(4), 4, false, true).unwrap_err(),
            "MESH_DISABLED"
        );
        assert_eq!(
            plugin_event_context(Some(4), 4, true, false).unwrap_err(),
            "MESH_APP_STOPPED"
        );
    }
    fn ready() -> Broker {
        let mut broker = Broker {
            enabled: Some(true),
            discovery_checked_at: Some(Instant::now()),
            ..Default::default()
        };
        broker
            .discovery
            .configured_services
            .push(overview::ConfiguredService {
                id: "relay".into(),
                kind: "relay".into(),
                endpoint: "fixture".into(),
                state: "authenticated".into(),
                discovery: "unavailable".into(),
                game_relay: true,
            });
        broker
    }
    #[test]
    fn fresh_relay_probe_is_ready_without_claiming_connected() {
        let status = network_readiness(&mut ready(), false);
        assert!(status.enabled && status.discovery_checked && status.relay_available);
        assert!(!status.connected);
    }
    #[test]
    fn expiry_requires_new_check_without_claiming_connection_failure() {
        let mut broker = ready();
        broker.discovery_checked_at = Some(Instant::now() - Duration::from_secs(121));
        let status = network_readiness(&mut broker, true);
        assert!(!status.discovery_checked);
        assert!(status.connected);
        assert!(!status.relay_available);
    }
    #[test]
    fn bootstrap_ads_and_unauthenticated_or_failed_probes_are_not_ready() {
        let mut broker = ready();
        broker.discovery.configured_services[0].kind = "bootstrap".into();
        assert!(!network_readiness(&mut broker, false).relay_available);
        broker.discovery.configured_services[0].kind = "relay".into();
        broker.discovery.configured_services[0].state = "unavailable".into();
        assert!(!network_readiness(&mut broker, false).relay_available);
        broker.discovery.configured_services.clear();
        broker.discovery.nodes.push(overview::PublishedNode {
            peer_id: "fixture".into(),
            endpoint: "fixture".into(),
            relay: true,
            discovery: true,
            expires_at: u64::MAX,
            evidence: "signed_advertisement".into(),
        });
        assert!(!network_readiness(&mut broker, false).relay_available);
    }
    #[test]
    fn offline_and_stop_clear_readiness() {
        let mut broker = ready();
        broker.enabled = Some(false);
        assert!(!network_readiness(&mut broker, true).connected);
        assert!(!network_readiness(&mut broker, false).relay_available);
        enabled::stop_all(&mut broker);
        let status = network_readiness(&mut broker, false);
        assert!(!status.discovery_checked && !status.relay_available);
        broker.enabled = Some(true);
        assert!(!network_readiness(&mut broker, false).relay_available);
    }
}
