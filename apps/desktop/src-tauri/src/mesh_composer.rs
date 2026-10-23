// SPDX-License-Identifier: MIT
//! User-selected public snapshots for pure drafts. Sending remains a separate action.
use super::*;
use cobble_plugin_api::{
    ComposerPermission, MessageComposerContext, MessageComposerRequest, MessageDraft, PeerSource,
    PublicIdentity, SelectedPeer, SelectedSession,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
static ACTIVE: OnceLock<Mutex<Option<Arc<AtomicBool>>>> = OnceLock::new();
fn active() -> &'static Mutex<Option<Arc<AtomicBool>>> {
    ACTIVE.get_or_init(Default::default)
}
pub(super) fn cancel() -> bool {
    if let Ok(slot) = active().lock()
        && let Some(flag) = slot.as_ref()
    {
        flag.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}
struct Worker(Arc<AtomicBool>);
impl Drop for Worker {
    fn drop(&mut self) {
        if let Ok(mut slot) = active().lock()
            && slot.as_ref().is_some_and(|flag| Arc::ptr_eq(flag, &self.0))
        {
            slot.take();
        }
    }
}
fn start() -> Result<Worker, String> {
    let mut slot = active().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
    if slot.is_some() {
        return Err("MESH_COMPOSER_BUSY".into());
    }
    let flag = Arc::new(AtomicBool::new(false));
    *slot = Some(flag.clone());
    Ok(Worker(flag))
}
fn social(peer_id: &str) -> Result<(u64, app::PublicProfile, app::PublicProfile), String> {
    let mut broker = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
    let current = app::snapshot(&mut broker)?.ok_or("MESH_APP_STOPPED")?;
    let peer = current
        .contacts
        .iter()
        .find(|contact| contact.profile.peer_id == peer_id)
        .ok_or("MESH_UNKNOWN_CONTACT")?;
    Ok((
        broker.social_generation,
        current.profile,
        peer.profile.clone(),
    ))
}
fn session(app: &AppHandle, id: Option<&str>) -> Result<Option<SelectedSession>, String> {
    let Some(id) = id else { return Ok(None) };
    let library = app.state::<GameLibrary>();
    let library = library.0.lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
    let game = library
        .games
        .iter()
        .find(|game| game.id == id)
        .ok_or("MESH_UNKNOWN_GAME")?;
    Ok(Some(SelectedSession {
        game_name: game.name.clone(),
        game_version: game.version.clone(),
        loader: game
            .loader_kind
            .as_ref()
            .map(|kind| match &game.loader_version {
                Some(version) => format!("{kind} {version}"),
                None => kind.clone(),
            }),
    }))
}
#[tauri::command]
pub fn mesh_list_message_composers()
-> Result<Vec<crate::plugins::plugin_composers::ComposerChoice>, String> {
    crate::plugins::plugin_composers::choices()
}
#[tauri::command]
pub fn mesh_cancel_message_composer() -> bool {
    cancel()
}
#[tauri::command]
pub async fn mesh_compose_message(
    app: AppHandle,
    plugin_id: String,
    peer_id: String,
    game_id: Option<String>,
    note: String,
    locale: String,
) -> Result<MessageDraft, String> {
    let activity = MeshActivity::new("message_compose");
    let worker = start()?;
    let plugin = crate::plugins::plugin_composers::selected(&plugin_id)?;
    let scopes = &plugin.manifest.composer_permissions;
    let peer = peer_id.clone();
    let identity = tauri::async_runtime::spawn_blocking(move || social(&peer))
        .await
        .map_err(|_| "MESH_OPERATION_FAILED")??;
    let selected_session = if scopes.contains(&ComposerPermission::SelectedSession) {
        session(&app, game_id.as_deref())?
    } else {
        None
    };
    let request = MessageComposerRequest {
        note,
        context: MessageComposerContext {
            locale,
            public_self: scopes
                .contains(&ComposerPermission::PublicIdentity)
                .then(|| PublicIdentity {
                    id: identity.1.peer_id.clone(),
                    display_name: Some(identity.1.display_name.clone()),
                }),
            selected_peer: scopes.contains(&ComposerPermission::SelectedPeer).then(|| {
                SelectedPeer {
                    identity: PublicIdentity {
                        id: identity.2.peer_id.clone(),
                        display_name: Some(identity.2.display_name.clone()),
                    },
                    source: PeerSource::Contact,
                }
            }),
            selected_session: selected_session.clone(),
        },
    };
    let result =
        crate::plugins::plugin_composers::compose(plugin_id, request, worker.0.clone()).await;
    let result = match result {
        Ok(draft) => {
            let after = tauri::async_runtime::spawn_blocking(move || social(&peer_id))
                .await
                .map_err(|_| "MESH_OPERATION_FAILED")??;
            if after != identity
                || worker.0.load(Ordering::SeqCst)
                || selected_session.is_some()
                    && session(&app, game_id.as_deref())? != selected_session
            {
                Err("MESH_CONTEXT_CHANGED".into())
            } else {
                Ok(draft)
            }
        }
        Err(error) => Err(error),
    };
    activity.complete(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn composer_cancel_keeps_worker_owned_until_completion() {
        let worker = start().unwrap();
        assert!(start().is_err());
        assert!(cancel());
        assert!(worker.0.load(Ordering::SeqCst));
        assert!(start().is_err());
        drop(worker);
        let next = start().unwrap();
        assert!(!next.0.load(Ordering::SeqCst));
        drop(next);
        assert!(!cancel());
    }
}
