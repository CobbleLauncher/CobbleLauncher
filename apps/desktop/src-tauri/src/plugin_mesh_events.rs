// SPDX-License-Identifier: MIT
//! View-owned event subscriptions on an already-running Mesh contact service.
//! Namespace consent does not authenticate the remote plugin or its publisher.
use crate::plugins::plugin_views::ViewAuthorization;
use cobble_plugin_api::{
    PermissionNode, PluginManifest, valid_mesh_event, valid_mesh_namespace, validate_mesh_payload,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

const LEASE: Duration = Duration::from_secs(120);
const MAX_HANDLES: usize = 32;
const MAX_VIEW_HANDLES: usize = 8;
const MAX_SAFE: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Subscribe {
    pub namespace: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Subscription {
    pub subscription_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receive {
    pub subscription_id: String,
    pub after: u64,
    pub limit: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Send {
    pub peer_id: String,
    pub namespace: String,
    pub event: String,
    pub payload: Value,
}

pub fn permitted(
    manifest: &PluginManifest,
    namespace: Option<&str>,
    send: bool,
) -> Result<(), String> {
    if namespace.is_some_and(|value| !valid_mesh_namespace(value)) {
        return Err("MESH_EVENT_NAMESPACE_INVALID".into());
    }
    let allowed = manifest.effective_permissions().iter().any(|permission| {
        let namespaces = match permission {
            PermissionNode::MeshEventsSend { namespaces } if send => namespaces,
            PermissionNode::MeshEventsReceive { namespaces } if !send => namespaces,
            _ => return false,
        };
        !namespaces.is_empty()
            && namespaces.iter().all(|value| valid_mesh_namespace(value))
            && namespace.is_none_or(|value| namespaces.iter().any(|item| item == value))
    });
    if allowed {
        Ok(())
    } else {
        Err("PLUGIN_PERMISSION_DENIED".into())
    }
}
pub fn peers_permitted(manifest: &PluginManifest) -> Result<(), String> {
    permitted(manifest, None, false).or_else(|_| permitted(manifest, None, true))
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Owner {
    view: String,
    plugin: String,
}
impl Owner {
    fn from_auth(auth: &ViewAuthorization) -> Self {
        Self {
            view: auth.session_id.clone(),
            plugin: auth.manifest.id.clone(),
        }
    }
}
#[derive(Clone)]
struct Handle {
    owner: Owner,
    namespace: String,
    generation: u64,
    touched: Instant,
    cursor: u64,
    busy: bool,
}
#[derive(Default)]
struct Handles {
    values: HashMap<String, Handle>,
    generation: Option<u64>,
}
impl Handles {
    fn observe_generation(&mut self, generation: u64) -> Result<(), String> {
        if self.generation.is_some_and(|current| current > generation) {
            return Err("MESH_IDENTITY_CHANGED".into());
        }
        self.values
            .retain(|_, handle| handle.generation == generation);
        self.generation = Some(generation);
        Ok(())
    }
    fn prune(&mut self, now: Instant) {
        self.values
            .retain(|_, handle| now.duration_since(handle.touched) < LEASE);
    }
    fn reserve(
        &mut self,
        id: String,
        owner: Owner,
        namespace: String,
        generation: u64,
        now: Instant,
    ) -> Result<(), String> {
        self.observe_generation(generation)?;
        self.prune(now);
        if self.values.len() >= MAX_HANDLES
            || self
                .values
                .values()
                .filter(|handle| handle.owner == owner)
                .count()
                >= MAX_VIEW_HANDLES
        {
            return Err("MESH_EVENT_SUBSCRIPTION_LIMIT".into());
        }
        if self.values.contains_key(&id) {
            return Err("MESH_EVENT_SUBSCRIPTION_INVALID".into());
        }
        self.values.insert(
            id,
            Handle {
                owner,
                namespace,
                generation,
                touched: now,
                cursor: 0,
                busy: true,
            },
        );
        Ok(())
    }
    fn owned(&mut self, id: &str, owner: &Owner, now: Instant) -> Result<Handle, String> {
        if !valid_uuid(id) {
            return Err("MESH_EVENT_SUBSCRIPTION_INVALID".into());
        }
        let handle = self
            .values
            .get(id)
            .filter(|handle| &handle.owner == owner)
            .ok_or("MESH_EVENT_SUBSCRIPTION_UNKNOWN")?;
        if now.duration_since(handle.touched) >= LEASE {
            self.values.remove(id);
            return Err("MESH_EVENT_SUBSCRIPTION_EXPIRED".into());
        }
        Ok(handle.clone())
    }
    fn remove_view(&mut self, view: &str) -> Vec<(String, u64)> {
        let ids: Vec<_> = self
            .values
            .iter()
            .filter(|(_, handle)| handle.owner.view == view)
            .map(|(id, handle)| (id.clone(), handle.generation))
            .collect();
        for (id, _) in &ids {
            self.values.remove(id);
        }
        ids
    }
}
static HANDLES: OnceLock<Mutex<Handles>> = OnceLock::new();
fn handles() -> &'static Mutex<Handles> {
    HANDLES.get_or_init(Default::default)
}
fn transport(generation: Option<u64>, request: Value) -> Result<(u64, Value), String> {
    crate::games::mesh::plugin_event_request(generation, request).map_err(safe_transport_error)
}
fn safe_transport_error(error: String) -> String {
    const EVENT_ERRORS: &[&str] = &[
        "MESH_EVENT_NAMESPACE_INVALID",
        "MESH_EVENT_RESERVED_NAMESPACE",
        "MESH_EVENT_NAME_INVALID",
        "MESH_EVENT_PAYLOAD_INVALID",
        "MESH_EVENT_SUBSCRIPTION_INVALID",
        "MESH_EVENT_SUBSCRIPTION_UNKNOWN",
        "MESH_EVENT_SUBSCRIPTION_LIMIT",
        "MESH_EVENT_NAMESPACE_MISMATCH",
        "MESH_EVENT_NO_LISTENER",
        "MESH_EVENT_QUEUE_FULL",
        "MESH_EVENT_OFFLINE",
        "MESH_EVENT_DELIVERY_UNKNOWN",
        "MESH_EVENT_PEER_UNKNOWN",
        "MESH_EVENT_CURSOR_INVALID",
        "MESH_EVENT_PAGE_INVALID",
        "MESH_EVENT_BUSY",
        "MESH_EVENT_MESSAGE_INVALID",
        "MESH_EVENT_PROTOCOL_UNSUPPORTED",
        "MESH_EVENT_SEQUENCE_EXHAUSTED",
        "MESH_EVENT_ROUTE_GAME_ACTIVE",
        "MESH_EVENT_ROUTE_ACTIVE",
        "MESH_EVENT_ROUTE_NOT_AUTHORIZED",
        "MESH_EVENT_DIRECT_REQUIRED",
        "MESH_EVENT_ROUTE_FAILED",
        "MESH_EVENT_ROUTE_NOT_READY",
        "MESH_EVENT_ROUTE_CHANGING",
    ];
    if EVENT_ERRORS.contains(&error.as_str())
        || matches!(
            error.as_str(),
            "MESH_DISABLED" | "MESH_IDENTITY_CHANGED" | "MESH_APP_BUSY"
        )
    {
        error
    } else {
        "MESH_EVENT_UNAVAILABLE".into()
    }
}
fn active(auth: &ViewAuthorization) -> Result<(), String> {
    if auth.cancel.load(Ordering::Acquire) {
        Err("PLUGIN_VIEW_CLOSED".into())
    } else {
        Ok(())
    }
}
async fn work(
    action: impl FnOnce() -> Result<Value, String> + std::marker::Send + 'static,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(action)
        .await
        .map_err(|_| "MESH_EVENT_UNAVAILABLE")?
}
fn uuid() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "MESH_EVENT_UNAVAILABLE")?;
    bytes[6] = bytes[6] & 0x0f | 0x40;
    bytes[8] = bytes[8] & 0x3f | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}
fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}
fn valid_peer(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}
fn message_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|_| "MESH_EVENT_UNAVAILABLE".into())
}

#[derive(Deserialize)]
struct Profile {
    peer_id: String,
    display_name: String,
}
#[derive(Deserialize)]
struct Contact {
    profile: Profile,
    connected: bool,
}
#[derive(Deserialize)]
struct Capabilities {
    #[serde(default)]
    events: bool,
    #[serde(default)]
    events_public: bool,
}
#[derive(Deserialize)]
struct Status {
    contacts: Vec<Contact>,
    capabilities: Capabilities,
}
fn status() -> Result<(u64, Status), String> {
    let (generation, value) = transport(None, json!({"op":"app_status"}))?;
    let status: Status = decode(value)?;
    if !status.capabilities.events
        || status.contacts.len() > 16
        || status.contacts.iter().any(|contact| {
            !valid_peer(&contact.profile.peer_id)
                || contact.profile.display_name.is_empty()
                || contact.profile.display_name.len() > 128
                || contact.profile.display_name.chars().any(char::is_control)
        })
    {
        return Err("MESH_EVENT_UNAVAILABLE".into());
    }
    handles()
        .lock()
        .map_err(|_| "MESH_EVENT_UNAVAILABLE")?
        .observe_generation(generation)?;
    Ok((generation, status))
}
pub async fn peers(auth: &ViewAuthorization) -> Result<Value, String> {
    let auth = auth.clone();
    work(move || {
        active(&auth)?; peers_permitted(&auth.manifest)?;
        let (_,status) = status()?;
        active(&auth)?;
        Ok(json!({"peers":status.contacts.into_iter().map(|contact| json!({"peerId":contact.profile.peer_id,"displayName":contact.profile.display_name,"connected":contact.connected})).collect::<Vec<_>>()}))
    }).await
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Subscribed {
    subscription_id: String,
    namespace: String,
    cursor: u64,
    lease_seconds: u32,
}
pub async fn subscribe(auth: &ViewAuthorization, request: Subscribe) -> Result<Value, String> {
    let auth = auth.clone();
    work(move || {
        active(&auth)?;
        permitted(&auth.manifest, Some(&request.namespace), false)?;
        let (generation, _) = status()?;
        active(&auth)?;
        let id = uuid()?;
        let owner = Owner::from_auth(&auth);
        handles()
            .lock()
            .map_err(|_| "MESH_EVENT_UNAVAILABLE")?
            .reserve(
                id.clone(),
                owner.clone(),
                request.namespace.clone(),
                generation,
                Instant::now(),
            )?;
        let response = transport(
            Some(generation),
            json!({"op":"app_event_subscribe","subscription_id":id,"namespace":request.namespace}),
        )
        .and_then(|(_, value)| decode::<Subscribed>(value));
        let response = response.and_then(|value| {
            if value.subscription_id == id
                && value.namespace == request.namespace
                && value.cursor == 0
                && value.lease_seconds == 120
            {
                Ok(())
            } else {
                Err("MESH_EVENT_UNAVAILABLE".into())
            }
        });
        if response.is_err() || active(&auth).is_err() {
            if let Ok(mut state) = handles().lock() {
                state.values.remove(&id);
            }
            cleanup(vec![(id, generation)]);
            active(&auth)?;
            return Err(response
                .err()
                .unwrap_or_else(|| "PLUGIN_VIEW_CLOSED".into()));
        }
        let mut state = handles().lock().map_err(|_| "MESH_EVENT_UNAVAILABLE")?;
        let handle = state
            .values
            .get_mut(&id)
            .filter(|handle| handle.owner == owner)
            .ok_or("PLUGIN_VIEW_CLOSED")?;
        handle.busy = false;
        handle.touched = Instant::now();
        Ok(json!({"subscriptionId":id,"cursor":0}))
    })
    .await
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Event {
    sequence: u64,
    message_id: String,
    peer_id: String,
    namespace: String,
    event: String,
    payload: Value,
    received_at: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Page {
    messages: Vec<Event>,
    cursor: u64,
    missed: bool,
}
fn validate_page(page: &Page, namespace: &str, after: u64, limit: u32) -> Result<(), String> {
    let mut previous = after;
    if page.messages.len() > limit as usize || page.cursor < after || page.cursor > MAX_SAFE {
        return Err("MESH_EVENT_UNAVAILABLE".into());
    }
    for event in &page.messages {
        if event.sequence <= previous
            || event.sequence > page.cursor
            || !message_id(&event.message_id)
            || !valid_peer(&event.peer_id)
            || event.namespace != namespace
            || !valid_mesh_event(&event.event)
            || validate_mesh_payload(&event.payload).is_err()
            || event.received_at > MAX_SAFE
        {
            return Err("MESH_EVENT_UNAVAILABLE".into());
        }
        previous = event.sequence;
    }
    Ok(())
}
pub async fn receive(auth: &ViewAuthorization, request: Receive) -> Result<Value, String> {
    let auth = auth.clone();
    work(move || {
        active(&auth)?;
        if request.limit == 0 || request.limit > 4 { return Err("MESH_EVENT_PAGE_INVALID".into()); }
        let owner = Owner::from_auth(&auth);
        let handle = {
            let mut state = handles().lock().map_err(|_| "MESH_EVENT_UNAVAILABLE")?;
            let handle = state.owned(&request.subscription_id,&owner,Instant::now())?;
            permitted(&auth.manifest,Some(&handle.namespace),false)?;
            if handle.busy { return Err("MESH_APP_BUSY".into()); }
            if request.after > handle.cursor { return Err("MESH_EVENT_CURSOR_INVALID".into()); }
            state.values.get_mut(&request.subscription_id).unwrap().busy = true;
            handle
        };
        let result = transport(Some(handle.generation),json!({"op":"app_event_read","subscription_id":request.subscription_id,"after":request.after,"limit":request.limit})).and_then(|(_,value)| decode::<Page>(value)).and_then(|page| { validate_page(&page,&handle.namespace,request.after,request.limit)?; Ok(page) });
        let mut state = handles().lock().map_err(|_| "MESH_EVENT_UNAVAILABLE")?;
        let Some(current) = state.values.get_mut(&request.subscription_id).filter(|current| current.owner == owner) else { active(&auth)?; return Err("MESH_EVENT_SUBSCRIPTION_UNKNOWN".into()); };
        current.busy = false;
        if let Ok(page) = &result { current.cursor = current.cursor.max(page.cursor); current.touched = Instant::now(); }
        if active(&auth).is_err() || result.as_ref().is_err_and(|error| matches!(error.as_str(),"MESH_DISABLED" | "MESH_IDENTITY_CHANGED" | "MESH_EVENT_SUBSCRIPTION_UNKNOWN" | "MESH_EVENT_SUBSCRIPTION_INVALID")) { state.values.remove(&request.subscription_id); }
        drop(state); active(&auth)?;
        serde_json::to_value(result?).map_err(|_| "MESH_EVENT_UNAVAILABLE".into())
    }).await
}
pub async fn unsubscribe(auth: &ViewAuthorization, request: Subscription) -> Result<Value, String> {
    let auth = auth.clone();
    work(move || {
        active(&auth)?;
        let handle = {
            let mut state = handles().lock().map_err(|_| "MESH_EVENT_UNAVAILABLE")?;
            let handle = state.owned(
                &request.subscription_id,
                &Owner::from_auth(&auth),
                Instant::now(),
            )?;
            permitted(&auth.manifest, Some(&handle.namespace), false)?;
            state.values.remove(&request.subscription_id);
            handle
        };
        let (_, value) = transport(
            Some(handle.generation),
            json!({"op":"app_event_unsubscribe","subscription_id":request.subscription_id}),
        )?;
        let response: Unsubscribed = decode(value)?;
        if response.subscription_id != request.subscription_id {
            return Err("MESH_EVENT_UNAVAILABLE".into());
        }
        active(&auth)?;
        Ok(json!({"subscriptionId":request.subscription_id,"removed":response.removed}))
    })
    .await
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Unsubscribed {
    subscription_id: String,
    removed: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Sent {
    message_id: String,
    status: String,
}
pub async fn send(auth: &ViewAuthorization, request: Send) -> Result<Value, String> {
    let auth = auth.clone();
    work(move || {
        active(&auth)?; permitted(&auth.manifest,Some(&request.namespace),true)?;
        if !valid_mesh_event(&request.event) { return Err("MESH_EVENT_NAME_INVALID".into()); }
        validate_mesh_payload(&request.payload).map_err(|error| error.code.to_owned())?;
        if !valid_peer(&request.peer_id) { return Err("MESH_EVENT_PEER_UNKNOWN".into()); }
        let (generation,status) = status()?;
        let peer = status.contacts.iter().find(|contact| contact.profile.peer_id == request.peer_id).ok_or("MESH_EVENT_PEER_UNKNOWN")?;
        if !peer.connected && !status.capabilities.events_public { return Err("MESH_EVENT_OFFLINE".into()); }
        active(&auth)?;
        let (_,value) = transport(Some(generation),json!({"op":"app_event_send","peer_id":request.peer_id,"namespace":request.namespace,"event":request.event,"payload":request.payload}))?;
        let result: Sent = decode(value).map_err(|_| "MESH_EVENT_DELIVERY_UNKNOWN")?;
        if !message_id(&result.message_id) || result.status != "received" { return Err("MESH_EVENT_DELIVERY_UNKNOWN".into()); }
        active(&auth)?;
        Ok(json!({"messageId":result.message_id,"status":"received"}))
    }).await
}
// Removing handles is synchronous. One bounded worker performs best-effort local
// companion unsubscription; closing a WebView never waits on a network request.
fn cleanup(values: Vec<(String, u64)>) {
    if values.is_empty() {
        return;
    }
    static WORKER: OnceLock<mpsc::SyncSender<(String, u64)>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel::<(String, u64)>(MAX_HANDLES);
        std::thread::spawn(move || {
            for (id, generation) in receiver {
                let _ = transport(
                    Some(generation),
                    json!({"op":"app_event_unsubscribe","subscription_id":id}),
                );
            }
        });
        sender
    });
    for value in values {
        let _ = worker.try_send(value);
    }
}
pub fn revoke_session(view: &str) {
    let removed = handles()
        .lock()
        .map(|mut state| state.remove_view(view))
        .unwrap_or_default();
    cleanup(removed);
}
pub(crate) fn reset_generation(generation: u64) {
    if let Ok(mut state) = handles().lock() {
        state.values.clear();
        state.generation = Some(generation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn owner(view: &str) -> Owner {
        Owner {
            view: view.into(),
            plugin: "plugin-a".into(),
        }
    }
    fn manifest() -> PluginManifest {
        serde_json::from_str(include_str!(
            "../../../../examples/ai-assistant/plugin.json"
        ))
        .unwrap()
    }
    #[test]
    fn exact_namespaces_and_directions_do_not_imply_other_grants() {
        let mut manifest = manifest();
        manifest.permissions = Some(vec![PermissionNode::MeshEventsReceive {
            namespaces: vec!["org.example.ready".into()],
        }]);
        assert!(permitted(&manifest, Some("org.example.ready"), false).is_ok());
        assert!(permitted(&manifest, Some("org.example.ready"), true).is_err());
        assert!(permitted(&manifest, Some("org.example.other"), false).is_err());
        assert!(permitted(&manifest, Some("org.example.ready.child"), false).is_err());
        assert!(permitted(&manifest, Some("cobble.internal"), false).is_err());
        assert!(peers_permitted(&manifest).is_ok());
        manifest.permissions = Some(vec![PermissionNode::AiChat {}]);
        assert!(peers_permitted(&manifest).is_err());
    }
    #[test]
    fn handles_cannot_be_stolen_by_another_view_or_plugin() {
        let mut state = Handles::default();
        let id = uuid().unwrap();
        let now = Instant::now();
        state
            .reserve(
                id.clone(),
                owner("view-a"),
                "org.example.ready".into(),
                4,
                now,
            )
            .unwrap();
        assert!(state.owned(&id, &owner("view-a"), now).is_ok());
        assert!(state.owned(&id, &owner("view-b"), now).is_err());
        let other = Owner {
            view: "view-a".into(),
            plugin: "plugin-b".into(),
        };
        assert!(state.owned(&id, &other, now).is_err());
        assert!(state.owned("../other", &owner("view-a"), now).is_err());
        assert_eq!(state.remove_view("view-a"), vec![(id, 4)]);
        assert!(state.values.is_empty());
    }
    #[test]
    fn identity_change_invalidates_old_handles_and_ignores_stale_contexts() {
        let mut state = Handles::default();
        let now = Instant::now();
        let first = uuid().unwrap();
        let second = uuid().unwrap();
        state
            .reserve(
                first.clone(),
                owner("view-a"),
                "org.example.ready".into(),
                4,
                now,
            )
            .unwrap();
        state
            .reserve(
                second.clone(),
                owner("view-b"),
                "org.example.ready".into(),
                5,
                now,
            )
            .unwrap();
        assert!(state.owned(&first, &owner("view-a"), now).is_err());
        assert!(state.observe_generation(4).is_err());
        assert!(state.owned(&second, &owner("view-b"), now).is_ok());
    }
    #[test]
    fn handles_have_per_view_limits_and_expire_without_a_read() {
        let mut state = Handles::default();
        let now = Instant::now();
        let mut first = String::new();
        for index in 0..MAX_VIEW_HANDLES {
            let id = uuid().unwrap();
            if index == 0 {
                first = id.clone();
            }
            state
                .reserve(id, owner("view-a"), "org.example.ready".into(), 4, now)
                .unwrap();
        }
        assert_eq!(
            state
                .reserve(
                    uuid().unwrap(),
                    owner("view-a"),
                    "org.example.ready".into(),
                    4,
                    now
                )
                .unwrap_err(),
            "MESH_EVENT_SUBSCRIPTION_LIMIT"
        );
        assert_eq!(
            state
                .owned(&first, &owner("view-a"), now + LEASE)
                .err()
                .unwrap(),
            "MESH_EVENT_SUBSCRIPTION_EXPIRED"
        );
        state.prune(now + LEASE);
        assert!(state.values.is_empty());
    }
    #[test]
    fn returned_pages_are_bounded_ordered_and_namespace_scoped() {
        let event = || Event {
            sequence: 1,
            message_id: "a".repeat(32),
            peer_id: "actualNoisePeer".into(),
            namespace: "org.example.ready".into(),
            event: "ready".into(),
            payload: json!({"ready":true}),
            received_at: 123,
        };
        let mut page = Page {
            messages: vec![event()],
            cursor: 1,
            missed: false,
        };
        assert!(validate_page(&page, "org.example.ready", 0, 4).is_ok());
        assert!(validate_page(&page, "org.example.other", 0, 4).is_err());
        page.messages.push(event());
        assert!(validate_page(&page, "org.example.ready", 0, 4).is_err());
        page.messages.pop();
        page.messages[0].payload = json!("x".repeat(1024));
        assert!(validate_page(&page, "org.example.ready", 0, 4).is_err());
        page.messages[0].payload = Value::Null;
        page.cursor = MAX_SAFE + 1;
        assert!(validate_page(&page, "org.example.ready", 0, 4).is_err());
    }
    #[test]
    fn requests_are_closed_and_transport_errors_never_expose_raw_content() {
        assert!(
            serde_json::from_value::<Subscribe>(
                json!({"namespace":"org.example.ready","pluginId":"spoof"})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<Receive>(
                json!({"subscriptionId":"id","after":0,"limit":4,"namespace":"spoof"})
            )
            .is_err()
        );
        assert!(serde_json::from_value::<Send>(json!({"peerId":"peer","namespace":"org.example.ready","event":"ready","payload":null,"address":"https://example.invalid"})).is_err());
        assert_eq!(
            safe_transport_error("PRIVATE peer/name/payload/path".into()),
            "MESH_EVENT_UNAVAILABLE"
        );
        assert_eq!(
            safe_transport_error("MESH_EVENT_DELIVERY_UNKNOWN".into()),
            "MESH_EVENT_DELIVERY_UNKNOWN"
        );
    }
}
