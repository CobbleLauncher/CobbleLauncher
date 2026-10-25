// SPDX-License-Identifier: MIT
//! Bounded, game-scoped snapshot bridge. Restore requests only open the host confirmation flow.
use cobble_plugin_api::{InspectionScope, PermissionNode, PluginManifest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub game_id: String,
    pub world_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Compare {
    pub game_id: String,
    pub world_id: String,
    pub before_id: String,
    pub after_id: String,
    pub offset: usize,
    pub limit: usize,
    #[serde(default)]
    pub query: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreRequest {
    pub game_id: String,
    pub world_id: String,
    pub snapshot_id: String,
}

pub fn permitted(manifest: &PluginManifest, restore: bool) -> Result<(), String> {
    let nodes = manifest.permissions.as_deref().unwrap_or_default();
    if nodes.contains(&PermissionNode::WorldSnapshotsRead {})
        && nodes.iter().any(|node| matches!(node, PermissionNode::GameInspectionRead { scopes } if scopes.contains(&InspectionScope::Worlds)))
        && (!restore || nodes.contains(&PermissionNode::WorldSnapshotsRestoreRequest {}))
    { Ok(()) } else { Err("PLUGIN_PERMISSION_DENIED".into()) }
}
pub fn ids(bound: Option<&str>, game_id: &str, world_id: &str) -> Result<(), String> {
    super::target(bound, game_id)?;
    if world_id.is_empty()
        || world_id.len() > 128
        || world_id.chars().any(char::is_control)
        || world_id == "."
        || world_id == ".."
        || world_id.contains('/')
        || world_id.contains('\\')
    {
        return Err("PLUGIN_WORLD_SCOPE".into());
    }
    Ok(())
}
pub fn compare_bounds(args: &Compare) -> Result<(), String> {
    if args.limit == 0
        || args.limit > 100
        || args.offset > 200_000
        || args.query.chars().count() > 120
        || args.query.chars().any(char::is_control)
    {
        Err("WORLD_SNAPSHOT_RANGE".into())
    } else {
        Ok(())
    }
}
fn bounded_value<T: Serialize>(value: T) -> Result<Value, String> {
    let bytes = serde_json::to_vec(&value).map_err(|_| "PLUGIN_FRONTEND_FAILED")?;
    if bytes.len() > 1024 * 1024 {
        return Err("WORLD_SNAPSHOT_LIMIT".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
}
pub async fn list(app: AppHandle, bound: Option<&str>, args: Target) -> Result<Value, String> {
    ids(bound, &args.game_id, &args.world_id)?;
    let snapshots =
        crate::games::world_snapshots::list_world_snapshots(app, args.game_id, args.world_id)
            .await
            .map_err(|_| "WORLD_SNAPSHOT_UNAVAILABLE")?;
    if snapshots.len() > 512 {
        return Err("WORLD_SNAPSHOT_LIMIT".into());
    }
    bounded_value(snapshots)
}
pub async fn compare(app: AppHandle, bound: Option<&str>, args: Compare) -> Result<Value, String> {
    ids(bound, &args.game_id, &args.world_id)?;
    compare_bounds(&args)?;
    let page = crate::games::world_snapshots::insights::compare_world_snapshots_filtered(
        app,
        crate::games::world_snapshots::insights::FilteredComparison {
            game_id: args.game_id,
            folder_name: args.world_id,
            before_id: args.before_id,
            after_id: args.after_id,
            offset: args.offset,
            limit: args.limit,
            query: args.query,
        },
    )
    .await
    .map_err(|_| "WORLD_SNAPSHOT_COMPARE_FAILED")?;
    bounded_value(page)
}
pub async fn open_restore(
    app: AppHandle,
    bound: Option<&str>,
    plugin_id: &str,
    args: RestoreRequest,
    cancelled: &AtomicBool,
) -> Result<Value, String> {
    ids(bound, &args.game_id, &args.world_id)?;
    if args.snapshot_id.len() != 64
        || !args
            .snapshot_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("WORLD_SNAPSHOT_NOT_FOUND".into());
    }
    let snapshots = crate::games::world_snapshots::list_world_snapshots(
        app.clone(),
        args.game_id.clone(),
        args.world_id.clone(),
    )
    .await
    .map_err(|_| "WORLD_SNAPSHOT_UNAVAILABLE")?;
    if !snapshots
        .iter()
        .any(|snapshot| snapshot.id == args.snapshot_id)
    {
        return Err("WORLD_SNAPSHOT_NOT_FOUND".into());
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_CLOSED".into());
    }
    app.emit_to("main", "plugin-open-snapshot-restore", json!({"game_id":args.game_id,"world_id":args.world_id,"snapshot_id":args.snapshot_id,"plugin_id":plugin_id})).map_err(|_| "PLUGIN_FRONTEND_FAILED")?;
    Ok(json!({"requested":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_permission_scope_and_bounded_payloads() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/world-dashboard/plugin.json"
        ))
        .unwrap();
        manifest.permissions.as_mut().unwrap().retain(|node| {
            !matches!(
                node,
                PermissionNode::WorldSnapshotsRead {}
                    | PermissionNode::WorldSnapshotsRestoreRequest {}
            )
        });
        assert!(permitted(&manifest, false).is_err());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::WorldSnapshotsRead {});
        assert!(permitted(&manifest, false).is_ok());
        assert!(permitted(&manifest, true).is_err());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::WorldSnapshotsRestoreRequest {});
        assert!(permitted(&manifest, true).is_ok());
        assert!(ids(Some("game"), "other", "world").is_err());
        assert!(ids(Some("game"), "game", "../world").is_err());
        let args: Compare = serde_json::from_value(json!({"gameId":"game","worldId":"world","beforeId":"a","afterId":"b","offset":0,"limit":101})).unwrap();
        assert!(compare_bounds(&args).is_err());
        assert_eq!(
            bounded_value(json!({"ok":true})).unwrap(),
            json!({"ok":true})
        );
        assert!(bounded_value(json!({"huge":"x".repeat(1024*1024)})).is_err());
        assert!(
            serde_json::from_value::<RestoreRequest>(
                json!({"gameId":"game","worldId":"world","snapshotId":"abc","replaceExisting":true})
            )
            .is_err()
        );
    }
}
