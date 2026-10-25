// SPDX-License-Identifier: MIT
//! Opt-in local-network identity/contact service on the owned private companion.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicProfile {
    pub peer_id: String,
    pub display_name: String,
    pub suffix: String,
    pub revision: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    pub profile: PublicProfile,
    pub connected: bool,
    pub last_seen: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    #[serde(default)]
    events: bool,
    #[serde(default)]
    events_public: bool,
    contact_invites: bool,
    direct_messages: bool,
    lan_only: bool,
    relay_messages: bool,
    groups: bool,
    vlan: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppSnapshot {
    pub profile: PublicProfile,
    pub contacts: Vec<Contact>,
    capabilities: Capabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    event_route: Option<EventRoute>,
    #[serde(default)]
    public_events_paused: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EventRoute {
    authorization: overview::RoutingAuthorization,
    relay_peer_id: String,
    state: String,
    game_coexistence: bool,
    last_delivery: Option<EventDelivery>,
    last_error: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EventDelivery {
    message_id: String,
    peer_id: String,
    connection_id: String,
    route: String,
}
impl AppSnapshot {
    pub(super) fn events_public(&self) -> bool {
        self.capabilities.events_public
    }
    pub(super) fn event_route_failed(&self) -> bool {
        self.event_route
            .as_ref()
            .is_some_and(|route| route.state == "failed")
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContactInvitation {
    invitation: String,
    expires_at: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MessageReceipt {
    id: String,
    received: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    sequence: u64,
    id: String,
    peer_id: String,
    direction: String,
    text: String,
    at: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MessagePage {
    messages: Vec<Message>,
    next_after: u64,
    oldest_sequence: Option<u64>,
    has_more: bool,
    generation: String,
}
fn plain(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn valid_name(value: &str) -> bool {
    plain(value, 128) && value.chars().count() <= 32
}
fn peer(value: &str) -> bool {
    plain(value, 128) && value.bytes().all(|b| b.is_ascii_alphanumeric())
}
fn valid_profile(value: &PublicProfile) -> bool {
    peer(&value.peer_id) && valid_name(&value.display_name) && plain(&value.suffix, 16)
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|_| "MESH_INVALID_RESPONSE".into())
}
pub(super) fn snapshot(broker: &mut Broker) -> Result<Option<AppSnapshot>, String> {
    if broker.social_client.is_none() {
        return Ok(None);
    }
    let value = match request(broker, json!({"op":"app_status"})) {
        Ok(value) => value,
        Err(error) if error == "MESH_APP_STOPPED" => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut value: AppSnapshot = decode(value)?;
    validate_snapshot(&value)?;
    value.public_events_paused = app_route::paused();
    Ok(Some(value))
}
pub(super) fn validate_snapshot(value: &AppSnapshot) -> Result<(), String> {
    if !valid_profile(&value.profile)
        || value.contacts.len() > 16
        || value.contacts.iter().any(|c| !valid_profile(&c.profile))
        || value.capabilities.lan_only == value.capabilities.events_public
        || (!value.capabilities.events_public && value.capabilities.relay_messages)
        || value.capabilities.events_public != value.event_route.is_some()
        || value.event_route.as_ref().is_some_and(|route| {
            !peer(&route.relay_peer_id)
                || route.game_coexistence
                || !matches!(route.state.as_str(), "starting" | "ready" | "failed")
                || route.last_error.as_deref().is_some_and(|error| {
                    !matches!(error, "event_route_failed" | "event_direct_required")
                })
                || route.last_delivery.as_ref().is_some_and(|delivery| {
                    delivery.message_id.len() != 32
                        || !delivery
                            .message_id
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit())
                        || !peer(&delivery.peer_id)
                        || !plain(&delivery.connection_id, 128)
                        || !matches!(delivery.route.as_str(), "direct" | "relay" | "lan")
                })
                || value.capabilities.relay_messages
                    == matches!(
                        route.authorization,
                        overview::RoutingAuthorization::Coordination
                    )
        })
        || value.capabilities.groups
        || value.capabilities.vlan
    {
        return Err("MESH_INVALID_RESPONSE".into());
    }
    Ok(())
}
fn request(broker: &mut Broker, command: Value) -> Result<Value, String> {
    enabled::require(broker)?;
    broker
        .social_client
        .as_mut()
        .ok_or("MESH_APP_STOPPED")?
        .request(command)
}
pub(super) fn private_directory() -> Result<PathBuf, String> {
    let directory = dirs::data_local_dir()
        .ok_or("MESH_STORAGE_UNAVAILABLE")?
        .join("CobbleLauncher/mesh-identity");
    std::fs::create_dir_all(directory.parent().ok_or("MESH_STORAGE_UNAVAILABLE")?)
        .map_err(|_| "MESH_STORAGE_UNAVAILABLE")?;
    // Companion creates/opens this native-derived private root and holds its lifetime lock.
    Ok(directory)
}
#[tauri::command]
pub async fn mesh_start_app(display_name: String) -> Result<AppSnapshot, String> {
    let activity = MeshActivity::new("identity_start");
    if !valid_name(display_name.trim()) {
        return Err("MESH_INVALID_NAME".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        enabled::require(&mut broker)?;
        if let Some(snapshot) = snapshot(&mut broker)? {
            return Ok(snapshot);
        }
        let client = broker.shared_client()?;
        let directory = broker
            .temporary_identity
            .as_ref()
            .map(|root| root.path().join("identity"))
            .map(Ok)
            .unwrap_or_else(private_directory)?;
        let value = client.request(
            json!({"op":"app_initialize","directory":directory,"display_name":display_name.trim()}),
        )?;
        let initial: AppSnapshot = decode(value)?;
        if !valid_profile(&initial.profile) {
            return Err("MESH_INVALID_RESPONSE".into());
        }
        broker.social_client = Some(client);
        broker.social_generation = broker.social_generation.wrapping_add(1);
        snapshot(&mut broker)?.ok_or_else(|| "MESH_APP_STOPPED".into())
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?;
    if result.is_ok() {
        app_route::schedule();
    }
    activity.complete(result)
}
#[tauri::command]
pub async fn mesh_app_snapshot() -> Result<Option<AppSnapshot>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        snapshot(&mut *broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?)
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?
}
#[tauri::command]
pub async fn mesh_stop_app(app: AppHandle) -> Result<bool, String> {
    let activity = MeshActivity::new("identity_stop");
    super::composer::cancel();
    let result = super::network::mesh_network_set_enabled(app, false).map(|_| true);
    activity.complete(result)
}
#[tauri::command]
pub async fn mesh_set_display_name(display_name: String) -> Result<PublicProfile, String> {
    let activity = MeshActivity::new("identity_rename");
    if !valid_name(display_name.trim()) {
        return Err("MESH_INVALID_NAME".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let profile: PublicProfile = decode(request(
            &mut broker,
            json!({"op":"app_rename","display_name":display_name.trim()}),
        )?)?;
        if !valid_profile(&profile) {
            return Err("MESH_INVALID_RESPONSE".into());
        }
        Ok(profile)
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?;
    activity.complete(result)
}
#[tauri::command]
pub async fn mesh_create_contact_invitation() -> Result<ContactInvitation, String> {
    let activity = MeshActivity::new("contact_invite");
    let result = tauri::async_runtime::spawn_blocking(|| {
        app_route::reconcile_now()?;
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let value: ContactInvitation = decode(request(&mut broker, json!({"op":"app_invite"}))?)?;
        if value.invitation.is_empty() || value.invitation.len() > 6000 {
            return Err("MESH_INVALID_RESPONSE".into());
        }
        Ok(value)
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?;
    activity.complete(result)
}
#[tauri::command]
pub async fn mesh_accept_contact_invitation(invitation: String) -> Result<Contact, String> {
    let activity = MeshActivity::new("contact_accept");
    if invitation.is_empty() || invitation.len() > 6000 {
        return Err("MESH_INVALID_INVITATION".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        app_route::reconcile_now()?;
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let contact: Contact = decode(request(
            &mut broker,
            json!({"op":"app_accept","invitation":invitation}),
        )?)?;
        if !valid_profile(&contact.profile) {
            return Err("MESH_INVALID_RESPONSE".into());
        }
        Ok(contact)
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?;
    activity.complete(result)
}
#[tauri::command]
pub async fn mesh_send_message(peer_id: String, text: String) -> Result<MessageReceipt, String> {
    let activity = MeshActivity::new("message_send");
    if !peer(&peer_id)
        || text.trim().is_empty()
        || text.len() > 1024
        || text.chars().any(|c| c.is_control() && c != '\n')
    {
        return Err("MESH_INVALID_MESSAGE".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let current = snapshot(&mut broker)?.ok_or("MESH_APP_STOPPED")?;
        if !current
            .contacts
            .iter()
            .any(|c| c.profile.peer_id == peer_id)
        {
            return Err("MESH_UNKNOWN_CONTACT".into());
        }
        let value: MessageReceipt = decode(request(
            &mut broker,
            json!({"op":"app_send","peer_id":peer_id,"text":text}),
        )?)?;
        if !value.received || !plain(&value.id, 128) {
            return Err("MESH_INVALID_RESPONSE".into());
        }
        Ok(value)
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?;
    activity.complete(result)
}
#[tauri::command]
pub async fn mesh_list_messages(after: u64, limit: u32) -> Result<MessagePage, String> {
    if !(1..=40).contains(&limit) {
        return Err("MESH_INVALID_PAGE".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let page: MessagePage = decode(request(
            &mut broker,
            json!({"op":"app_messages","after":after,"limit":limit}),
        )?)?;
        if page.messages.len() > limit as usize
            || !plain(&page.generation, 128)
            || page.messages.iter().any(|m| {
                !peer(&m.peer_id)
                    || !plain(&m.id, 128)
                    || m.text.len() > 1024
                    || !matches!(m.direction.as_str(), "incoming" | "outgoing")
            })
        {
            return Err("MESH_INVALID_RESPONSE".into());
        }
        Ok(page)
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_route_status_uses_application_evidence_and_keeps_legacy_lan() {
        let mut value = json!({"profile":{"peer_id":"FixturePeer","display_name":"Fixture","suffix":"1234","revision":1},"contacts":[],"capabilities":{"contact_invites":true,"direct_messages":true,"lan_only":true,"relay_messages":false,"groups":false,"vlan":false,"events":true}});
        let old: AppSnapshot = decode(value.clone()).unwrap();
        assert!(validate_snapshot(&old).is_ok());
        assert!(!old.events_public());
        value["capabilities"]["events_public"] = json!(true);
        value["capabilities"]["lan_only"] = json!(false);
        value["capabilities"]["relay_messages"] = json!(true);
        value["event_route"] = json!({"authorization":"legacy_test","relay_peer_id":"FixtureRelay","state":"starting","game_coexistence":false,"last_delivery":null,"last_error":null});
        for state in ["starting", "ready", "failed"] {
            value["event_route"]["state"] = json!(state);
            let snapshot: AppSnapshot = decode(value.clone()).unwrap();
            assert!(validate_snapshot(&snapshot).is_ok());
        }
        value["event_route"]["last_delivery"] = json!({"message_id":"a".repeat(32),"peer_id":"ActualNoisePeer","connection_id":"12","route":"relay"});
        let snapshot: AppSnapshot = decode(value.clone()).unwrap();
        assert!(validate_snapshot(&snapshot).is_ok());
        value["event_route"]["last_error"] = json!("PRIVATE raw body");
        assert!(validate_snapshot(&decode(value.clone()).unwrap()).is_err());
        value["event_route"]["last_error"] = Value::Null;
        value["event_route"]["game_coexistence"] = json!(true);
        assert!(validate_snapshot(&decode(value).unwrap()).is_err());
    }
    #[test]
    fn public_metadata_and_message_boundaries_do_not_accept_private_fields() {
        let profile = json!({"peer_id":"12D3KooWTestPeer","display_name":"Name \\\"quoted\\\"","suffix":"ABCD","revision":1});
        let value: PublicProfile = decode(profile.clone()).unwrap();
        assert!(valid_profile(&value));
        let mut secret = profile;
        secret["private_key"] = json!("never-render");
        assert!(decode::<PublicProfile>(secret).is_err());
        assert!(!plain("name\nsecret", 64));
        assert!(!plain(&"中".repeat(22), 64));
        assert!(valid_name(&"中".repeat(32)));
        assert!(valid_name(&"😀".repeat(32)));
        assert!(!valid_name(&"a".repeat(33)));
        assert!(!peer("127.0.0.1:3000"));
        let frame = json!({"version":1,"id":1,"command":{"op":"app_send","peer_id":"12D3KooWTestPeer","text":"\\\"\n".repeat(512)}});
        assert!(serde_json::to_vec(&frame).unwrap().len() < client::MAX_FRAME);
    }
    #[test]
    fn stopped_snapshot_never_starts_helper() {
        let mut state = Broker::default();
        assert!(snapshot(&mut state).unwrap().is_none());
        assert!(state.social_client.is_none());
    }
    #[test]
    #[ignore = "Requires explicit owned companion; starts loopback/LAN service in disposable identity directory, no public network"]
    fn actual_social_companion_releases_identity_lock_and_persists_public_id() {
        let helper = std::env::var_os("COBBLE_MESH_TEST_HELPER").expect("explicithelperpath");
        let directory = tempfile::tempdir().unwrap();
        let client = client::Client::spawn(std::path::Path::new(&helper)).unwrap();
        let snapshot:AppSnapshot=decode(client.request(json!({"op":"app_initialize","directory":directory.path().join("identity"),"display_name":"Native fixture"})).unwrap()).unwrap();
        assert!(valid_profile(&snapshot.profile));
        let renamed: PublicProfile = decode(
            client
                .request(json!({"op":"app_rename","display_name":"Native renamed"}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(renamed.peer_id, snapshot.profile.peer_id);
        let invite: ContactInvitation =
            decode(client.request(json!({"op":"app_invite"})).unwrap()).unwrap();
        assert!(!invite.invitation.is_empty());
        let page: MessagePage = decode(
            client
                .request(json!({"op":"app_messages","after":0,"limit":40}))
                .unwrap(),
        )
        .unwrap();
        assert!(page.messages.is_empty());
        client.shutdown();
        drop(client);

        let reader = client::Client::spawn(std::path::Path::new(&helper)).unwrap();
        let saved: PublicProfile = decode(reader.request(json!({"op":"app_identity_profile","directory":directory.path().join("identity")})).unwrap()).unwrap();
        assert_eq!(saved, renamed);
        reader.shutdown();
        drop(reader);
        let ephemeral = tempfile::tempdir().unwrap();
        let ephemeral_path = ephemeral.path().to_owned();
        let guest = client::Client::spawn(std::path::Path::new(&helper)).unwrap();
        let temporary: AppSnapshot = decode(guest.request(json!({"op":"app_initialize","directory":ephemeral.path().join("identity"),"display_name":"Temporary"})).unwrap()).unwrap();
        assert_ne!(temporary.profile.peer_id, renamed.peer_id);
        assert!(temporary.contacts.is_empty());
        let page: MessagePage = decode(
            guest
                .request(json!({"op":"app_messages","after":0,"limit":40}))
                .unwrap(),
        )
        .unwrap();
        assert!(page.messages.is_empty());
        guest.shutdown();
        drop(guest);
        drop(ephemeral);
        assert!(!ephemeral_path.exists());
        let second = client::Client::spawn(std::path::Path::new(&helper)).unwrap();
        let reopened:AppSnapshot=decode(second.request(json!({"op":"app_initialize","directory":directory.path().join("identity"),"display_name":"Ignored fallback"})).unwrap()).unwrap();
        assert_eq!(reopened.profile.peer_id, snapshot.profile.peer_id);
        assert_eq!(reopened.profile.display_name, "Native renamed");
        second.shutdown();
        drop(second);
    }
}
#[tauri::command]
pub async fn mesh_remove_contact(peer_id: String) -> Result<AppSnapshot, String> {
    let activity = MeshActivity::new("contact_remove");
    if !peer(&peer_id) {
        return Err("MESH_UNKNOWN_CONTACT".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let current = snapshot(&mut broker)?.ok_or("MESH_APP_STOPPED")?;
        if !current
            .contacts
            .iter()
            .any(|c| c.profile.peer_id == peer_id)
        {
            return Err("MESH_UNKNOWN_CONTACT".into());
        }
        super::composer::cancel();
        let _: AppSnapshot = decode(request(
            &mut broker,
            json!({"op":"app_remove","peer_id":peer_id}),
        )?)?;
        snapshot(&mut broker)?.ok_or_else(|| "MESH_APP_STOPPED".into())
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?;
    activity.complete(result)
}

/// Shared contact, network and multiplayer identity.
#[derive(Serialize)]
pub struct IdentityState {
    profile: Option<PublicProfile>,
    mode: &'static str,
    active: bool,
}
#[tauri::command]
pub async fn mesh_identity_state() -> Result<IdentityState, String> {
    let result = tauri::async_runtime::spawn_blocking(|| {
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        let current = snapshot(&mut broker)?;
        let profile = if let Some(value) = &current {
            Some(value.profile.clone())
        } else {
            let client = client::Client::spawn(&helper_path()?)?;
            let value = client
                .request(json!({"op":"app_identity_profile","directory":broker.temporary_identity.as_ref().map(|root|root.path().join("identity")).map(Ok).unwrap_or_else(private_directory)?}));
            client.shutdown();
            let profile: Option<PublicProfile> = decode(value?)?;
            if profile.as_ref().is_some_and(|p| !valid_profile(p)) {
                return Err("MESH_INVALID_RESPONSE".into());
            }
            profile
        };
        Ok(IdentityState {
            active: current.is_some(),
            profile,
            mode: if broker.temporary_identity.is_some() {
                "temporary"
            } else {
                "persistent"
            },
        })
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?;
    if result.is_err() {
        // Stage only: never log profile data, private paths or companion payloads.
        tracing::warn!(event = "mesh_identity_load_failed");
    }
    result
}
fn start_identity(
    broker: &mut Broker,
    directory: PathBuf,
    name: &str,
) -> Result<AppSnapshot, String> {
    enabled::require(broker)?;
    let client = broker.shared_client()?;
    let initial: AppSnapshot = decode(
        client.request(json!({"op":"app_initialize","directory":directory,"display_name":name}))?,
    )?;
    if !valid_profile(&initial.profile) {
        return Err("MESH_INVALID_RESPONSE".into());
    }
    broker.social_client = Some(client);
    broker.social_generation = broker.social_generation.wrapping_add(1);
    snapshot(broker)?.ok_or_else(|| "MESH_APP_STOPPED".into())
}
#[tauri::command]
pub async fn mesh_set_identity_mode(
    app: AppHandle,
    temporary: bool,
    display_name: String,
) -> Result<AppSnapshot, String> {
    if !valid_name(display_name.trim()) {
        return Err("MESH_INVALID_NAME".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        super::composer::cancel();
        let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        super::network::prepare_identity(&mut broker);
        enabled::require(&mut broker)?;
        if temporary == broker.temporary_identity.is_some()
            && let Some(current) = snapshot(&mut broker)?
        {
            return Ok(current);
        }
        // TempDir owns a private, unguessable native-only root; no renderer paths.
        let next = if temporary {
            Some(
                tempfile::Builder::new()
                    .prefix("cobble-contact-identity-")
                    .tempdir()
                    .map_err(|_| "MESH_STORAGE_UNAVAILABLE")?,
            )
        } else {
            None
        };
        let directory = next
            .as_ref()
            .map(|root| root.path().join("identity"))
            .map(Ok)
            .unwrap_or_else(private_directory)?;
        super::network::invalidate(&app)?;
        enabled::stop_all(&mut broker);
        broker.social_generation = broker.social_generation.wrapping_add(1);
        // Shut down before removing all temporary contact/history/key material.
        broker.temporary_identity = next;
        let result = start_identity(&mut broker, directory, display_name.trim());
        drop(broker);
        if result.is_ok() {
            let _ = super::network::mesh_network_start(app);
        }
        result
    })
    .await
    .map_err(|_| "MESH_OPERATION_FAILED")?
}
