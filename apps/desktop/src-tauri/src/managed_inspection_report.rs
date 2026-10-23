// SPDX-License-Identifier: MIT
//! Bounded read-only inspection snapshots and real context-aware Wasm reports.
use super::*;
use cobble_plugin_api::{
    InspectionGame, InspectionScope, InspectionSnapshot, MAX_INSPECTION_BYTES, PermissionNode,
};
pub(crate) fn permitted(manifest: &PluginManifest, scope: InspectionScope) -> Result<(), String> {
    if manifest.permissions.as_deref().unwrap_or_default().iter().any(|node|matches!(node,PermissionNode::GameInspectionRead {scopes} if scopes.contains(&scope))) {Ok(())}else{Err("PLUGIN_PERMISSION_DENIED".into())}
}
fn envelope(
    game: InspectionGame,
    scope: InspectionScope,
    data: serde_json::Value,
) -> Result<InspectionSnapshot, String> {
    let mut snapshot = InspectionSnapshot {
        schema_version: 1,
        scope,
        game,
        data,
        revision: String::new(),
    };
    let mut stable = snapshot.clone();
    if let Some(object) = stable.data.as_object_mut() {
        object.remove("inspectedAt");
    }
    let bytes = serde_json::to_vec(&stable).map_err(|_| "INSPECTION_FAILED")?;
    let actual = serde_json::to_vec(&snapshot).map_err(|_| "INSPECTION_FAILED")?;
    if actual.len() > MAX_INSPECTION_BYTES - 64 {
        return Err("INSPECTION_LIMIT".into());
    }
    snapshot.revision = cobble_plugin_host::digest(&bytes);
    Ok(snapshot)
}
pub(crate) async fn snapshot(
    app: &AppHandle,
    game_id: &str,
    scope: InspectionScope,
) -> Result<InspectionSnapshot, String> {
    let started = std::time::Instant::now();
    crate::games::ensure_registered_game(app, game_id)?;
    let game = crate::games::frontend_game_summaries(app)?
        .into_iter()
        .find(|game| game["id"] == game_id)
        .ok_or("INSPECTION_GAME_UNAVAILABLE")?;
    let game: InspectionGame = serde_json::from_value(game).map_err(|_| "INSPECTION_FAILED")?;
    let data = crate::games::game_inspection::data_snapshot(app, game_id, scope.as_str()).await?;
    crate::games::ensure_registered_game(app, game_id)?;
    let result = envelope(game, scope, data);
    tracing::debug!(
        event = "plugin_inspection_snapshot",
        scope = scope.as_str(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        success = result.is_ok()
    );
    result
}
pub(crate) async fn run_for_frontend(
    app: AppHandle,
    game_id: String,
    scope: InspectionScope,
    revision: String,
    manifest: PluginManifest,
    locale: String,
    cancel: Arc<AtomicBool>,
) -> Result<Option<Report>, String> {
    permitted(&manifest, scope)?;
    if revision.len() != 64 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("INSPECTION_CHANGED".into());
    }
    {
        let mut state = state().lock().map_err(|_| "PLUGIN_REPORT_BUSY")?;
        if state.active.is_some() {
            return Err("PLUGIN_REPORT_BUSY".into());
        }
        state.active = Some(Analysis {
            plugin_id: manifest.id.clone(),
            cancel: cancel.clone(),
        });
    }
    let result = async {
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        let current = snapshot(&app, &game_id, scope).await?;
        if current.revision != revision {
            return Err("INSPECTION_CHANGED".into());
        }
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        let (plugin, component) = tauri::async_runtime::spawn_blocking(move || {
            let plugin = selected_plugin(&manifest.id)?;
            if plugin.manifest != manifest {
                return Err("PLUGIN_VIEW_STALE".into());
            }
            permitted(&plugin.manifest, scope)?;
            let component = package(&plugin)?;
            Ok::<_, String>((plugin, component))
        })
        .await
        .map_err(|_| "INSPECTION_FAILED")??;
        let bytes = serde_json::to_vec(&current).map_err(|_| "INSPECTION_FAILED")?;
        execute_report(
            plugin,
            component,
            bytes,
            "application/json",
            cancel.clone(),
            locale,
            Some(game_id),
        )
        .await
    }
    .await;
    state()
        .lock()
        .map_err(|_| "PLUGIN_REPORT_BUSY")?
        .finish(&cancel, result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> InspectionGame {
        InspectionGame {
            id: "public".into(),
            name: "Fixture".into(),
            version: "1.21.1".into(),
        }
    }
    #[test]
    fn immutable_revision_covers_scope_game_and_data_with_bounded_envelope() {
        let first = envelope(
            game(),
            InspectionScope::Mods,
            serde_json::json!({"items":[],"truncated":false}),
        )
        .unwrap();
        let same = envelope(game(), InspectionScope::Mods, first.data.clone()).unwrap();
        assert_eq!(first.revision, same.revision);
        assert_ne!(
            first.revision,
            envelope(game(), InspectionScope::Packs, first.data.clone())
                .unwrap()
                .revision
        );
        assert_ne!(
            first.revision,
            envelope(
                game(),
                InspectionScope::Mods,
                serde_json::json!({"items":[{"name":"changed"}]})
            )
            .unwrap()
            .revision
        );
        assert_eq!(
            envelope(
                game(),
                InspectionScope::Backups,
                serde_json::json!({"items":[],"inspectedAt":1})
            )
            .unwrap()
            .revision,
            envelope(
                game(),
                InspectionScope::Backups,
                serde_json::json!({"items":[],"inspectedAt":2})
            )
            .unwrap()
            .revision
        );
        assert!(
            envelope(
                game(),
                InspectionScope::Mods,
                serde_json::json!({"text":"x".repeat(MAX_INSPECTION_BYTES)})
            )
            .is_err()
        );
    }
}
