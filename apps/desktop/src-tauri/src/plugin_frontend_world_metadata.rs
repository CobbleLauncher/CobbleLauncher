// SPDX-License-Identifier: MIT
//! Seed access supplements worlds inspection; returned facts come only from saved metadata.
use cobble_plugin_api::{InspectionScope, PermissionNode, PluginManifest};
use serde_json::Value;
use tauri::AppHandle;
pub fn permitted(manifest: &PluginManifest) -> Result<(), String> {
    let nodes = manifest.permissions.as_deref().unwrap_or_default();
    if nodes.contains(&PermissionNode::WorldSeedRead {}) && nodes.iter().any(|node|
        matches!(node,PermissionNode::GameInspectionRead {scopes} if scopes.contains(&InspectionScope::Worlds))) {
        Ok(())
    } else {Err("PLUGIN_PERMISSION_DENIED".into())}
}
pub async fn read(
    app: AppHandle,
    bound: Option<&str>,
    args: super::snapshots::Target,
) -> Result<Value, String> {
    super::snapshots::ids(bound, &args.game_id, &args.world_id)?;
    let metadata =
        crate::games::worlds::read_local_world_metadata(app, args.game_id, args.world_id).await?;
    serde_json::to_value(metadata).map_err(|_| "WORLD_METADATA_UNAVAILABLE".into())
}
