// SPDX-License-Identifier: MIT
//! Bounded, one-shot typed handoffs between opted-in plugin interfaces.
use cobble_plugin_api::{InstalledPlugin, InterfaceRequirement, PluginManifest, ProvidedInterface};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

pub const WAYPOINT_INTERFACE: &str = "launcher:waypoint-import";
pub const WAYPOINT_VERSION: &str = "0.1.0";
const MAX_PENDING: usize = 32;
const TTL: Duration = Duration::from_secs(5 * 60);
const MAX_PAYLOAD_BYTES: usize = 2048;
const WORLD_BORDER: i32 = 29_999_984;
static QUEUE: OnceLock<Mutex<VecDeque<Pending>>> = OnceLock::new();
fn queue() -> &'static Mutex<VecDeque<Pending>> {
    QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Waypoint {
    pub name: String,
    pub dimension: Dimension,
    pub x: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    pub z: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Dimension {
    Overworld,
    Nether,
    End,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendRequest {
    pub target_plugin_id: String,
    pub interface_id: String,
    pub version: String,
    pub game_id: String,
    pub world_id: Option<String>,
    pub payload: Waypoint,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receiver {
    pub plugin_id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffDraft {
    pub handoff_id: String,
    pub sender: PluginIdentity,
    pub game: GameIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world_id: Option<String>,
    pub payload: Waypoint,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginIdentity {
    pub plugin_id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameIdentity {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct SendResult {
    pub requested: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestEvent {
    handoff_id: String,
    target_plugin_id: String,
    game_id: String,
}
struct Pending {
    id: String,
    sender: PluginIdentity,
    target_plugin_id: String,
    game: GameIdentity,
    world_id: Option<String>,
    payload: Waypoint,
    expires: Instant,
}

fn safe_text(value: &str, max: usize, allow_empty: bool) -> bool {
    value.len() <= max
        && (allow_empty || !value.trim().is_empty())
        && !value.chars().any(char::is_control)
}
fn safe_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn safe_world_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && !value.chars().any(char::is_control)
        && !value.contains('/')
        && !value.contains('\\')
}
fn supported_request(manifest: &PluginManifest) -> bool {
    let provided = ProvidedInterface {
        id: WAYPOINT_INTERFACE.into(),
        version: WAYPOINT_VERSION.into(),
    };
    manifest
        .interfaces
        .optional
        .iter()
        .chain(&manifest.interfaces.requires)
        .any(|requirement: &InterfaceRequirement| requirement.matches(&provided))
}
fn provides_waypoint(manifest: &PluginManifest) -> bool {
    manifest
        .interfaces
        .provides
        .iter()
        .any(|provided| provided.id == WAYPOINT_INTERFACE && provided.version == WAYPOINT_VERSION)
}
fn eligible_receiver(plugin: &InstalledPlugin) -> bool {
    plugin.enabled
        && plugin.problem.is_none()
        && plugin.manifest.frontend.is_some()
        && provides_waypoint(&plugin.manifest)
}
pub fn receivers(
    sender: &PluginManifest,
    plugins: &[InstalledPlugin],
) -> Result<Vec<Receiver>, String> {
    if !supported_request(sender) {
        return Err("PLUGIN_HANDOFF_INTERFACE_NOT_DECLARED".into());
    }
    Ok(plugins
        .iter()
        .filter(|plugin| plugin.manifest.id != sender.id && eligible_receiver(plugin))
        .map(|plugin| Receiver {
            plugin_id: plugin.manifest.id.clone(),
            name: plugin.manifest.name.clone(),
        })
        .take(64)
        .collect())
}
fn validate(
    request: &SendRequest,
    sender: &PluginManifest,
    bound_game_id: Option<&str>,
    receiver: &InstalledPlugin,
) -> Result<(), String> {
    if request.interface_id != WAYPOINT_INTERFACE || request.version != WAYPOINT_VERSION {
        return Err("PLUGIN_HANDOFF_INTERFACE_UNSUPPORTED".into());
    }
    if !supported_request(sender)
        || !provides_waypoint(&receiver.manifest)
        || !receiver.enabled
        || receiver.problem.is_some()
        || receiver.manifest.frontend.is_none()
        || receiver.manifest.id != request.target_plugin_id
        || sender.id == receiver.manifest.id
    {
        return Err("PLUGIN_HANDOFF_RECEIVER_UNAVAILABLE".into());
    }
    if bound_game_id.is_some_and(|bound| bound != request.game_id) {
        return Err("PLUGIN_HANDOFF_GAME_SCOPE".into());
    }
    if !safe_identifier(&request.game_id, 128)
        || request
            .world_id
            .as_deref()
            .is_some_and(|id| !safe_world_id(id))
    {
        return Err("PLUGIN_HANDOFF_CONTEXT_INVALID".into());
    }
    validate_waypoint(&request.payload)
}
fn validate_waypoint(payload: &Waypoint) -> Result<(), String> {
    if !safe_text(&payload.name, 320, false)
        || !safe_text(payload.source.as_deref().unwrap_or_default(), 480, true)
        || !(-WORLD_BORDER..=WORLD_BORDER).contains(&payload.x)
        || !(-WORLD_BORDER..=WORLD_BORDER).contains(&payload.z)
        || payload.y.is_some_and(|y| !(-4096..=4096).contains(&y))
    {
        return Err("PLUGIN_HANDOFF_PAYLOAD_INVALID".into());
    }
    if serde_json::to_vec(payload)
        .map_err(|_| "PLUGIN_HANDOFF_PAYLOAD_INVALID")?
        .len()
        > MAX_PAYLOAD_BYTES
    {
        return Err("PLUGIN_HANDOFF_PAYLOAD_INVALID".into());
    }
    Ok(())
}
pub(super) fn token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?;
    Ok(bytes.iter().map(|value| format!("{value:02x}")).collect())
}
pub fn send(
    app: &AppHandle,
    sender: &PluginManifest,
    bound_game_id: Option<&str>,
    sender_game: GameIdentity,
    receiver: &InstalledPlugin,
    request: SendRequest,
) -> Result<SendResult, String> {
    validate(&request, sender, bound_game_id, receiver)?;
    if sender_game.id != request.game_id || !safe_text(&sender_game.name, 320, false) {
        return Err("PLUGIN_HANDOFF_GAME_SCOPE".into());
    }
    let id = token()?;
    let pending = Pending {
        id: id.clone(),
        sender: PluginIdentity {
            plugin_id: sender.id.clone(),
            name: sender.name.clone(),
        },
        target_plugin_id: receiver.manifest.id.clone(),
        game: sender_game,
        world_id: request.world_id,
        payload: request.payload,
        expires: Instant::now() + TTL,
    };
    {
        let mut values = queue().lock().map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?;
        values.retain(|item| item.expires > Instant::now());
        if values.len() >= MAX_PENDING {
            return Err("PLUGIN_HANDOFF_BUSY".into());
        }
        values.push_back(pending);
    }
    if app
        .emit_to(
            "main",
            "plugin-handoff-request",
            RequestEvent {
                handoff_id: id.clone(),
                target_plugin_id: receiver.manifest.id.clone(),
                game_id: request.game_id,
            },
        )
        .is_err()
    {
        if let Ok(mut values) = queue().lock() {
            values.retain(|item| item.id != id);
        }
        return Err("PLUGIN_HANDOFF_UNAVAILABLE".into());
    }
    Ok(SendResult { requested: true })
}
/// Validate an opaque token at view-open time without consuming it.
pub fn authorize_view(
    target_plugin_id: &str,
    game_id: Option<&str>,
    handoff_id: Option<&str>,
) -> Result<(), String> {
    let Some(id) = handoff_id else {
        return Ok(());
    };
    let mut values = queue().lock().map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?;
    values.retain(|item| item.expires > Instant::now());
    let Some(pending) = values.iter().find(|item| item.id == id) else {
        drop(values);
        return super::plugin_material_handoffs::authorize_view(target_plugin_id, game_id, id);
    };
    if pending.target_plugin_id != target_plugin_id || Some(pending.game.id.as_str()) != game_id {
        return Err("PLUGIN_HANDOFF_VIEW_SCOPE".into());
    }
    Ok(())
}
/// Consume only the token explicitly bound to the current receiver view and game.
pub fn take(
    receiver: &PluginManifest,
    bound_game_id: Option<&str>,
    handoff_id: Option<&str>,
    interface_id: &str,
    version: &str,
) -> Result<Option<HandoffDraft>, String> {
    if interface_id != WAYPOINT_INTERFACE || version != WAYPOINT_VERSION {
        return Err("PLUGIN_HANDOFF_INTERFACE_UNSUPPORTED".into());
    }
    if !provides_waypoint(receiver) {
        return Err("PLUGIN_HANDOFF_RECEIVER_UNAVAILABLE".into());
    }
    let Some(id) = handoff_id else {
        return Ok(None);
    };
    let mut values = queue().lock().map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?;
    values.retain(|item| item.expires > Instant::now());
    let Some(index) = values.iter().position(|item| item.id == id) else {
        return Ok(None);
    };
    let pending = &values[index];
    if pending.target_plugin_id != receiver.id || bound_game_id != Some(pending.game.id.as_str()) {
        return Err("PLUGIN_HANDOFF_VIEW_SCOPE".into());
    }
    let pending = values.remove(index).ok_or("PLUGIN_HANDOFF_UNAVAILABLE")?;
    Ok(Some(HandoffDraft {
        handoff_id: pending.id,
        sender: pending.sender,
        game: pending.game,
        world_id: pending.world_id,
        payload: pending.payload,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_plugin_api::{Interfaces, PluginManifest};

    fn manifest(id: &str, declares_handoff: bool, provides_handoff: bool) -> PluginManifest {
        PluginManifest {
            tool_form: None,
            frontend: Some(cobble_plugin_api::FrontendManifest {
                api_version: "0.1.0".into(),
                entry: "ui/index.html".into(),
                assets_sha256: "0".repeat(64),
            }),
            permissions: None,
            id: id.into(),
            name: id.into(),
            description: None,
            tags: vec![],
            icon: None,
            version: "1.0.0".into(),
            api_version: "0.1.0".into(),
            publisher: "fixture".into(),
            capabilities: vec!["viewer".into()],
            composer_permissions: vec![],
            interfaces: Interfaces {
                provides: if provides_handoff {
                    vec![ProvidedInterface {
                        id: WAYPOINT_INTERFACE.into(),
                        version: WAYPOINT_VERSION.into(),
                    }]
                } else {
                    vec![]
                },
                requires: vec![],
                optional: if declares_handoff {
                    vec![InterfaceRequirement {
                        id: WAYPOINT_INTERFACE.into(),
                        version: WAYPOINT_VERSION.into(),
                    }]
                } else {
                    vec![]
                },
            },
            file_extensions: vec![],
            report_protocols: vec![],
            provider_origins: vec![],
            contributions: None,
            component_sha256: "0".repeat(64),
        }
    }

    fn send_request() -> SendRequest {
        SendRequest {
            target_plugin_id: "coordinate-book".into(),
            interface_id: WAYPOINT_INTERFACE.into(),
            version: WAYPOINT_VERSION.into(),
            game_id: "registered-game".into(),
            world_id: None,
            payload: Waypoint {
                name: "Village".into(),
                dimension: Dimension::Overworld,
                x: 0,
                y: None,
                z: 0,
                source: None,
            },
        }
    }

    #[test]
    fn send_allows_selected_game_when_unbound_and_rejects_bound_mismatch_or_undeclared_sender() {
        let sender = manifest("seed-compass", true, false);
        let receiver = InstalledPlugin {
            manifest: manifest("coordinate-book", false, true),
            enabled: true,
            problem: None,
        };
        assert!(validate(&send_request(), &sender, None, &receiver).is_ok());
        assert_eq!(
            validate(&send_request(), &sender, Some("another-game"), &receiver).unwrap_err(),
            "PLUGIN_HANDOFF_GAME_SCOPE"
        );
        let undeclared_sender = manifest("seed-compass", false, false);
        assert_eq!(
            validate(&send_request(), &undeclared_sender, None, &receiver).unwrap_err(),
            "PLUGIN_HANDOFF_RECEIVER_UNAVAILABLE"
        );
    }
    #[test]
    fn validates_waypoint_coordinate_and_text_bounds() {
        let okay = Waypoint {
            name: "Village".into(),
            dimension: Dimension::Overworld,
            x: WORLD_BORDER,
            y: Some(-64),
            z: -WORLD_BORDER,
            source: Some("Seed Compass".into()),
        };
        assert!(validate_waypoint(&okay).is_ok());
        assert!(
            validate_waypoint(&Waypoint {
                x: WORLD_BORDER + 1,
                ..okay.clone()
            })
            .is_err()
        );
        assert!(
            validate_waypoint(&Waypoint {
                y: Some(4097),
                ..okay.clone()
            })
            .is_err()
        );
        assert!(
            validate_waypoint(&Waypoint {
                name: "\n/path".into(),
                ..okay
            })
            .is_err()
        );
    }
    #[test]
    fn rejects_path_like_world_identifiers() {
        assert!(!safe_identifier("../world", 128));
        assert!(!safe_identifier("C:\\world", 128));
        assert!(safe_world_id("World One"));
        assert!(!safe_world_id("../world"));
    }
    fn pending(target: &str, game: &str, expires: Instant) -> Pending {
        Pending {
            id: "a".repeat(32),
            sender: PluginIdentity {
                plugin_id: "seed-compass".into(),
                name: "Seed Compass".into(),
            },
            target_plugin_id: target.into(),
            game: GameIdentity {
                id: game.into(),
                name: "Fixture".into(),
            },
            world_id: None,
            payload: Waypoint {
                name: "Village".into(),
                dimension: Dimension::Overworld,
                x: 0,
                y: None,
                z: 0,
                source: None,
            },
            expires,
        }
    }
    fn take_pending(
        values: &mut VecDeque<Pending>,
        receiver: &str,
        game: &str,
        id: &str,
        now: Instant,
    ) -> Result<Option<HandoffDraft>, String> {
        values.retain(|item| item.expires > now);
        let Some(index) = values.iter().position(|item| item.id == id) else {
            return Ok(None);
        };
        let item = &values[index];
        if item.target_plugin_id != receiver || item.game.id != game {
            return Err("PLUGIN_HANDOFF_VIEW_SCOPE".into());
        }
        let item = values.remove(index).ok_or("PLUGIN_HANDOFF_UNAVAILABLE")?;
        Ok(Some(HandoffDraft {
            handoff_id: item.id,
            sender: item.sender,
            game: item.game,
            world_id: item.world_id,
            payload: item.payload,
        }))
    }
    #[test]
    fn handoffs_are_target_and_game_bound_single_use_and_expiring() {
        let now = Instant::now();
        let mut values = VecDeque::from([pending("coordinate-book", "game-a", now + TTL)]);
        assert!(take_pending(&mut values, "other-plugin", "game-a", &"a".repeat(32), now).is_err());
        assert!(
            take_pending(
                &mut values,
                "coordinate-book",
                "game-b",
                &"a".repeat(32),
                now
            )
            .is_err()
        );
        let received = take_pending(
            &mut values,
            "coordinate-book",
            "game-a",
            &"a".repeat(32),
            now,
        )
        .unwrap();
        assert_eq!(received.unwrap().payload.name, "Village");
        assert!(
            take_pending(
                &mut values,
                "coordinate-book",
                "game-a",
                &"a".repeat(32),
                now
            )
            .unwrap()
            .is_none()
        );
        values.push_back(pending("coordinate-book", "game-a", now));
        assert!(
            take_pending(
                &mut values,
                "coordinate-book",
                "game-a",
                &"a".repeat(32),
                now
            )
            .unwrap()
            .is_none()
        );
    }
}
