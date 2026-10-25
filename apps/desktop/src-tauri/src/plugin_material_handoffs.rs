// SPDX-License-Identifier: MIT
//! Full, bounded material requirements. Delivery uses the same one-shot view binding as waypoints.
use super::plugin_handoffs::{GameIdentity, PluginIdentity, Receiver, SendResult};
use cobble_plugin_api::{InstalledPlugin, PluginManifest, ProvidedInterface};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
pub const INTERFACE: &str = "launcher:materials-import";
pub const VERSION: &str = "0.1.0";
const MAX_BYTES: usize = 1_048_576;
const TTL: Duration = Duration::from_secs(300);
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Material {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub required: u64,
    pub stack_size: Option<u16>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Payload {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub materials: Vec<Material>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendRequest {
    pub target_plugin_id: String,
    pub interface_id: String,
    pub version: String,
    pub game_id: Option<String>,
    pub world_id: Option<String>,
    pub payload: Payload,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    handoff_id: String,
    sender: PluginIdentity,
    game: Option<GameIdentity>,
    world_id: Option<String>,
    payload: Payload,
}
struct Pending {
    id: String,
    target: String,
    sender: PluginIdentity,
    game: Option<GameIdentity>,
    world: Option<String>,
    payload: Payload,
    expires: Instant,
}
static QUEUE: OnceLock<Mutex<VecDeque<Pending>>> = OnceLock::new();
fn queue() -> &'static Mutex<VecDeque<Pending>> {
    QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}
pub fn declared(manifest: &PluginManifest) -> bool {
    let provided = ProvidedInterface {
        id: INTERFACE.into(),
        version: VERSION.into(),
    };
    manifest
        .interfaces
        .optional
        .iter()
        .chain(&manifest.interfaces.requires)
        .any(|r| r.matches(&provided))
}
fn provides(manifest: &PluginManifest) -> bool {
    manifest
        .interfaces
        .provides
        .iter()
        .any(|r| r.id == INTERFACE && r.version == VERSION)
}
fn eligible(plugin: &InstalledPlugin) -> bool {
    plugin.enabled
        && plugin.problem.is_none()
        && plugin.manifest.frontend.is_some()
        && provides(&plugin.manifest)
}
pub fn receivers(
    sender: &PluginManifest,
    plugins: &[InstalledPlugin],
) -> Result<Vec<Receiver>, String> {
    if !declared(sender) {
        return Err("PLUGIN_HANDOFF_INTERFACE_NOT_DECLARED".into());
    }
    Ok(plugins
        .iter()
        .filter(|p| p.manifest.id != sender.id && eligible(p))
        .take(64)
        .map(|p| Receiver {
            plugin_id: p.manifest.id.clone(),
            name: p.manifest.name.clone(),
        })
        .collect())
}
fn text(value: &str, bytes: usize, chars: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= bytes
        && value.chars().count() <= chars
        && !value.chars().any(char::is_control)
}
pub fn validate(payload: &Payload) -> Result<(), String> {
    let mut ids = HashSet::new();
    if !text(&payload.name, 320, 80)
        || payload
            .source
            .as_deref()
            .is_some_and(|v| !text(v, 480, 120))
        || payload.materials.is_empty()
        || payload.materials.len() > 2048
        || payload.materials.iter().any(|m| {
            !text(&m.id, 1024, 1024)
                || !ids.insert(&m.id)
                || m.name.as_deref().is_some_and(|n| !text(n, 320, 80))
                || m.required == 0
                || m.required > 1_000_000_000
                || m.stack_size.is_some_and(|s| !(1..=999).contains(&s))
        })
        || serde_json::to_vec(payload)
            .map_err(|_| "PLUGIN_HANDOFF_PAYLOAD_INVALID")?
            .len()
            > MAX_BYTES
    {
        return Err("PLUGIN_HANDOFF_PAYLOAD_INVALID".into());
    }
    Ok(())
}
pub fn send(
    app: &AppHandle,
    sender: &PluginManifest,
    bound: Option<&str>,
    game: Option<GameIdentity>,
    receiver: &InstalledPlugin,
    request: SendRequest,
) -> Result<SendResult, String> {
    if request.interface_id != INTERFACE || request.version != VERSION {
        return Err("PLUGIN_HANDOFF_INTERFACE_UNSUPPORTED".into());
    }
    if !declared(sender)
        || !eligible(receiver)
        || receiver.manifest.id != request.target_plugin_id
        || sender.id == receiver.manifest.id
    {
        return Err("PLUGIN_HANDOFF_RECEIVER_UNAVAILABLE".into());
    }
    if bound.is_some_and(|id| Some(id) != request.game_id.as_deref())
        || game.as_ref().map(|g| g.id.as_str()) != request.game_id.as_deref()
        || (request.world_id.is_some() && game.is_none())
    {
        return Err("PLUGIN_HANDOFF_GAME_SCOPE".into());
    }
    validate(&request.payload)?;
    let id = super::plugin_handoffs::token()?;
    let mut values = queue().lock().map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?;
    values.retain(|v| v.expires > Instant::now());
    if values.len() >= 8 {
        return Err("PLUGIN_HANDOFF_BUSY".into());
    }
    values.push_back(Pending {
        id: id.clone(),
        target: request.target_plugin_id.clone(),
        sender: PluginIdentity {
            plugin_id: sender.id.clone(),
            name: sender.name.clone(),
        },
        game,
        world: request.world_id,
        payload: request.payload,
        expires: Instant::now() + TTL,
    });
    drop(values);
    if app.emit_to("main","plugin-handoff-request",serde_json::json!({"handoffId":id,"targetPluginId":request.target_plugin_id,"gameId":request.game_id,"interfaceId":INTERFACE,"version":VERSION})).is_err(){if let Ok(mut values)=queue().lock(){values.retain(|v|v.id!=id)} return Err("PLUGIN_HANDOFF_UNAVAILABLE".into())}
    Ok(SendResult { requested: true })
}
pub fn authorize_view(target: &str, game: Option<&str>, id: &str) -> Result<(), String> {
    let mut values = queue().lock().map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?;
    values.retain(|v| v.expires > Instant::now());
    let pending = values
        .iter()
        .find(|v| v.id == id)
        .ok_or("PLUGIN_HANDOFF_NOT_FOUND")?;
    if pending.target != target || pending.game.as_ref().map(|g| g.id.as_str()) != game {
        return Err("PLUGIN_HANDOFF_VIEW_SCOPE".into());
    }
    Ok(())
}
fn take_pending(
    values: &mut VecDeque<Pending>,
    target: &str,
    game: Option<&str>,
    id: &str,
    now: Instant,
) -> Result<Option<Draft>, String> {
    values.retain(|v| v.expires > now);
    let Some(index) = values.iter().position(|v| v.id == id) else {
        return Ok(None);
    };
    let pending = &values[index];
    if pending.target != target || pending.game.as_ref().map(|g| g.id.as_str()) != game {
        return Err("PLUGIN_HANDOFF_VIEW_SCOPE".into());
    }
    let p = values.remove(index).ok_or("PLUGIN_HANDOFF_UNAVAILABLE")?;
    Ok(Some(Draft {
        handoff_id: p.id,
        sender: p.sender,
        game: p.game,
        world_id: p.world,
        payload: p.payload,
    }))
}
pub fn take(
    receiver: &PluginManifest,
    game: Option<&str>,
    id: Option<&str>,
) -> Result<Option<Draft>, String> {
    if !provides(receiver) {
        return Err("PLUGIN_HANDOFF_RECEIVER_UNAVAILABLE".into());
    }
    let Some(id) = id else { return Ok(None) };
    take_pending(
        &mut *queue().lock().map_err(|_| "PLUGIN_HANDOFF_UNAVAILABLE")?,
        &receiver.id,
        game,
        id,
        Instant::now(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn thousand() -> Payload {
        Payload {
            name: "Fixture".into(),
            source: None,
            materials: (0..1000)
                .map(|i| Material {
                    id: format!("fixture:block_{i}"),
                    name: None,
                    required: 1,
                    stack_size: None,
                })
                .collect(),
        }
    }
    #[test]
    fn full_thousand_materials_survive_without_guessed_stacks() {
        let p = thousand();
        validate(&p).unwrap();
        let restored: Payload = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        assert_eq!(restored.materials.len(), 1000);
        assert!(restored.materials.iter().all(|m| m.stack_size.is_none()));
    }
    #[test]
    fn rejects_duplicate_and_out_of_bounds_requirements() {
        let mut p = thousand();
        p.materials[1].id = p.materials[0].id.clone();
        assert!(validate(&p).is_err());
        let mut p = thousand();
        p.materials[0].required = 0;
        assert!(validate(&p).is_err());
        let mut p = thousand();
        p.materials[0].stack_size = Some(1000);
        assert!(validate(&p).is_err());
        let mut p = thousand();
        p.materials.extend(thousand().materials);
        p.materials.extend(thousand().materials);
        assert!(validate(&p).is_err());
    }
    #[test]
    fn rejects_encoded_payload_over_one_megabyte_without_dropping_rows() {
        let mut p = thousand();
        for (i, m) in p.materials.iter_mut().enumerate() {
            m.id = format!("{i:04}{}", "x".repeat(1020));
        }
        assert!(serde_json::to_vec(&p).unwrap().len() > MAX_BYTES);
        assert!(validate(&p).is_err());
        assert_eq!(p.materials.len(), 1000);
    }
    #[test]
    fn general_materials_are_target_bound_one_shot_and_expiring() {
        let now = Instant::now();
        let pending = |expires| Pending {
            id: "a".repeat(32),
            target: "material-planner".into(),
            sender: PluginIdentity {
                plugin_id: "schematic-viewer".into(),
                name: "Viewer".into(),
            },
            game: None,
            world: None,
            payload: thousand(),
            expires,
        };
        let mut values = VecDeque::from([pending(now + TTL)]);
        assert!(take_pending(&mut values, "another-plugin", None, &"a".repeat(32), now).is_err());
        assert!(
            take_pending(
                &mut values,
                "material-planner",
                Some("first-game"),
                &"a".repeat(32),
                now
            )
            .is_err()
        );
        let draft = take_pending(&mut values, "material-planner", None, &"a".repeat(32), now)
            .unwrap()
            .unwrap();
        assert!(draft.game.is_none());
        assert_eq!(draft.payload.materials.len(), 1000);
        assert!(
            take_pending(&mut values, "material-planner", None, &"a".repeat(32), now)
                .unwrap()
                .is_none()
        );
        values.push_back(pending(now));
        assert!(
            take_pending(&mut values, "material-planner", None, &"a".repeat(32), now)
                .unwrap()
                .is_none()
        );
    }
}
