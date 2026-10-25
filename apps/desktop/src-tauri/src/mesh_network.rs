// SPDX-License-Identifier: MIT
//! Cached native service state. Companion owns cancellable network retry tasks.
use super::*;
#[derive(Clone, Deserialize, Serialize)]
pub struct MeshNetworkSnapshot {
    pub enabled: bool,
    pub phase: String,
    pub generation: u64,
    pub attempt: u32,
    pub retry_at: Option<u64>,
    pub profile: Option<app::PublicProfile>,
    pub services: Vec<overview::ConfiguredService>,
    pub last_error: Option<String>,
    pub relay_available: bool,
    pub session_route: Option<String>,
    #[serde(default)]
    pub checks: Vec<NetworkCheck>,
    #[serde(default)]
    pub session_evidence: Option<SessionEvidence>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct SessionEvidence {
    peer_scope: String,
    selected_route: String,
    selected_connection: Option<String>,
    bytes_sent: u64,
    bytes_received: u64,
    completed_streams: u64,
    active_streams: u32,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct NetworkCheck {
    capability: String,
    supported: bool,
    state: String,
    observed_at: Option<u64>,
    stage: String,
    peer_scope: String,
    error_code: Option<String>,
}
impl Default for MeshNetworkSnapshot {
    fn default() -> Self {
        let enabled = enabled::current();
        Self {
            enabled,
            phase: if enabled { "connecting" } else { "offline" }.into(),
            generation: 0,
            attempt: 0,
            retry_at: None,
            profile: None,
            services: vec![],
            last_error: None,
            relay_available: false,
            session_route: None,
            checks: vec![],
            session_evidence: None,
        }
    }
}
#[derive(Default)]
struct State {
    snapshot: MeshNetworkSnapshot,
    companion_generation: u64,
    event_generation: Option<u64>,
    client: Option<client::Client>,
    started: bool,
}
impl State {
    fn should_start(&self, retry: bool) -> bool {
        self.snapshot.enabled
            && (retry
                || !self.started
                || !matches!(
                    self.snapshot.phase.as_str(),
                    "connecting" | "online" | "reconnecting"
                ))
    }
    fn response_is_current(&self, generation: u64) -> bool {
        self.event_generation
            .is_none_or(|event_generation| event_generation < generation)
    }
    fn accepts(&self, epoch: u64, generation: u64) -> bool {
        self.snapshot.enabled
            && self.snapshot.generation == epoch
            && generation >= self.companion_generation
    }
}
static NETWORK: OnceLock<Mutex<State>> = OnceLock::new();
static RESET_IDENTITY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) fn prepare_identity(broker: &mut Broker) {
    if RESET_IDENTITY.swap(false, std::sync::atomic::Ordering::SeqCst) {
        enabled::stop_all(broker);
    }
}
fn state() -> &'static Mutex<State> {
    NETWORK.get_or_init(Default::default)
}
fn publish(app: &AppHandle, snapshot: &MeshNetworkSnapshot) {
    let _ = app.emit("mesh-network-changed", snapshot);
}
pub(super) fn invalidate(app: &AppHandle) -> Result<(), String> {
    groups::clear();
    let mut state = state().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
    state.snapshot.generation = state.snapshot.generation.wrapping_add(1);
    state.snapshot.phase = if state.snapshot.enabled {
        "connecting"
    } else {
        "offline"
    }
    .into();
    state.started = false;
    state.snapshot.relay_available = false;
    state.snapshot.profile = None;
    state.snapshot.services.clear();
    state.snapshot.session_route = None;
    state.snapshot.checks.clear();
    state.snapshot.session_evidence = None;
    state.snapshot.retry_at = None;
    let snapshot = state.snapshot.clone();
    drop(state);
    publish(app, &snapshot);
    Ok(())
}
fn accept(app: &AppHandle, epoch: u64, mut value: Value, from_event: bool) -> Result<(), String> {
    let companion_generation = value["generation"]
        .as_u64()
        .ok_or("MESH_INVALID_RESPONSE")?;
    value["enabled"] = json!(true);
    let mut snapshot: MeshNetworkSnapshot =
        serde_json::from_value(value).map_err(|_| "MESH_INVALID_RESPONSE")?;
    if !matches!(
        snapshot.phase.as_str(),
        "offline" | "connecting" | "online" | "reconnecting" | "failed"
    ) || snapshot.services.len() > 8
        || snapshot.checks.len() > 3
        || snapshot.checks.iter().any(|check| {
            !matches!(check.capability.as_str(), "relay" | "direct" | "hole_punch")
                || !matches!(
                    check.state.as_str(),
                    "not_tested" | "testing" | "verified" | "failed" | "unsupported"
                )
                || check.stage.len() > 128
                || check.peer_scope.len() > 256
                || check
                    .error_code
                    .as_ref()
                    .is_some_and(|code| code.len() > 128)
        })
    {
        return Err("MESH_INVALID_RESPONSE".into());
    }
    let mut state = state().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
    if !state.accepts(epoch, companion_generation) {
        return Ok(());
    }
    if !from_event && !state.response_is_current(companion_generation) {
        return Ok(());
    }
    if from_event {
        state.event_generation = Some(companion_generation);
    }
    state.companion_generation = companion_generation;
    snapshot.generation = epoch;
    state.snapshot = snapshot.clone();
    drop(state);
    publish(app, &snapshot);
    if snapshot.phase == "online" {
        app_route::schedule();
    }
    Ok(())
}
#[tauri::command]
pub fn mesh_network_snapshot() -> Result<MeshNetworkSnapshot, String> {
    Ok(state()
        .lock()
        .map_err(|_| "MESH_STATE_UNAVAILABLE")?
        .snapshot
        .clone())
}
fn start(app: AppHandle, retry: bool) -> Result<MeshNetworkSnapshot, String> {
    let snapshot = {
        let mut state = state().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        if !state.should_start(retry) {
            return Ok(state.snapshot.clone());
        }
        state.started = true;
        state.snapshot.generation = state.snapshot.generation.wrapping_add(1);
        state.snapshot.phase = "connecting".into();
        state.snapshot.last_error = None;
        state.snapshot.relay_available = false;
        state.snapshot.session_route = None;
        state.snapshot.session_evidence = None;
        state.companion_generation = 0;
        state.event_generation = None;
        state.snapshot.clone()
    };
    publish(&app, &snapshot);
    let epoch = snapshot.generation;
    std::thread::spawn(move || {
        let result = (|| {
            let (client, directory, existing) = {
                let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
                prepare_identity(&mut broker);
                enabled::require(&mut broker)?;
                let directory = broker
                    .temporary_identity
                    .as_ref()
                    .map(|root| root.path().join("identity"))
                    .map(Ok)
                    .unwrap_or_else(app::private_directory)?;
                let existing = broker
                    .client
                    .as_ref()
                    .or(broker.social_client.as_ref())
                    .is_some_and(|client| client.alive());
                (broker.shared_client()?, directory, existing)
            };
            {
                let mut state = state().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
                if !state.snapshot.enabled || state.snapshot.generation != epoch {
                    return Ok(());
                }
                state.client = Some(client.clone());
            }
            let event_app = app.clone();
            client.on_event(Arc::new(move |frame| {
                if frame["event"] == "network_state_changed" {
                    if frame["generation"] == frame["snapshot"]["generation"] {
                        let _ = accept(&event_app, epoch, frame["snapshot"].clone(), true);
                    }
                } else if frame["event"] == "companion_exited"
                    && let Ok(mut state) = state().lock()
                    && state.snapshot.enabled
                    && state.snapshot.generation == epoch
                {
                    state.snapshot.phase = "failed".into();
                    state.snapshot.relay_available = false;
                    state.snapshot.session_route = None;
                    state.snapshot.session_evidence = None;
                    state.snapshot.checks.clear();
                    state.snapshot.last_error = Some("MESH_COMPONENT_EXITED".into());
                    let snapshot = state.snapshot.clone();
                    drop(state);
                    publish(&event_app, &snapshot);
                    reconnect_after_exit(event_app.clone(), epoch);
                }
            }));
            // No broker mutex is held during companion I/O or connection setup.
            let value = if retry && existing {
                client.request(json!({"op":"network_retry"}))?
            } else {
                client.request(json!({"op":"network_start","directory":directory}))?
            };
            accept(&app, epoch, value, false)
        })();
        if let Err(error) = result
            && let Ok(mut state) = state().lock()
            && state.snapshot.enabled
            && state.snapshot.generation == epoch
        {
            state.snapshot.phase = "failed".into();
            state.snapshot.last_error = Some(error);
            let snapshot = state.snapshot.clone();
            drop(state);
            publish(&app, &snapshot);
        }
    });
    Ok(snapshot)
}
fn reconnect_after_exit(app: AppHandle, epoch: u64) {
    std::thread::spawn(move || {
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(100));
            if !enabled::current()
                || state()
                    .lock()
                    .map(|state| !state.accepts(epoch, u64::MAX))
                    .unwrap_or(true)
            {
                return;
            }
        }
        let _ = start(app, true);
    });
}
#[tauri::command]
pub fn mesh_network_start(app: AppHandle) -> Result<MeshNetworkSnapshot, String> {
    start(app, false)
}
#[tauri::command]
pub fn mesh_network_retry(app: AppHandle) -> Result<MeshNetworkSnapshot, String> {
    start(app, true)
}
/// Connectivity/window resume hints never start offline Mesh or restart a healthy service.
#[tauri::command]
pub fn mesh_network_resume_if_needed(app: AppHandle) -> Result<MeshNetworkSnapshot, String> {
    let snapshot = mesh_network_snapshot()?;
    if snapshot.enabled && matches!(snapshot.phase.as_str(), "failed" | "reconnecting") {
        start(app, true)
    } else {
        Ok(snapshot)
    }
}
#[tauri::command]
pub fn mesh_network_set_enabled(
    app: AppHandle,
    enabled: bool,
) -> Result<MeshNetworkSnapshot, String> {
    super::enabled::save(enabled)?;
    if !enabled {
        groups::clear();
        RESET_IDENTITY.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    let (snapshot, client) = {
        let mut state = state().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let generation = state.snapshot.generation.wrapping_add(1);
        state.snapshot = MeshNetworkSnapshot {
            enabled,
            generation,
            ..Default::default()
        };
        state.started = false;
        let client = if !enabled { state.client.take() } else { None };
        (state.snapshot.clone(), client)
    };
    publish(&app, &snapshot);
    // Persisted gate and event generation flip before any potentially slow shutdown.
    let stop_app = app.clone();
    let epoch = snapshot.generation;
    std::thread::spawn(move || {
        if state()
            .lock()
            .map(|state| state.snapshot.generation != epoch)
            .unwrap_or(true)
        {
            return;
        }
        if let Some(client) = client {
            client.shutdown();
        }
        if let Ok(mut broker) = broker().lock() {
            if state()
                .lock()
                .map(|state| state.snapshot.generation != epoch)
                .unwrap_or(true)
            {
                return;
            }
            broker.enabled = Some(enabled);
            if !enabled {
                super::enabled::stop_all(&mut broker);
                RESET_IDENTITY.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
        if enabled {
            let _ = start(stop_app, false);
        }
    });
    Ok(snapshot)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_and_new_native_generation_reject_late_companion_events() {
        let mut state = State {
            snapshot: MeshNetworkSnapshot {
                enabled: true,
                generation: 7,
                ..Default::default()
            },
            companion_generation: 3,
            event_generation: None,
            client: None,
            started: true,
        };
        assert!(state.accepts(7, 3));
        assert!(!state.accepts(7, 2));
        state.snapshot.enabled = false;
        assert!(!state.accepts(7, 100));
        state.snapshot.enabled = true;
        state.snapshot.generation = 8;
        assert!(!state.accepts(7, 100));
        assert!(state.accepts(8, 4));
    }
    #[test]
    fn late_start_response_does_not_replace_newer_online_event_in_same_generation() {
        let state = State {
            event_generation: Some(4),
            ..Default::default()
        };
        assert!(!state.response_is_current(4));
        assert!(!state.response_is_current(3));
        assert!(state.response_is_current(5));
    }
    #[test]
    fn enabled_initial_connecting_runs_once_but_offline_never_starts() {
        let mut state = State {
            snapshot: MeshNetworkSnapshot {
                enabled: true,
                phase: "connecting".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(state.should_start(false));
        state.started = true;
        assert!(!state.should_start(false));
        state.snapshot.enabled = false;
        assert!(!state.should_start(false));
        assert!(!state.should_start(true));
        state.snapshot.enabled = true;
        state.started = false;
        assert!(state.should_start(false));
    }
    #[test]
    fn pending_offline_cleanup_discards_temporary_identity_before_next_service() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().to_owned();
        let mut broker = Broker {
            enabled: Some(true),
            temporary_identity: Some(root),
            ..Default::default()
        };
        RESET_IDENTITY.store(true, std::sync::atomic::Ordering::SeqCst);
        prepare_identity(&mut broker);
        assert!(broker.temporary_identity.is_none());
        assert!(!path.exists());
        assert!(!RESET_IDENTITY.load(std::sync::atomic::Ordering::SeqCst));
    }
}
