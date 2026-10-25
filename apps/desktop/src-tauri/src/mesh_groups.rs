// SPDX-License-Identifier: MIT
//! One verified running-game group on the existing owned companion and Mesh identity.
use super::*;
#[path = "mesh_group_commands.rs"]
pub mod commands;
#[path = "mesh_group_types.rs"]
mod types;
use types::{GroupSnapshot, WireGroup};
pub use types::{InvitationResult, JoinResult, View};

struct Binding {
    game: sessions::Session,
    name: String,
    id: u64,
    client: client::Client,
    app: AppHandle,
    wire: Option<WireGroup>,
    event: u64,
    pending_event: Option<(u64, WireGroup)>,
    busy: bool,
}
#[derive(Default)]
struct State {
    epoch: u64,
    next_id: u64,
    generation: u64,
    binding: Option<Binding>,
}
static GROUPS: OnceLock<Mutex<State>> = OnceLock::new();
fn state() -> &'static Mutex<State> {
    GROUPS.get_or_init(Default::default)
}
pub(super) fn environment_client(game_id: &str) -> Result<(client::Client, u64), String> {
    let state = state().lock().map_err(|_| "MESH_GROUP_UNAVAILABLE")?;
    let binding = state
        .binding
        .as_ref()
        .filter(|binding| binding.game.game_id == game_id && !binding.busy)
        .ok_or("MESH_GROUP_NOT_ACTIVE")?;
    let wire = binding
        .wire
        .as_ref()
        .filter(|wire| wire.role == "host" && wire.state == "hosting")
        .ok_or("MESH_GROUP_NOT_HOST")?;
    if !sessions::alive(&binding.game) {
        return Err("MESH_GROUP_GAME_EXITED".into());
    }
    Ok((binding.client.clone(), wire.group_id))
}
impl State {
    fn view(&self, game: Option<&str>) -> View {
        View {
            generation: self.generation,
            group: self
                .binding
                .as_ref()
                .filter(|binding| game.is_none_or(|id| id == binding.game.game_id))
                .and_then(|binding| {
                    binding.wire.as_ref().map(|wire| {
                        GroupSnapshot::new(binding.id, &binding.game, &binding.name, wire)
                    })
                }),
        }
    }
    fn occupied(&self) -> bool {
        self.binding.as_ref().is_some_and(|binding| {
            binding.wire.as_ref().is_none_or(WireGroup::active) || binding.busy
        })
    }
}
pub(super) fn occupied() -> bool {
    state().lock().map(|state| state.occupied()).unwrap_or(true)
}
fn publish(app: &AppHandle, view: &View) {
    let _ = app.emit("mesh-group-changed", view);
}
pub(super) fn clear() {
    let (app, view, previous) = {
        let Ok(mut state) = state().lock() else {
            return;
        };
        let app = state.binding.as_ref().map(|binding| binding.app.clone());
        let previous = state.binding.take();
        state.epoch = state.epoch.wrapping_add(1);
        state.generation = state.generation.wrapping_add(1);
        (app, state.view(None), previous)
    };
    // Last-client drop reaps worker threads; never do that under the cache lock.
    drop(previous);
    if let Some(app) = app {
        publish(&app, &view);
    }
}
fn event(app: &AppHandle, epoch: u64, value: Value) {
    let exit = value["event"] == "companion_exited";
    let decoded = if exit {
        None
    } else {
        let Some(generation) = value["generation"].as_u64() else {
            return;
        };
        let Ok(wire) = types::decode(&value) else {
            return;
        };
        Some((generation, wire))
    };
    let view = {
        let Ok(mut state) = state().lock() else {
            return;
        };
        if state.epoch != epoch {
            return;
        }
        let Some(binding) = state.binding.as_mut() else {
            return;
        };
        if exit {
            if let Some(wire) = binding.wire.as_mut() {
                wire.state = "failed".into();
                wire.members.clear();
                wire.admission_open = false;
                wire.invitation_expires_at = None;
                wire.bridge_address = None;
            }
            binding.busy = false;
        } else if let Some((generation, wire)) = decoded {
            if generation <= binding.event {
                return;
            }
            binding.event = generation;
            if let Some(current) = &binding.wire {
                if current.group_id != wire.group_id {
                    return;
                }
                if current.role != wire.role
                    || current.host_peer_id != wire.host_peer_id
                    || (!current.active() && wire.active())
                {
                    return;
                }
                if !wire.active() {
                    binding.busy = false;
                }
                binding.wire = Some(wire);
            } else {
                binding.pending_event = Some((generation, wire));
                return;
            }
        }
        state.generation = state.generation.wrapping_add(1);
        state.view(None)
    };
    publish(app, &view);
}
#[derive(Clone)]
struct Context {
    epoch: u64,
    id: u64,
    game: sessions::Session,
    client: client::Client,
    event: u64,
    owns_busy: bool,
}
fn reserve(app: &AppHandle, game: sessions::Session) -> Result<(Context, PathBuf), String> {
    let name = {
        let library = app.state::<GameLibrary>();
        let library = library.0.lock().map_err(|_| "MESH_GROUP_UNAVAILABLE")?;
        library
            .games
            .iter()
            .find(|value| value.id == game.game_id)
            .ok_or("MESH_GROUP_GAME_EXITED")?
            .name
            .clone()
    };
    let mut broker = broker().lock().map_err(|_| "MESH_GROUP_UNAVAILABLE")?;
    broker.prepare().map_err(group_error)?;
    let client = broker.shared_client()?;
    let directory = broker
        .temporary_identity
        .as_ref()
        .map(|root| root.path().join("identity"))
        .map(Ok)
        .unwrap_or_else(app::private_directory)?;
    let mut state = state().lock().map_err(|_| "MESH_GROUP_UNAVAILABLE")?;
    if state.occupied() {
        return Err("MESH_GROUP_ACTIVE".into());
    }
    state.next_id = state
        .next_id
        .checked_add(1)
        .filter(|id| *id <= 9_007_199_254_740_991)
        .ok_or("MESH_GROUP_LIMIT")?;
    state.epoch = state.epoch.wrapping_add(1);
    let context = Context {
        epoch: state.epoch,
        id: state.next_id,
        game: game.clone(),
        client: client.clone(),
        event: 0,
        owns_busy: true,
    };
    let previous = state.binding.replace(Binding {
        game,
        name,
        id: context.id,
        client: client.clone(),
        app: app.clone(),
        wire: None,
        event: 0,
        pending_event: None,
        busy: true,
    });
    let event_app = app.clone();
    let epoch = context.epoch;
    client.on_group_event(Arc::new(move |value| event(&event_app, epoch, value)));
    drop(state);
    drop(broker);
    drop(previous);
    Ok((context, directory))
}
fn finish(app: &AppHandle, context: &Context, mut wire: WireGroup) -> Result<View, String> {
    let view = {
        let mut state = state().lock().map_err(|_| "MESH_GROUP_UNAVAILABLE")?;
        if state.epoch != context.epoch {
            return Err("MESH_GROUP_NOT_ACTIVE".into());
        }
        let binding = state.binding.as_mut().ok_or("MESH_GROUP_NOT_ACTIVE")?;
        if let Some(current) = &binding.wire {
            if current.group_id != wire.group_id
                || current.role != wire.role
                || current.host_peer_id != wire.host_peer_id
            {
                return Err("MESH_GROUP_INVALID_RESPONSE".into());
            }
            if binding.event > context.event || (!current.active() && wire.active()) {
                wire = current.clone();
            }
        } else if let Some((_, pending)) = binding
            .pending_event
            .take()
            .filter(|(_, pending)| pending.group_id == wire.group_id)
        {
            wire = pending;
        }
        let terminal = !wire.active();
        if context.owns_busy || terminal {
            binding.busy = false;
        }
        if binding.wire.as_ref() == Some(&wire) {
            return Ok(state.view(None));
        }
        binding.wire = Some(wire);
        state.generation = state.generation.wrapping_add(1);
        if terminal {
            state.epoch = state.epoch.wrapping_add(1);
        }
        state.view(None)
    };
    publish(app, &view);
    Ok(view)
}
fn failed(context: &Context) {
    let mut state = match state().lock() {
        Ok(state) => state,
        Err(_) => return,
    };
    if state.epoch != context.epoch {
        return;
    }
    if let Some(binding) = state.binding.as_mut() {
        binding.busy = false;
        if binding.wire.is_none() {
            state.binding = None;
        }
    }
}
fn request(context: &Context, command: Value) -> Result<Value, String> {
    if !enabled::current() {
        return Err("MESH_DISABLED".into());
    }
    context.client.request(command).map_err(group_error)
}
fn group_error(error: String) -> String {
    match error.as_str() {
        "MESH_DIRECT_PATH_REQUIRED" | "MESH_OFFICIAL_COORDINATION_ONLY" => {
            "MESH_GROUP_DIRECT_REQUIRED".into()
        }
        "已有联机会话正在进行" => "MESH_GROUP_ACTIVE".into(),
        "邀请无效或已过期" => "MESH_GROUP_INVALID_INVITATION".into(),
        "中继地址无效或不支持" => "MESH_GROUP_INVALID_RELAY".into(),
        "联机连接超时" => "MESH_GROUP_TIMEOUT".into(),
        "联机会话已结束" => "MESH_GROUP_NOT_ACTIVE".into(),
        _ if error.starts_with("MESH_GROUP_") || error == "MESH_DISABLED" => error,
        _ => "MESH_GROUP_UNAVAILABLE".into(),
    }
}
fn action(game_id: &str, id: u64, host_only: bool) -> Result<(Context, u64), String> {
    let mut state = state().lock().map_err(|_| "MESH_GROUP_UNAVAILABLE")?;
    let epoch = state.epoch;
    let binding = state
        .binding
        .as_mut()
        .filter(|binding| binding.game.game_id == game_id && binding.id == id)
        .ok_or("MESH_GROUP_NOT_ACTIVE")?;
    let wire = binding
        .wire
        .as_ref()
        .filter(|wire| wire.active())
        .ok_or("MESH_GROUP_NOT_ACTIVE")?;
    if binding.busy {
        return Err("MESH_GROUP_BUSY".into());
    }
    if host_only && wire.role != "host" {
        return Err("MESH_GROUP_NOT_HOST".into());
    }
    binding.busy = true;
    Ok((
        Context {
            epoch,
            id,
            game: binding.game.clone(),
            client: binding.client.clone(),
            event: binding.event,
            owns_busy: true,
        },
        wire.group_id,
    ))
}
fn watch(app: AppHandle, context: Context) {
    std::thread::spawn(move || {
        let _restore = app_route::RestoreOnExit;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let current = {
                let Ok(state) = state().lock() else { return };
                if state.epoch != context.epoch {
                    return;
                }
                let Some(binding) = state.binding.as_ref() else {
                    return;
                };
                let Some(wire) = binding.wire.as_ref().filter(|wire| wire.active()) else {
                    return;
                };
                (
                    Context {
                        event: binding.event,
                        owns_busy: false,
                        ..context.clone()
                    },
                    wire.group_id,
                    binding.busy,
                )
            };
            if !sessions::alive(&context.game) {
                let result = current
                    .0
                    .client
                    .request(json!({"op":"group_stop","group_id":current.1}));
                if let Ok(value) = result
                    && let Ok(wire) = types::decode(&value)
                    && wire.state == "ended"
                {
                    let _ = finish(&app, &current.0, wire);
                } else {
                    current.0.client.shutdown();
                }
                return;
            }
            if current.2 {
                continue;
            }
            match request(&current.0, json!({"op":"group_status"})).and_then(|value| {
                if value.get("group") == Some(&Value::Null) {
                    let state = state().lock().map_err(|_| "MESH_GROUP_UNAVAILABLE")?;
                    if state.epoch != current.0.epoch {
                        return Err("MESH_GROUP_NOT_ACTIVE".into());
                    }
                    let mut wire = state
                        .binding
                        .as_ref()
                        .and_then(|binding| binding.wire.clone())
                        .ok_or("MESH_GROUP_NOT_ACTIVE")?;
                    wire.state = "ended".into();
                    wire.members.clear();
                    wire.admission_open = false;
                    wire.invitation_expires_at = None;
                    wire.bridge_address = None;
                    Ok(wire)
                } else {
                    types::decode(&value)
                }
            }) {
                Ok(wire) => {
                    let _ = finish(&app, &current.0, wire);
                }
                Err(error) => {
                    if matches!(
                        error.as_str(),
                        "MESH_GROUP_IO_FAILED" | "MESH_GROUP_INVALID_RESPONSE"
                    ) {
                        current.0.client.shutdown();
                    }
                    event(&app, context.epoch, json!({"event":"companion_exited"}));
                    return;
                }
            }
        }
    });
}
