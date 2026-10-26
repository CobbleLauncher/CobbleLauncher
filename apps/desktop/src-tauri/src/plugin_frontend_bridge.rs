// SPDX-License-Identifier: MIT
//! Typed plugin frontend broker. Caller identity is supplied only by native WebView state.
use cobble_plugin_api::{InspectionScope, PermissionNode, PluginManifest};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Webview};
use tauri_plugin_clipboard_manager::ClipboardExt;
#[path = "plugin_frontend_snapshots.rs"]
mod snapshots;
#[path = "plugin_frontend_world_metadata.rs"]
mod world_metadata;
#[derive(Deserialize)]
#[serde(tag = "method", content = "params", deny_unknown_fields)]
pub enum FrontendRequest {
    #[serde(rename = "mesh.subscribe")]
    MeshSubscribe(super::plugin_mesh_events::Subscribe),
    #[serde(rename = "mesh.receive")]
    MeshReceive(super::plugin_mesh_events::Receive),
    #[serde(rename = "mesh.unsubscribe")]
    MeshUnsubscribe(super::plugin_mesh_events::Subscription),
    #[serde(rename = "mesh.send")]
    MeshSend(super::plugin_mesh_events::Send),
    #[serde(rename = "mesh.peers")]
    MeshPeers(Empty),
    #[serde(rename = "generatedBlueprint.save")]
    GeneratedSave(super::plugin_generated_blueprint::SaveRequest),
    #[serde(rename = "generatedBlueprint.open")]
    GeneratedOpen(super::plugin_generated_blueprint::OpenRequest),
    #[serde(rename = "ai.status")]
    AiStatus(Empty),
    #[serde(rename = "ai.configure")]
    AiConfigure(super::plugin_ai::Configure),
    #[serde(rename = "ai.clear")]
    AiClear(Empty),
    #[serde(rename = "ai.chat")]
    AiChat(super::plugin_ai::Chat),
    #[serde(rename = "ai.cancel")]
    AiCancel(super::plugin_ai::Cancel),
    #[serde(rename = "trialCopy.preview")]
    TrialPreview(crate::games::trial_copy::PreviewRequest),
    #[serde(rename = "recovery.experimentPreview")]
    ExperimentPreview(crate::games::trial_copy::experiments::PreviewExperimentRequest),
    #[serde(rename = "recovery.experimentOpen")]
    ExperimentOpen(crate::games::trial_copy::experiments::CleanupRequest),
    #[serde(rename = "trialCopy.cleanup")]
    ExperimentCleanup(crate::games::trial_copy::experiments::CleanupRequest),
    #[serde(rename = "trialCopy.pickDestination")]
    TrialDestination(crate::games::trial_copy::PreviewRequest),
    #[serde(rename = "trialCopy.start")]
    TrialStart(crate::games::trial_copy::PlanRequest),
    #[serde(rename = "trialCopy.task")]
    TrialTask(crate::games::trial_copy::TaskRequest),
    #[serde(rename = "trialCopy.cancel")]
    TrialCancel(crate::games::trial_copy::CancelRequest),
    #[serde(rename = "trialCopy.openGame")]
    TrialOpen(crate::games::trial_copy::TaskRequest),
    #[serde(rename = "storageWorkspace.overview")]
    StorageOverview(crate::games::storage_workspace::OverviewRequest),
    #[serde(rename = "storageWorkspace.pickDestination")]
    StorageDestination(crate::games::storage_workspace::GameRequest),
    #[serde(rename = "storageWorkspace.cleanCache")]
    StorageClean(crate::games::storage_workspace::EmptyRequest),
    #[serde(rename = "storageWorkspace.startMove")]
    StorageMove(crate::games::storage_workspace::PlanRequest),
    #[serde(rename = "storageWorkspace.task")]
    StorageTask(crate::games::storage_workspace::TaskRequest),
    #[serde(rename = "storageWorkspace.cancel")]
    StorageCancel(crate::games::storage_workspace::TaskRequest),
    #[serde(rename = "storageWorkspace.releaseSource")]
    StorageRelease(crate::games::storage_workspace::TaskRequest),
    #[serde(rename = "storageWorkspace.openSource")]
    StorageOpen(crate::games::storage_workspace::TaskRequest),
    #[serde(rename = "configWorkspace.list")]
    ConfigList(super::plugin_config_workspace::GameTarget),
    #[serde(rename = "configWorkspace.read")]
    ConfigRead(super::plugin_config_workspace::ConfigTarget),
    #[serde(rename = "configWorkspace.readBackup")]
    ConfigBackup(super::plugin_config_workspace::BackupTarget),
    #[serde(rename = "configWorkspace.write")]
    ConfigWrite(super::plugin_config_workspace::WriteRequest),
    #[serde(rename = "configWorkspace.restore")]
    ConfigRestore(super::plugin_config_workspace::RestoreRequest),
    #[serde(rename = "recovery.snapshot")]
    RecoverySnapshot(super::plugin_recovery_workspace::GameTarget),
    #[serde(rename = "recovery.openRequest")]
    RecoveryOpen(super::plugin_recovery_workspace::OpenRequest),
    #[serde(rename = "playManifest.preview")]
    PlayPreview(super::plugin_play_manifest::PreviewRequest),
    #[serde(rename = "playManifest.export")]
    PlayExport(super::plugin_play_manifest::ExportRequest),
    #[serde(rename = "fileInbox.pick")]
    InboxPick(Empty),
    #[serde(rename = "fileInbox.pending")]
    InboxPending(Empty),
    #[serde(rename = "fileInbox.open")]
    InboxOpen(super::plugin_file_inbox::OpenRequest),
    #[serde(rename = "fileInbox.openLink")]
    InboxOpenLink(super::plugin_file_inbox::LinkRequest),
    #[serde(rename = "fileInbox.release")]
    InboxRelease(super::plugin_file_inbox::ReleaseRequest),
    #[serde(rename = "langPackage.pick")]
    LangPick(Empty),
    #[serde(rename = "langPackage.check")]
    LangCheck(super::plugin_lang_package::CheckRequest),
    #[serde(rename = "langPackage.document")]
    LangDocument(super::plugin_lang_package::DocumentRequest),
    #[serde(rename = "langPackage.cancel")]
    LangCancel(Empty),
    #[serde(rename = "exports.text")]
    TextExport(super::plugin_frontend_exports::TextRequest),
    #[serde(rename = "clipboard.writeText")]
    ClipboardWrite(ClipboardRequest),
    #[serde(rename = "view.setDirty")]
    ViewSetDirty(DirtyRequest),
    #[serde(rename = "handoffs.receivers")]
    HandoffReceivers(super::plugin_frontend_handoffs::InterfaceRequest),
    #[serde(rename = "handoffs.send")]
    HandoffSend(super::plugin_frontend_handoffs::SendRequest),
    #[serde(rename = "handoffs.take")]
    HandoffTake(super::plugin_frontend_handoffs::InterfaceRequest),
    #[serde(rename = "data.read")]
    DataRead(super::plugin_data::ReadRequest),
    #[serde(rename = "data.write")]
    DataWrite(super::plugin_data::WriteRequest),
    #[serde(rename = "data.remove")]
    DataRemove(super::plugin_data::RemoveRequest),
    #[serde(rename = "worlds.metadata")]
    WorldMetadata(snapshots::Target),
    #[serde(rename = "seedMap.capabilities")]
    SeedMapCapabilities(Empty),
    #[serde(rename = "seedMap.generate")]
    SeedMapGenerate(crate::seed_map::GenerateRequest),
    #[serde(rename = "seedMap.search")]
    SeedMapSearch(crate::seed_map::SearchRequest),
    #[serde(rename = "seedMap.cancel")]
    SeedMapCancel(crate::seed_map::CancelRequest),
    #[serde(rename = "context")]
    Context(Empty),
    #[serde(rename = "inspection.snapshot")]
    InspectionSnapshot(InspectionTarget),
    #[serde(rename = "inspection.check")]
    InspectionCheck(InspectionCheckTarget),
    #[serde(rename = "diagnostics.list")]
    DiagnosticsList(GameTarget),
    #[serde(rename = "diagnostics.read")]
    DiagnosticsRead(DiagnosticTarget),
    #[serde(rename = "diagnostics.check")]
    DiagnosticsCheck(DiagnosticCheck),
    #[serde(rename = "diagnostics.pick")]
    DiagnosticsPick(Empty),
    #[serde(rename = "report.pick")]
    ReportPick(Empty),
    #[serde(rename = "configs.list")]
    List(GameTarget),
    #[serde(rename = "configs.read")]
    Read(ConfigTarget),
    #[serde(rename = "report.check")]
    Check(CheckTarget),
    #[serde(rename = "report.cancel")]
    Cancel(Empty),
    #[serde(rename = "keybindings.preview")]
    KeybindingsPreview(TransferTarget),
    #[serde(rename = "keybindings.apply")]
    KeybindingsApply(TransferApply),
    #[serde(rename = "keybindings.restore")]
    KeybindingsRestore(TransferRestore),
    #[serde(rename = "worldSnapshots.list")]
    WorldSnapshotsList(snapshots::Target),
    #[serde(rename = "worldSnapshots.compare")]
    WorldSnapshotsCompare(snapshots::Compare),
    #[serde(rename = "worldSnapshots.openRestore")]
    WorldSnapshotsOpenRestore(snapshots::RestoreRequest),
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferTarget {
    source_game_id: String,
    target_game_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferApply {
    source_game_id: String,
    target_game_id: String,
    source_revision: String,
    target_revision: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransferRestore {
    target_game_id: String,
    backup_id: String,
    target_revision: String,
}
fn keybinding_permission(manifest: &PluginManifest, write: bool) -> Result<(), String> {
    let nodes = manifest.permissions.as_deref().unwrap_or_default();
    if nodes.contains(&PermissionNode::GameKeybindingsRead {})
        && (!write || nodes.contains(&PermissionNode::GameKeybindingsWrite {}))
    {
        Ok(())
    } else {
        Err("PLUGIN_PERMISSION_DENIED".into())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirtyRequest {
    dirty: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipboardRequest {
    text: String,
}
fn clipboard_text(text: &str) -> Result<(), String> {
    if text.len() > 8192 || text.contains('\0') {
        Err("PLUGIN_CLIPBOARD_TEXT_INVALID".into())
    } else {
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectionTarget {
    game_id: String,
    scope: InspectionScope,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectionCheckTarget {
    game_id: String,
    scope: InspectionScope,
    revision: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticTarget {
    game_id: String,
    diagnostic_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticCheck {
    game_id: String,
    diagnostic_id: String,
    revision: String,
}
fn diagnostic_permission(manifest: &PluginManifest) -> Result<(), String> {
    if manifest
        .permissions
        .as_deref()
        .unwrap_or_default()
        .contains(&PermissionNode::GameDiagnosticsRead {})
    {
        Ok(())
    } else {
        Err("PLUGIN_PERMISSION_DENIED".into())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameTarget {
    game_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigTarget {
    game_id: String,
    config_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckTarget {
    game_id: String,
    config_ids: Vec<String>,
}
fn permission(manifest: &PluginManifest) -> Result<(), String> {
    if manifest
        .permissions
        .as_ref()
        .is_some_and(|nodes| nodes.contains(&PermissionNode::GameConfigRead {}))
    {
        Ok(())
    } else {
        Err("PLUGIN_PERMISSION_DENIED".into())
    }
}
fn unit_permission(manifest: &PluginManifest, node: PermissionNode) -> Result<(), String> {
    if manifest
        .permissions
        .as_deref()
        .unwrap_or_default()
        .contains(&node)
    {
        Ok(())
    } else {
        Err("PLUGIN_PERMISSION_DENIED".into())
    }
}
fn storage_permission(manifest: &PluginManifest) -> Result<(), String> {
    if manifest.permissions.as_deref().unwrap_or_default().iter().any(|node|matches!(node,PermissionNode::GameInspectionRead{scopes}if scopes.contains(&InspectionScope::Storage))){Ok(())}else{Err("PLUGIN_PERMISSION_DENIED".into())}
}
fn authorize_method(manifest: &PluginManifest, request: &FrontendRequest) -> Result<(), String> {
    match request {
        FrontendRequest::MeshSubscribe(args) => super::plugin_mesh_events::permitted(manifest, Some(&args.namespace), false),
        FrontendRequest::MeshReceive(_) | FrontendRequest::MeshUnsubscribe(_) => super::plugin_mesh_events::permitted(manifest, None, false),
        FrontendRequest::MeshSend(args) => super::plugin_mesh_events::permitted(manifest, Some(&args.namespace), true),
        FrontendRequest::MeshPeers(_) => super::plugin_mesh_events::peers_permitted(manifest),
        FrontendRequest::GeneratedSave(_) | FrontendRequest::GeneratedOpen(_) => unit_permission(manifest, PermissionNode::GeneratedBlueprintSave {}),
        FrontendRequest::AiStatus(_) | FrontendRequest::AiChat(_) | FrontendRequest::AiCancel(_) => unit_permission(manifest, PermissionNode::AiChat {}),
        FrontendRequest::AiConfigure(_) | FrontendRequest::AiClear(_) => unit_permission(manifest, PermissionNode::AiConfigure {}),
        FrontendRequest::TrialPreview(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceCopyRequest {})),
        FrontendRequest::ExperimentPreview(_) => storage_permission(manifest)
            .and_then(|()| unit_permission(manifest, PermissionNode::InstanceCopyRequest {}))
            .and_then(|()| unit_permission(manifest, PermissionNode::GameRecoveryRead {}))
            .and_then(|()| {
                if manifest.permissions.as_deref().unwrap_or_default().iter().any(|node| matches!(node,
                    PermissionNode::GameInspectionRead { scopes } if scopes.contains(&InspectionScope::Mods))) {
                    Ok(())
                } else {
                    Err("PLUGIN_PERMISSION_DENIED".into())
                }
            }),
        FrontendRequest::ExperimentOpen(_) | FrontendRequest::ExperimentCleanup(_) => storage_permission(manifest)
            .and_then(|()| unit_permission(manifest, PermissionNode::InstanceCopyRequest {}))
            .and_then(|()| unit_permission(manifest, PermissionNode::GameRecoveryRead {})),
        FrontendRequest::TrialDestination(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceCopyRequest {})),
        FrontendRequest::TrialStart(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceCopyRequest {})),
        FrontendRequest::TrialTask(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceCopyRequest {})),
        FrontendRequest::TrialCancel(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceCopyRequest {})),
        FrontendRequest::TrialOpen(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceCopyRequest {})),
        FrontendRequest::StorageOverview(_) => storage_permission(manifest),
        FrontendRequest::StorageDestination(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceMoveRequest {})),
        FrontendRequest::StorageClean(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::StorageCacheClear {})),
        FrontendRequest::StorageMove(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceMoveRequest {})),
        FrontendRequest::StorageTask(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceMoveRequest {})),
        FrontendRequest::StorageCancel(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceMoveRequest {})),
        FrontendRequest::StorageRelease(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceMoveRequest {})),
        FrontendRequest::StorageOpen(_) => storage_permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::InstanceMoveRequest {})),
        FrontendRequest::ConfigList(_) => permission(manifest),
        FrontendRequest::ConfigRead(_) => permission(manifest),
        FrontendRequest::ConfigBackup(_) => permission(manifest),
        FrontendRequest::ConfigWrite(_) => permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::GameConfigWrite {})),
        FrontendRequest::ConfigRestore(_) => permission(manifest).and_then(|()| unit_permission(manifest,PermissionNode::GameConfigWrite {})),
        FrontendRequest::RecoverySnapshot(_) => unit_permission(manifest,PermissionNode::GameRecoveryRead {}),
        FrontendRequest::RecoveryOpen(_) => unit_permission(manifest,PermissionNode::GameRecoveryRead {}).and_then(|()|unit_permission(manifest,PermissionNode::GameRecoveryOpenRequest {})),
        FrontendRequest::PlayPreview(_) => unit_permission(manifest,PermissionNode::PlayManifestExport {}),
        FrontendRequest::PlayExport(_) => unit_permission(manifest,PermissionNode::PlayManifestExport {}),
        FrontendRequest::InboxPick(_) => unit_permission(manifest,PermissionNode::FileInboxImportRequest {}),
        FrontendRequest::InboxPending(_) => unit_permission(manifest,PermissionNode::FileInboxImportRequest {}),
        FrontendRequest::InboxOpen(_) => unit_permission(manifest,PermissionNode::FileInboxImportRequest {}),
        FrontendRequest::InboxOpenLink(_) => unit_permission(manifest,PermissionNode::FileInboxImportRequest {}),
        FrontendRequest::InboxRelease(_) => unit_permission(manifest,PermissionNode::FileInboxImportRequest {}),
        FrontendRequest::LangPick(_) => super::plugin_lang_package::permitted(manifest),
        FrontendRequest::LangCheck(_) => super::plugin_lang_package::permitted(manifest),
        FrontendRequest::LangDocument(_) => super::plugin_lang_package::permitted(manifest),
        FrontendRequest::LangCancel(_) => super::plugin_lang_package::permitted(manifest),

        FrontendRequest::TextExport(_) => if manifest.permissions.as_deref().unwrap_or_default().contains(&PermissionNode::TextExport {}) { Ok(()) } else { Err("PLUGIN_PERMISSION_DENIED".into()) },
        FrontendRequest::ClipboardWrite(_) => if manifest.permissions.as_deref().unwrap_or_default().contains(&PermissionNode::ClipboardWrite {}) { Ok(()) } else { Err("PLUGIN_PERMISSION_DENIED".into()) },
        FrontendRequest::ViewSetDirty(_) | FrontendRequest::HandoffReceivers(_) | FrontendRequest::HandoffSend(_) | FrontendRequest::HandoffTake(_) => Ok(()),
        FrontendRequest::DataRead(_) | FrontendRequest::DataWrite(_) | FrontendRequest::DataRemove(_) => super::plugin_data::permitted(manifest),
        FrontendRequest::WorldMetadata(_) => world_metadata::permitted(manifest),
        FrontendRequest::SeedMapCapabilities(_) | FrontendRequest::SeedMapGenerate(_)
            | FrontendRequest::SeedMapSearch(_) | FrontendRequest::SeedMapCancel(_) => Ok(()),
        FrontendRequest::ReportPick(_) => super::plugin_reports::managed_selected_report::permitted(manifest),
        FrontendRequest::InspectionSnapshot(args)=>super::plugin_reports::managed_inspection_report::permitted(manifest,args.scope),
        FrontendRequest::InspectionCheck(args)=>super::plugin_reports::managed_inspection_report::permitted(manifest,args.scope),
        FrontendRequest::DiagnosticsList(_) | FrontendRequest::DiagnosticsRead(_) | FrontendRequest::DiagnosticsCheck(_) => diagnostic_permission(manifest),
        FrontendRequest::DiagnosticsPick(_) => {
            if manifest.capabilities == ["report"] && manifest.permissions.as_deref().unwrap_or_default().iter().any(|node| matches!(node, PermissionNode::SelectedFileRead { extensions } if manifest.file_extensions.iter().all(|extension| extensions.contains(extension)) && extensions.iter().all(|extension| matches!(extension.as_str(), "log" | "txt")))) { Ok(()) } else { Err("PLUGIN_PERMISSION_DENIED".into()) }
        }
        FrontendRequest::List(_) | FrontendRequest::Read(_) | FrontendRequest::Check(_) => {
            permission(manifest)
        }
        FrontendRequest::Context(_) | FrontendRequest::Cancel(_) => Ok(()),
        FrontendRequest::KeybindingsPreview(_) => keybinding_permission(manifest, false),
        FrontendRequest::KeybindingsApply(_) | FrontendRequest::KeybindingsRestore(_) => {
            keybinding_permission(manifest, true)
        }
        FrontendRequest::WorldSnapshotsList(_) | FrontendRequest::WorldSnapshotsCompare(_) => snapshots::permitted(manifest, false),
        FrontendRequest::WorldSnapshotsOpenRestore(_) => snapshots::permitted(manifest, true),
    }
}
fn report_locale(locale: &str) -> &'static str {
    if locale == "zh-CN" { "zh-CN" } else { "en" }
}
fn target(bound: Option<&str>, game: &str) -> Result<(), String> {
    if game.is_empty() || game.len() > 128 || bound.is_some_and(|id| id != game) {
        Err("PLUGIN_GAME_SCOPE".into())
    } else {
        Ok(())
    }
}
fn supported(manifest: &PluginManifest, name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| {
        manifest
            .file_extensions
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(ext))
    })
}
fn safe_error(error: String) -> String {
    const KNOWN: &[&str] = &[
        "CONFIG_NOT_FOUND",
        "CONFIG_NOT_UTF8",
        "CONFIG_TOO_LARGE",
        "CONFIG_SELECTION_INVALID",
        "CANCEL_REQUESTED",
        "PLUGIN_REPORT_BUSY",
        "PLUGIN_VIEW_STALE",
        "FOLDER_REPORT_PERMISSION_REQUIRED",
        "CONFIG_CHANGED",
        "CONFIG_BUSY",
        "CONFIG_BACKUP_NOT_FOUND",
        "CONFIG_BACKUP_CHANGED",
        "KEYBINDINGS_SAME_GAME",
        "KEYBINDINGS_NO_CHANGES",
        "KEYBINDINGS_DUPLICATE",
        "DIAGNOSTIC_NOT_FOUND",
        "DIAGNOSTIC_NOT_UTF8",
        "DIAGNOSTIC_TOO_LARGE",
        "DIAGNOSTIC_INVENTORY_LIMIT",
        "DIAGNOSTIC_CHANGED",
        "DIAGNOSTIC_IO",
        "INSPECTION_FAILED",
        "INSPECTION_LIMIT",
        "INSPECTION_CHANGED",
        "INSPECTION_GAME_UNAVAILABLE",
        "INSPECTION_UNAVAILABLE",
        "PLUGIN_WORLD_SCOPE",
        "WORLD_SNAPSHOT_UNAVAILABLE",
        "WORLD_SNAPSHOT_LIMIT",
        "WORLD_SNAPSHOT_RANGE",
        "WORLD_SNAPSHOT_COMPARE_FAILED",
        "WORLD_SNAPSHOT_NOT_FOUND",
    ];
    if KNOWN.contains(&error.as_str()) {
        error
    } else {
        "PLUGIN_FRONTEND_FAILED".into()
    }
}
struct Access {
    capability: &'static str,
    action: &'static str,
    game_id: Option<String>,
}
fn access_requests(request: &FrontendRequest) -> Vec<Access> {
    let one = |capability, action, game: Option<&str>| {
        vec![Access {
            capability,
            action,
            game_id: game.map(str::to_owned),
        }]
    };
    match request {
        FrontendRequest::MeshSubscribe(_) => one("mesh-events-receive", "mesh.subscribe", None),
        FrontendRequest::MeshReceive(_) => one("mesh-events-receive", "mesh.receive", None),
        FrontendRequest::MeshUnsubscribe(_) => one("mesh-events-receive", "mesh.unsubscribe", None),
        FrontendRequest::MeshSend(_) => one("mesh-events-send", "mesh.send", None),
        FrontendRequest::MeshPeers(_) => one("mesh-events", "mesh.peers", None),
        FrontendRequest::GeneratedSave(_) => {
            one("generated-blueprint", "generatedBlueprint.save", None)
        }
        FrontendRequest::GeneratedOpen(_) => {
            one("generated-blueprint", "generatedBlueprint.open", None)
        }
        FrontendRequest::AiStatus(_) => one("ai-chat", "ai.status", None),
        FrontendRequest::AiConfigure(_) => one("ai-configure", "ai.configure", None),
        FrontendRequest::AiClear(_) => one("ai-configure", "ai.clear", None),
        FrontendRequest::AiChat(args) => {
            let mut accesses = one("ai-chat", "ai.chat", None);
            if let Some(game_id) = args.diagnostic_game_id() {
                accesses.extend(one(
                    "game-diagnostics-read",
                    "ai.chat.attachDiagnostic",
                    Some(game_id),
                ));
            }
            accesses
        }
        FrontendRequest::AiCancel(_) => one("ai-chat", "ai.cancel", None),
        FrontendRequest::TrialPreview(args) => {
            one("instance-copy", "trialCopy.preview", Some(&args.game_id))
        }
        FrontendRequest::ExperimentPreview(args) => one(
            "game-recovery",
            "recovery.experimentPreview",
            Some(&args.game_id),
        ),
        FrontendRequest::ExperimentOpen(args) => one(
            "game-recovery",
            "recovery.experimentOpen",
            Some(&args.game_id),
        ),
        FrontendRequest::ExperimentCleanup(args) => one(
            "instance-copy",
            "trialCopy.cleanup",
            Some(&args.copy_game_id),
        ),
        FrontendRequest::TrialDestination(args) => one(
            "instance-copy",
            "trialCopy.pickDestination",
            Some(&args.game_id),
        ),
        FrontendRequest::TrialStart(_) => one("instance-copy", "trialCopy.start", None),
        FrontendRequest::TrialTask(_) => one("instance-copy", "trialCopy.task", None),
        FrontendRequest::TrialCancel(_) => one("instance-copy", "trialCopy.cancel", None),
        FrontendRequest::TrialOpen(_) => one("instance-copy", "trialCopy.openGame", None),
        FrontendRequest::StorageOverview(args) => one(
            "storage-workspace",
            "storageWorkspace.overview",
            args.game_id.as_deref(),
        ),
        FrontendRequest::StorageDestination(args) => one(
            "storage-workspace",
            "storageWorkspace.pickDestination",
            Some(&args.game_id),
        ),
        FrontendRequest::StorageClean(_) => {
            one("storage-workspace", "storageWorkspace.cleanCache", None)
        }
        FrontendRequest::StorageMove(_) => {
            one("storage-workspace", "storageWorkspace.startMove", None)
        }
        FrontendRequest::StorageTask(_) => one("storage-workspace", "storageWorkspace.task", None),
        FrontendRequest::StorageCancel(_) => {
            one("storage-workspace", "storageWorkspace.cancel", None)
        }
        FrontendRequest::StorageRelease(_) => {
            one("storage-workspace", "storageWorkspace.releaseSource", None)
        }
        FrontendRequest::StorageOpen(_) => {
            one("storage-workspace", "storageWorkspace.openSource", None)
        }
        FrontendRequest::ConfigList(args) => one(
            "game-config-read",
            "configWorkspace.list",
            Some(&args.game_id),
        ),
        FrontendRequest::ConfigRead(args) => one(
            "game-config-read",
            "configWorkspace.read",
            Some(&args.game_id),
        ),
        FrontendRequest::ConfigBackup(args) => one(
            "game-config-read",
            "configWorkspace.readBackup",
            Some(&args.game_id),
        ),
        FrontendRequest::ConfigWrite(args) => one(
            "game-config-write",
            "configWorkspace.write",
            Some(&args.game_id),
        ),
        FrontendRequest::ConfigRestore(args) => one(
            "game-config-write",
            "configWorkspace.restore",
            Some(&args.game_id),
        ),
        FrontendRequest::RecoverySnapshot(args) => {
            one("game-recovery", "recovery.snapshot", Some(&args.game_id))
        }
        FrontendRequest::RecoveryOpen(args) => {
            one("game-recovery", "recovery.openRequest", Some(&args.game_id))
        }
        FrontendRequest::PlayPreview(args) => {
            one("play-manifest", "playManifest.preview", Some(&args.game_id))
        }
        FrontendRequest::PlayExport(args) => {
            one("play-manifest", "playManifest.export", Some(&args.game_id))
        }
        FrontendRequest::InboxPick(_) => one("file-inbox", "fileInbox.pick", None),
        FrontendRequest::InboxPending(_) => one("file-inbox", "fileInbox.pending", None),
        FrontendRequest::InboxOpen(args) => {
            one("file-inbox", "fileInbox.open", args.game_id.as_deref())
        }
        FrontendRequest::InboxOpenLink(args) => {
            one("file-inbox", "fileInbox.openLink", args.game_id.as_deref())
        }
        FrontendRequest::InboxRelease(_) => one("file-inbox", "fileInbox.release", None),
        FrontendRequest::LangPick(_) => one("selected-file-read", "langPackage.pick", None),
        FrontendRequest::LangCheck(_) => one("language-report", "langPackage.check", None),
        FrontendRequest::LangDocument(_) => one("selected-file-read", "langPackage.document", None),
        FrontendRequest::LangCancel(_) => one("language-report", "langPackage.cancel", None),

        FrontendRequest::TextExport(_) => one("text-export", "exports.text", None),
        FrontendRequest::ClipboardWrite(_) => one("clipboard-write", "clipboard.writeText", None),
        FrontendRequest::HandoffSend(args) => {
            one("plugin-handoff", "handoffs.send", args.game_id())
        }
        FrontendRequest::HandoffTake(_) => one("plugin-handoff", "handoffs.take", None),
        FrontendRequest::HandoffReceivers(_) | FrontendRequest::ViewSetDirty(_) => vec![],
        FrontendRequest::DataRead(args) => one(
            "private-storage",
            "data.read",
            args.scope.game_id.as_deref(),
        ),
        FrontendRequest::DataWrite(args) => one(
            "private-storage",
            "data.write",
            args.scope.game_id.as_deref(),
        ),
        FrontendRequest::DataRemove(args) => one(
            "private-storage",
            "data.remove",
            args.scope.game_id.as_deref(),
        ),
        FrontendRequest::WorldMetadata(args) => {
            one("worlds-seed-read", "worlds.metadata", Some(&args.game_id))
        }
        FrontendRequest::SeedMapGenerate(_) => one("seed-map", "seedMap.generate", None),
        FrontendRequest::SeedMapSearch(_) => one("seed-map", "seedMap.search", None),
        FrontendRequest::SeedMapCapabilities(_) | FrontendRequest::SeedMapCancel(_) => vec![],
        FrontendRequest::InspectionSnapshot(args) => one(
            "game-inspection-read",
            "inspection.snapshot",
            Some(&args.game_id),
        ),
        FrontendRequest::InspectionCheck(args) => one(
            "game-inspection-read",
            "inspection.check",
            Some(&args.game_id),
        ),
        FrontendRequest::DiagnosticsList(args) => one(
            "game-diagnostics-read",
            "diagnostics.list",
            Some(&args.game_id),
        ),
        FrontendRequest::DiagnosticsRead(args) => one(
            "game-diagnostics-read",
            "diagnostics.read",
            Some(&args.game_id),
        ),
        FrontendRequest::DiagnosticsCheck(args) => one(
            "game-diagnostics-read",
            "diagnostics.check",
            Some(&args.game_id),
        ),
        FrontendRequest::DiagnosticsPick(_) => one("selected-file-read", "diagnostics.pick", None),
        FrontendRequest::ReportPick(_) => one("selected-file-read", "report.pick", None),
        FrontendRequest::List(args) => one("game-config-read", "configs.list", Some(&args.game_id)),
        FrontendRequest::Read(args) => one("game-config-read", "configs.read", Some(&args.game_id)),
        FrontendRequest::Check(args) => {
            one("game-config-read", "report.check", Some(&args.game_id))
        }
        FrontendRequest::KeybindingsPreview(args) => vec![
            Access {
                capability: "game-keybindings-read",
                action: "keybindings.preview",
                game_id: Some(args.source_game_id.clone()),
            },
            Access {
                capability: "game-keybindings-read",
                action: "keybindings.preview",
                game_id: Some(args.target_game_id.clone()),
            },
        ],
        FrontendRequest::KeybindingsApply(args) => vec![
            Access {
                capability: "game-keybindings-read",
                action: "keybindings.apply",
                game_id: Some(args.source_game_id.clone()),
            },
            Access {
                capability: "game-keybindings-write",
                action: "keybindings.apply",
                game_id: Some(args.target_game_id.clone()),
            },
        ],
        FrontendRequest::KeybindingsRestore(args) => one(
            "game-keybindings-write",
            "keybindings.restore",
            Some(&args.target_game_id),
        ),
        FrontendRequest::WorldSnapshotsList(args) => one(
            "world-snapshots-read",
            "worldSnapshots.list",
            Some(&args.game_id),
        ),
        FrontendRequest::WorldSnapshotsCompare(args) => one(
            "world-snapshots-read",
            "worldSnapshots.compare",
            Some(&args.game_id),
        ),
        FrontendRequest::WorldSnapshotsOpenRestore(args) => one(
            "world-snapshots-restore-request",
            "worldSnapshots.openRestore",
            Some(&args.game_id),
        ),
        FrontendRequest::Context(_) | FrontendRequest::Cancel(_) => vec![],
    }
}
fn access_outcome(result: &Result<Value, String>) -> super::plugin_access_log::Outcome {
    use super::plugin_access_log::Outcome;
    match result {
        Ok(value) if value.get("saved") == Some(&Value::Bool(false)) => Outcome::Cancelled,
        Ok(_) => Outcome::Completed,
        Err(code)
            if matches!(
                code.as_str(),
                "PLUGIN_PERMISSION_DENIED"
                    | "PLUGIN_GAME_SCOPE"
                    | "PLUGIN_WORLD_SCOPE"
                    | "FOLDER_REPORT_PERMISSION_REQUIRED"
                    | "PLUGIN_HANDOFF_GAME_SCOPE"
                    | "PLUGIN_HANDOFF_VIEW_SCOPE"
                    | "PLUGIN_HANDOFF_INTERFACE_NOT_DECLARED"
            ) =>
        {
            Outcome::Denied
        }
        Err(code)
            if matches!(
                code.as_str(),
                "CANCEL_REQUESTED"
                    | "PLUGIN_VIEW_CLOSED"
                    | "SEED_MAP_CANCELLED"
                    | "PLUGIN_ACTION_CANCELLED"
                    | "TRIAL_COPY_CANCELLED"
                    | "AI_CANCELLED"
                    | "AI_CONFIRM_CANCELLED"
            ) =>
        {
            Outcome::Cancelled
        }
        Err(_) => Outcome::Failed,
    }
}
fn audit_result(access: &Access, result: &Result<Value, String>) -> bool {
    // Empty adaptive pulls must not rewrite the on-disk audit journal.
    access.action != "mesh.receive"
        || !result.as_ref().is_ok_and(|value| {
            value["messages"].as_array().is_some_and(Vec::is_empty) && value["missed"] != true
        })
}
#[tauri::command]
pub async fn frontend_call(
    app: AppHandle,
    webview: Webview,
    request: FrontendRequest,
) -> Result<Value, String> {
    let auth = super::plugin_views::authorize_view(&webview)?;
    // Identity is native-authorized before a request may contribute audit metadata.
    let accesses = access_requests(&request);
    let plugin_id = auth.manifest.id.clone();
    let result: Result<Value, String> = async {
    authorize_method(&auth.manifest, &request)?;
    let locale = report_locale(&auth.locale);
    let result = match request {
        FrontendRequest::MeshSubscribe(args) => super::plugin_mesh_events::subscribe(&auth,args).await,
        FrontendRequest::MeshReceive(args) => super::plugin_mesh_events::receive(&auth,args).await,
        FrontendRequest::MeshUnsubscribe(args) => super::plugin_mesh_events::unsubscribe(&auth,args).await,
        FrontendRequest::MeshSend(args) => super::plugin_mesh_events::send(&auth,args).await,
        FrontendRequest::MeshPeers(_) => super::plugin_mesh_events::peers(&auth).await,
        FrontendRequest::AiStatus(_) => super::plugin_ai::status().await,
        FrontendRequest::AiConfigure(args) => super::plugin_ai::configure(args).await,
        FrontendRequest::GeneratedSave(args) => super::plugin_generated_blueprint::save(&auth.manifest.id, args),
        FrontendRequest::GeneratedOpen(args) => super::plugin_generated_blueprint::open(app, &auth.manifest.id, args),
        FrontendRequest::AiClear(_) => super::plugin_ai::clear().await,
        FrontendRequest::AiChat(args) => super::plugin_ai::chat(app.clone(), &auth, args).await,
        FrontendRequest::AiCancel(args) => super::plugin_ai::cancel(&auth, args),
        FrontendRequest::TrialPreview(args) => crate::games::trial_copy::preview(app.clone(), &auth, args).await,
        FrontendRequest::ExperimentPreview(args) => crate::games::trial_copy::experiments::preview(app.clone(), &auth, args).await,
        FrontendRequest::ExperimentOpen(args) => crate::games::trial_copy::experiments::open(app.clone(), &auth, args).await,
        FrontendRequest::ExperimentCleanup(args) => crate::games::trial_copy::experiments::cleanup(app.clone(), &auth, args).await,
        FrontendRequest::TrialDestination(args) => crate::games::trial_copy::pick_destination(app.clone(), &auth, args).await,
        FrontendRequest::TrialStart(args) => crate::games::trial_copy::start(app.clone(), &auth, args).await,
        FrontendRequest::TrialTask(args) => crate::games::trial_copy::task(app.clone(), &auth, args).await,
        FrontendRequest::TrialCancel(args) => crate::games::trial_copy::cancel(app.clone(), &auth, args).await,
        FrontendRequest::TrialOpen(args) => crate::games::trial_copy::open_game(app.clone(), &auth, args).await,
        FrontendRequest::StorageOverview(args) => crate::games::storage_workspace::overview(app.clone(), &auth, args).await,
        FrontendRequest::StorageDestination(args) => crate::games::storage_workspace::pick_destination(app.clone(), &auth, args).await,
        FrontendRequest::StorageClean(args) => crate::games::storage_workspace::clean_cache(app.clone(), &auth, args).await,
        FrontendRequest::StorageMove(args) => crate::games::storage_workspace::start_move(app.clone(), &auth, args).await,
        FrontendRequest::StorageTask(args) => crate::games::storage_workspace::task(app.clone(), &auth, args).await,
        FrontendRequest::StorageCancel(args) => crate::games::storage_workspace::cancel(app.clone(), &auth, args).await,
        FrontendRequest::StorageRelease(args) => crate::games::storage_workspace::release_source(app.clone(), &auth, args).await,
        FrontendRequest::StorageOpen(args) => crate::games::storage_workspace::open_source(app.clone(), &auth, args).await,
        FrontendRequest::ConfigList(args) => super::plugin_config_workspace::list(app.clone(), auth.clone(), args).await,
        FrontendRequest::ConfigRead(args) => super::plugin_config_workspace::read(app.clone(), auth.clone(), args).await,
        FrontendRequest::ConfigBackup(args) => super::plugin_config_workspace::read_backup(app.clone(), auth.clone(), args).await,
        FrontendRequest::ConfigWrite(args) => super::plugin_config_workspace::write(app.clone(), auth.clone(), args).await,
        FrontendRequest::ConfigRestore(args) => super::plugin_config_workspace::restore(app.clone(), auth.clone(), args).await,
        FrontendRequest::RecoverySnapshot(args) => super::plugin_recovery_workspace::snapshot(app.clone(), &auth, args).await,
        FrontendRequest::RecoveryOpen(args) => super::plugin_recovery_workspace::open_request(app.clone(), &auth, args).await,
        FrontendRequest::PlayPreview(args) => super::plugin_play_manifest::preview(app.clone(), &auth, args).await,
        FrontendRequest::PlayExport(args) => super::plugin_play_manifest::export(app.clone(), &auth, args).await,
        FrontendRequest::InboxPick(_) => super::plugin_file_inbox::pick(app.clone(), &auth).await,
        FrontendRequest::InboxPending(_) => super::plugin_file_inbox::pending(app.clone(), &auth).await,
        FrontendRequest::InboxOpen(args) => super::plugin_file_inbox::open(app.clone(), &auth, args).await,
        FrontendRequest::InboxOpenLink(args) => super::plugin_file_inbox::open_link(app.clone(), &auth, args).await,
        FrontendRequest::InboxRelease(args) => super::plugin_file_inbox::release(app.clone(), &auth, args).await,
        FrontendRequest::LangPick(_) => serde_json::to_value(super::plugin_lang_package::pick(app.clone(), &auth).await?).map_err(|_| "LANG_PACKAGE_IO".into()),
        FrontendRequest::LangCheck(args) => serde_json::to_value(super::plugin_lang_package::check(app.clone(), &auth, args).await?).map_err(|_| "LANG_PACKAGE_IO".into()),
        FrontendRequest::LangDocument(args) => super::plugin_lang_package::document(&auth, args).await,
        FrontendRequest::LangCancel(_) => super::plugin_lang_package::cancel(&auth).await.map(|()|Value::Null),

        FrontendRequest::TextExport(args) => super::plugin_frontend_exports::text(app.clone(), &auth, args).await,
        FrontendRequest::ClipboardWrite(args) => {
            clipboard_text(&args.text)?;
            app.clipboard().write_text(args.text).map_err(|_| "PLUGIN_CLIPBOARD_UNAVAILABLE")?;
            Ok(Value::Null)
        },
        FrontendRequest::ViewSetDirty(args) => { super::plugin_views::set_dirty(&auth.session_id, args.dirty)?; Ok(Value::Null) },
        FrontendRequest::HandoffReceivers(args) => super::plugin_frontend_handoffs::receivers(&auth, args).await,
        FrontendRequest::HandoffSend(args) => super::plugin_frontend_handoffs::send(app.clone(), &auth, args).await,
        FrontendRequest::HandoffTake(args) => super::plugin_frontend_handoffs::take(&auth, args),
        FrontendRequest::DataRead(args) => serde_json::to_value(super::plugin_data::read_for_frontend(&auth,args).await?).map_err(|_|"PLUGIN_DATA_UNAVAILABLE".into()),
        FrontendRequest::DataWrite(args) => serde_json::to_value(super::plugin_data::write_for_frontend(&auth,args).await?).map_err(|_|"PLUGIN_DATA_UNAVAILABLE".into()),
        FrontendRequest::DataRemove(args) => super::plugin_data::remove_for_frontend(&auth,args).await.map(|()| Value::Null),
        FrontendRequest::WorldMetadata(args) => world_metadata::read(app.clone(), auth.instance_id.as_deref(), args).await,
        FrontendRequest::SeedMapCapabilities(_) => serde_json::to_value(crate::seed_map::capabilities_for_frontend()?).map_err(|_|"SEED_MAP_WORKER_FAILED".into()),
        FrontendRequest::SeedMapGenerate(args) => serde_json::to_value(crate::seed_map::generate_for_frontend(&auth,args).await?).map_err(|_|"SEED_MAP_WORKER_FAILED".into()),
        FrontendRequest::SeedMapSearch(args) => serde_json::to_value(crate::seed_map::search_for_frontend(&auth,args).await?).map_err(|_|"SEED_MAP_WORKER_FAILED".into()),
        FrontendRequest::SeedMapCancel(args) => serde_json::to_value(crate::seed_map::cancel_for_frontend(&auth,args).await?).map_err(|_|"SEED_MAP_WORKER_FAILED".into()),
        FrontendRequest::Context(_) => {
            let mut inspection_games=if auth.manifest.permissions.as_deref().unwrap_or_default().iter().any(|node|matches!(node,PermissionNode::GameInspectionRead {scopes} if !scopes.is_empty())){crate::games::frontend_game_summaries(&app)?}else{vec![]};
            if let Some(id)=&auth.instance_id {inspection_games.retain(|game|game["id"].as_str()==Some(id));}
            let mut games = if permission(&auth.manifest).is_ok() || auth.manifest.permissions.as_deref().unwrap_or_default().iter().any(|node|matches!(node,PermissionNode::GameRecoveryRead {}|PermissionNode::PlayManifestExport {}|PermissionNode::InstanceCopyRequest {}|PermissionNode::InstanceMoveRequest {}|PermissionNode::FileInboxImportRequest {})) {
                crate::games::frontend_game_summaries(&app)?
            } else {
                vec![]
            };
            if let Some(id) = &auth.instance_id {
                games.retain(|game| game["id"].as_str() == Some(id));
            }
            let keybinding_games = if keybinding_permission(&auth.manifest, false).is_ok() {
                crate::games::frontend_game_summaries(&app)?
            } else {
                vec![]
            };
            Ok(
                json!({"locale":locale,"theme":auth.theme,"appearance":auth.appearance,"instanceId":auth.instance_id,"keybindingTargetId":auth.keybinding_target_id,"configRestoreTarget":auth.config_restore_target,"games":games,"keybindingGames":keybinding_games,"inspectionGames":inspection_games,"recentGames":if diagnostic_permission(&auth.manifest).is_ok() || keybinding_permission(&auth.manifest, false).is_ok() { crate::games::game_diagnostics::recent(&app, auth.instance_id.as_deref())? } else { vec![] }}),
            )
        }
        FrontendRequest::InspectionSnapshot(args)=>{
            target(auth.instance_id.as_deref(),&args.game_id)?;
            let snapshot=super::plugin_reports::managed_inspection_report::snapshot(&app,&args.game_id,args.scope).await.map_err(safe_error)?;
            serde_json::to_value(snapshot).map_err(|_|"PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::InspectionCheck(args)=>{
            target(auth.instance_id.as_deref(),&args.game_id)?;
            let job=Arc::new(AtomicBool::new(false));
            super::plugin_views::register_job(&auth.session_id,job.clone())?;
            let report=super::plugin_reports::managed_inspection_report::run_for_frontend(app.clone(),args.game_id,args.scope,args.revision,auth.manifest.clone(),locale.into(),job.clone()).await;
            super::plugin_views::clear_job(&auth.session_id,&job);
            serde_json::to_value(report.map_err(safe_error)?).map_err(|_|"PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::ReportPick(_) => {
            let job = Arc::new(AtomicBool::new(false));
            super::plugin_views::register_job(&auth.session_id, job.clone())?;
            let report = super::plugin_reports::managed_selected_report::pick_for_frontend(app.clone(), auth.manifest.clone(), locale.into(), auth.instance_id.clone(), job.clone()).await;
            super::plugin_views::clear_job(&auth.session_id, &job);
            serde_json::to_value(report.map_err(safe_error)?).map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::DiagnosticsPick(_) => {
            let job = Arc::new(AtomicBool::new(false));
            super::plugin_views::register_job(&auth.session_id, job.clone())?;
            let report = super::plugin_reports::managed_diagnostic_report::pick_for_frontend(app.clone(), auth.manifest.clone(), locale.into(), auth.instance_id.clone(), job.clone()).await;
            super::plugin_views::clear_job(&auth.session_id, &job);
            serde_json::to_value(report.map_err(safe_error)?).map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::DiagnosticsList(args) => {
            target(auth.instance_id.as_deref(), &args.game_id)?;
            let files = crate::games::game_diagnostics::list(app.clone(), args.game_id).await.map_err(safe_error)?;
            serde_json::to_value(files.into_iter().filter(|file| supported(&auth.manifest, &file.name)).collect::<Vec<_>>()).map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::DiagnosticsRead(args) => {
            target(auth.instance_id.as_deref(), &args.game_id)?;
            let document = crate::games::game_diagnostics::read(app.clone(), args.game_id, args.diagnostic_id).await.map_err(safe_error)?;
            if !supported(&auth.manifest, &document.name) { return Err("PLUGIN_PERMISSION_DENIED".into()); }
            serde_json::to_value(document).map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::DiagnosticsCheck(args) => {
            target(auth.instance_id.as_deref(), &args.game_id)?;
            let job = Arc::new(AtomicBool::new(false));
            super::plugin_views::register_job(&auth.session_id, job.clone())?;
            let report = super::plugin_reports::managed_diagnostic_report::run_for_frontend(app.clone(), args.game_id, args.diagnostic_id, args.revision, auth.manifest.clone(), locale.into(), job.clone()).await;
            super::plugin_views::clear_job(&auth.session_id, &job);
            serde_json::to_value(report.map_err(safe_error)?).map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::List(args) => {
            target(auth.instance_id.as_deref(), &args.game_id)?;
            let files = crate::games::game_config::list_game_configs(app.clone(), args.game_id)
                .await
                .map_err(safe_error)?;
            let value = serde_json::to_value(files).map_err(|_| "PLUGIN_FRONTEND_FAILED")?;
            Ok(Value::Array(
                value
                    .as_array()
                    .ok_or("PLUGIN_FRONTEND_FAILED")?
                    .iter()
                    .filter(|file| supported(&auth.manifest, file["name"].as_str().unwrap_or("")))
                    .cloned()
                    .collect(),
            ))
        }
        FrontendRequest::Read(args) => {
            target(auth.instance_id.as_deref(), &args.game_id)?;
            if args.config_id.len() != 64 || !args.config_id.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("CONFIG_NOT_FOUND".into());
            }
            let file = crate::games::game_config::read_game_config(
                app.clone(),
                args.game_id,
                args.config_id,
            )
            .await
            .map_err(safe_error)?;
            let value = serde_json::to_value(file).map_err(|_| "PLUGIN_FRONTEND_FAILED")?;
            if !supported(&auth.manifest, value["name"].as_str().unwrap_or("")) {
                return Err("PLUGIN_PERMISSION_DENIED".into());
            }
            Ok(
                json!({"id":value["id"],"name":value["name"],"text":value["text"],"revision":value["revision"]}),
            )
        }
        FrontendRequest::Check(args) => {
            target(auth.instance_id.as_deref(), &args.game_id)?;
            if args.config_ids.is_empty() || args.config_ids.len() > 64 {
                return Err("CONFIG_SELECTION_INVALID".into());
            }
            let job = Arc::new(AtomicBool::new(false));
            super::plugin_views::register_job(&auth.session_id, job.clone())?;
            let report = super::plugin_reports::managed_config_report::run_for_frontend(
                app.clone(),
                args.game_id,
                args.config_ids,
                auth.manifest.clone(),
                locale.into(),
                job.clone(),
            )
            .await;
            super::plugin_views::clear_job(&auth.session_id, &job);
            serde_json::to_value(report.map_err(safe_error)?)
                .map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::KeybindingsPreview(args) => {
            target(auth.instance_id.as_deref(), &args.source_game_id)?;
            let value = crate::games::game_config::keybindings::preview_game_keybindings(
                app.clone(),
                args.source_game_id,
                args.target_game_id,
            )
            .await
            .map_err(safe_error)?;
            serde_json::to_value(value).map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::KeybindingsApply(args) => {
            target(auth.instance_id.as_deref(), &args.source_game_id)?;
            let value = crate::games::game_config::keybindings::apply_game_keybindings(
                app.clone(),
                args.source_game_id,
                args.target_game_id,
                args.source_revision,
                args.target_revision,
            )
            .await
            .map_err(safe_error)?;
            serde_json::to_value(value).map_err(|_| "PLUGIN_FRONTEND_FAILED".into())
        }
        FrontendRequest::KeybindingsRestore(args) => {
            let value = crate::games::game_config::keybindings::restore_game_keybindings(
                app.clone(),
                args.target_game_id,
                args.backup_id,
                args.target_revision,
            )
            .await
            .map_err(safe_error)?;
            Ok(json!(value))
        }
        FrontendRequest::WorldSnapshotsList(args) => snapshots::list(app.clone(), auth.instance_id.as_deref(), args).await,
        FrontendRequest::WorldSnapshotsCompare(args) => snapshots::compare(app.clone(), auth.instance_id.as_deref(), args).await,
        FrontendRequest::WorldSnapshotsOpenRestore(args) => snapshots::open_restore(app.clone(), auth.instance_id.as_deref(), &auth.manifest.id, args, &auth.cancel).await,
        FrontendRequest::Cancel(_) => {
            let canceled = Arc::new(AtomicBool::new(true));
            super::plugin_views::register_job(&auth.session_id, canceled.clone())?;
            super::plugin_views::clear_job(&auth.session_id, &canceled);
            Ok(Value::Null)
        }
    };
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_CLOSED".into());
    }
    super::plugin_views::authorize_view(&webview)?;
    result
    }.await;
    let outcome = access_outcome(&result);
    for access in accesses {
        if !audit_result(&access, &result) {
            continue;
        }
        super::plugin_access_log::record(
            &plugin_id,
            access.capability,
            access.action,
            access.game_id.as_deref().map_or(
                super::plugin_access_log::Target::None,
                super::plugin_access_log::Target::Game,
            ),
            outcome,
        );
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mesh_methods_have_exact_grants_and_metadata_only_audit() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/ai-assistant/plugin.json"
        ))
        .unwrap();
        let request: FrontendRequest = serde_json::from_value(json!({"method":"mesh.send","params":{"peerId":"PRIVATE_PEER","namespace":"org.example.ready","event":"ready","payload":{"secret":"PRIVATE_PAYLOAD"}}})).unwrap();
        assert!(authorize_method(&manifest, &request).is_err());
        manifest.permissions = Some(vec![PermissionNode::MeshEventsSend {
            namespaces: vec!["org.example.ready".into()],
        }]);
        assert!(authorize_method(&manifest, &request).is_ok());
        let access = access_requests(&request);
        assert_eq!(access.len(), 1);
        assert_eq!(access[0].capability, "mesh-events-send");
        assert_eq!(access[0].action, "mesh.send");
        assert!(access[0].game_id.is_none());
        let receive: FrontendRequest = serde_json::from_value(
            json!({"method":"mesh.subscribe","params":{"namespace":"org.example.ready"}}),
        )
        .unwrap();
        assert!(authorize_method(&manifest, &receive).is_err());
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"mesh.receive","params":{"subscriptionId":"id","after":0,"limit":4,"pluginId":"spoof"}})).is_err());
        let access = Access {
            capability: "mesh-events-receive",
            action: "mesh.receive",
            game_id: None,
        };
        assert!(!audit_result(
            &access,
            &Ok(json!({"messages":[],"cursor":0,"missed":false}))
        ));
        assert!(audit_result(
            &access,
            &Ok(json!({"messages":[{"payload":"PRIVATE"}],"cursor":1,"missed":false}))
        ));
        assert!(audit_result(
            &access,
            &Ok(json!({"messages":[],"cursor":1,"missed":true}))
        ));
        assert!(audit_result(
            &access,
            &Err("PLUGIN_PERMISSION_DENIED".into())
        ));
    }
    #[test]
    fn ai_bridge_requires_exact_grants_and_never_accepts_a_request_url() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/ai-assistant/plugin.json"
        ))
        .unwrap();
        let chat: FrontendRequest = serde_json::from_value(json!({"method":"ai.chat","params":{"requestId":"one","messages":[{"role":"user","content":"hi"}]}})).unwrap();
        assert!(authorize_method(&manifest, &chat).is_ok());
        let access = access_requests(&chat);
        assert_eq!(access[0].action, "ai.chat");
        assert!(access[0].game_id.is_none());
        let attached: FrontendRequest = serde_json::from_value(json!({"method":"ai.chat","params":{"requestId":"two","messages":[{"role":"user","content":"hi"}],"diagnostic":{"gameId":"game","diagnosticId":"id","revision":"revision"}}})).unwrap();
        let access = access_requests(&attached);
        assert_eq!(access.len(), 2);
        assert_eq!(access[1].action, "ai.chat.attachDiagnostic");
        assert_eq!(access[1].game_id.as_deref(), Some("game"));
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::AiChat {}));
        assert!(authorize_method(&manifest, &chat).is_err());
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"ai.chat","params":{"requestId":"one","messages":[{"role":"user","content":"hi"}],"url":"https://elsewhere.example"}})).is_err());
    }
    #[test]
    fn generated_blueprints_require_an_independent_grant() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/crash-doctor/plugin.json"
        ))
        .unwrap();
        let request: FrontendRequest = serde_json::from_value(json!({"method":"generatedBlueprint.save","params":{"name":"cube","width":1,"height":1,"length":1,"block":"minecraft:stone","occupancy":"AQ=="}})).unwrap();
        assert!(authorize_method(&manifest, &request).is_err());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::GeneratedBlueprintSave {});
        assert!(authorize_method(&manifest, &request).is_ok());
        assert_eq!(
            access_requests(&request)[0].action,
            "generatedBlueprint.save"
        );
        assert!(access_requests(&request)[0].game_id.is_none());
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"generatedBlueprint.open","params":{"blueprintId":"opaque","path":"../world"}})).is_err());
    }
    #[test]
    fn text_exports_require_an_independent_grant_and_audit_no_text_or_name() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/crash-doctor/plugin.json"
        ))
        .unwrap();
        let request:FrontendRequest=serde_json::from_value(json!({"method":"exports.text","params":{"name":"private-log","text":"private contents"}})).unwrap();
        manifest.permissions = Some(vec![
            PermissionNode::ClipboardWrite {},
            PermissionNode::PrivateStorage {},
        ]);
        assert!(authorize_method(&manifest, &request).is_err());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::TextExport {});
        assert!(authorize_method(&manifest, &request).is_ok());
        let access = access_requests(&request);
        assert_eq!(access.len(), 1);
        assert_eq!(access[0].action, "exports.text");
        assert!(access[0].game_id.is_none());
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"exports.text","params":{"name":"log","text":"x","path":"C:/private"}})).is_err());
        assert_eq!(
            access_outcome(&Ok(json!({"saved":false}))),
            super::super::plugin_access_log::Outcome::Cancelled
        );
    }
    #[test]
    fn world_snapshot_requests_require_separate_grants_and_audit_only_game_scope() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/world-dashboard/plugin.json"
        ))
        .unwrap();
        let read: FrontendRequest = serde_json::from_value(json!({"method":"worldSnapshots.compare","params":{"gameId":"game","worldId":"world","beforeId":"a","afterId":"b","offset":0,"limit":50,"query":"region"}})).unwrap();
        let restore: FrontendRequest = serde_json::from_value(json!({"method":"worldSnapshots.openRestore","params":{"gameId":"game","worldId":"world","snapshotId":"opaque"}})).unwrap();
        assert!(authorize_method(&manifest, &read).is_ok());
        assert!(authorize_method(&manifest, &restore).is_ok());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::WorldSnapshotsRestoreRequest {}));
        assert!(authorize_method(&manifest, &read).is_ok());
        assert!(authorize_method(&manifest, &restore).is_err());
        let accesses = access_requests(&read);
        assert_eq!(accesses[0].game_id.as_deref(), Some("game"));
        assert_eq!(accesses[0].action, "worldSnapshots.compare");
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"worldSnapshots.openRestore","params":{"gameId":"game","worldId":"world","snapshotId":"opaque","replaceExisting":true}})).is_err());
    }
    #[test]
    fn inspection_scope_and_game_are_closed_and_explicitly_authorized() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/crash-doctor/plugin.json"
        ))
        .unwrap();
        manifest.permissions = Some(vec![PermissionNode::GameInspectionRead {
            scopes: vec![InspectionScope::Mods],
        }]);
        let request: FrontendRequest = serde_json::from_value(
            json!({"method":"inspection.snapshot","params":{"gameId":"public","scope":"mods"}}),
        )
        .unwrap();
        assert!(authorize_method(&manifest, &request).is_ok());
        let denied:FrontendRequest=serde_json::from_value(json!({"method":"inspection.check","params":{"gameId":"public","scope":"packs","revision":"hash"}})).unwrap();
        assert!(authorize_method(&manifest, &denied).is_err());
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"inspection.snapshot","params":{"gameId":"public","scope":"accounts"}})).is_err());
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"inspection.snapshot","params":{"gameId":"public","scope":"mods","path":"private"}})).is_err());
        let accesses = access_requests(&request);
        assert_eq!(accesses[0].capability, "game-inspection-read");
        assert_eq!(accesses[0].action, "inspection.snapshot");
        assert!(target(Some("bound"), "public").is_err());
    }
    #[test]
    fn access_metadata_covers_sensitive_methods_without_file_handles_or_revisions() {
        for method in ["diagnostics.list", "configs.list"] {
            let request: FrontendRequest =
                serde_json::from_value(json!({"method":method,"params":{"gameId":"game-public"}}))
                    .unwrap();
            let accesses = access_requests(&request);
            assert_eq!(accesses.len(), 1);
            assert_eq!(accesses[0].action, method);
            assert_eq!(accesses[0].game_id.as_deref(), Some("game-public"));
        }
        let request: FrontendRequest = serde_json::from_value(json!({"method":"diagnostics.check","params":{"gameId":"game-public","diagnosticId":"opaque-handle","revision":"private-revision"}})).unwrap();
        let accesses = access_requests(&request);
        assert_eq!(accesses[0].capability, "game-diagnostics-read");
        assert_eq!(accesses[0].game_id.as_deref(), Some("game-public"));
        let request: FrontendRequest = serde_json::from_value(json!({"method":"keybindings.apply","params":{"sourceGameId":"source","targetGameId":"target","sourceRevision":"private-one","targetRevision":"private-two"}})).unwrap();
        let accesses = access_requests(&request);
        assert_eq!(accesses[0].capability, "game-keybindings-read");
        assert_eq!(accesses[0].game_id.as_deref(), Some("source"));
        assert_eq!(accesses[1].capability, "game-keybindings-write");
        assert_eq!(accesses[1].game_id.as_deref(), Some("target"));
        let request: FrontendRequest =
            serde_json::from_value(json!({"method":"diagnostics.pick","params":{}})).unwrap();
        let accesses = access_requests(&request);
        assert_eq!(accesses[0].capability, "selected-file-read");
        assert!(accesses[0].game_id.is_none());
    }
    #[test]
    fn access_outcomes_do_not_copy_error_details() {
        use super::super::plugin_access_log::Outcome;
        assert_eq!(
            access_outcome(&Err("PLUGIN_PERMISSION_DENIED".into())),
            Outcome::Denied
        );
        assert_eq!(
            access_outcome(&Err("PLUGIN_GAME_SCOPE".into())),
            Outcome::Denied
        );
        assert_eq!(
            access_outcome(&Err("CANCEL_REQUESTED".into())),
            Outcome::Cancelled
        );
        assert_eq!(
            access_outcome(&Err("private filesystem path or token".into())),
            Outcome::Failed
        );
        assert_eq!(access_outcome(&Ok(Value::Null)), Outcome::Completed);
    }
    #[test]
    fn keybinding_writes_require_both_explicit_grants_and_closed_payloads() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/config-doctor/plugin.json"
        ))
        .unwrap();
        let preview: FrontendRequest = serde_json::from_value(json!({"method":"keybindings.preview","params":{"sourceGameId":"a","targetGameId":"b"}})).unwrap();
        let apply: FrontendRequest = serde_json::from_value(json!({"method":"keybindings.apply","params":{"sourceGameId":"a","targetGameId":"b","sourceRevision":"one","targetRevision":"two"}})).unwrap();
        manifest.permissions = Some(vec![PermissionNode::GameConfigRead {}]);
        assert!(authorize_method(&manifest, &preview).is_err());
        manifest.permissions = Some(vec![PermissionNode::GameKeybindingsRead {}]);
        assert!(authorize_method(&manifest, &preview).is_ok());
        assert!(authorize_method(&manifest, &apply).is_err());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::GameKeybindingsWrite {});
        assert!(authorize_method(&manifest, &apply).is_ok());
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"keybindings.restore","params":{"targetGameId":"b","backupId":"opaque","targetRevision":"two","path":"options.txt"}})).is_err());
    }
    #[test]
    fn bridge_wire_rejects_identity_paths_and_unknown_operations() {
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"configs.read","params":{"gameId":"a","configId":"b","path":"C:/secret"}})).is_err());
        assert!(
            serde_json::from_value::<FrontendRequest>(json!({"method":"shell","params":{}}))
                .is_err()
        );
        assert!(
            serde_json::from_value::<FrontendRequest>(
                json!({"method":"context","params":{},"pluginId":"other"})
            )
            .is_err()
        );
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/config-doctor/plugin.json"
        ))
        .unwrap();
        manifest.permissions = Some(vec![]);
        assert!(permission(&manifest).is_err());
        manifest.permissions = Some(vec![PermissionNode::GameConfigRead {}]);
        assert!(permission(&manifest).is_ok());
        assert!(target(Some("a"), "b").is_err());
        assert!(target(None, "a").is_ok());
        assert_eq!(
            safe_error("C:/Users/private.toml".into()),
            "PLUGIN_FRONTEND_FAILED"
        );
    }
    #[test]
    fn selected_report_picker_requires_exact_explicit_extensions_and_closed_input() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/crash-doctor/plugin.json"
        ))
        .unwrap();
        manifest.file_extensions = vec!["json".into()];
        manifest.permissions = Some(vec![
            PermissionNode::UiWebview {},
            PermissionNode::SelectedFileRead {
                extensions: vec!["json".into()],
            },
        ]);
        let request: FrontendRequest =
            serde_json::from_value(json!({"method":"report.pick","params":{}})).unwrap();
        assert!(authorize_method(&manifest, &request).is_ok());
        let diagnostic: FrontendRequest =
            serde_json::from_value(json!({"method":"diagnostics.pick","params":{}})).unwrap();
        assert!(authorize_method(&manifest, &diagnostic).is_err());
        assert_eq!(access_requests(&request)[0].action, "report.pick");
        manifest.permissions = Some(vec![PermissionNode::SelectedFileRead {
            extensions: vec!["txt".into()],
        }]);
        assert!(authorize_method(&manifest, &request).is_err());
        manifest.file_extensions.clear();
        assert!(authorize_method(&manifest, &request).is_err());
        assert!(
            serde_json::from_value::<FrontendRequest>(
                json!({"method":"report.pick","params":{"path":"/private"}})
            )
            .is_err()
        );
    }
    #[test]
    fn world_seed_requires_separate_grant_and_audits_no_seed_world_name_or_path() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/world-dashboard/plugin.json"
        ))
        .unwrap();
        let request: FrontendRequest = serde_json::from_value(
            json!({"method":"worlds.metadata","params":{"gameId":"game","worldId":"My world"}}),
        )
        .unwrap();
        assert!(authorize_method(&manifest, &request).is_err());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::WorldSeedRead {});
        assert!(authorize_method(&manifest, &request).is_ok());
        let accesses = access_requests(&request);
        assert_eq!(accesses.len(), 1);
        assert_eq!(accesses[0].capability, "worlds-seed-read");
        assert_eq!(accesses[0].game_id.as_deref(), Some("game"));
        assert!(snapshots::ids(Some("other"), "game", "My world").is_err());
        assert!(snapshots::ids(None, "game", "../private").is_err());
        assert!(serde_json::from_value::<FrontendRequest>(json!({"method":"worlds.metadata","params":{"gameId":"game","worldId":"World","path":"/private"}})).is_err());
        let pure: FrontendRequest =
            serde_json::from_value(json!({"method":"seedMap.capabilities","params":{}})).unwrap();
        manifest.permissions = Some(vec![]);
        assert!(authorize_method(&manifest, &pure).is_ok());
        assert!(access_requests(&pure).is_empty());
    }
    #[test]
    fn ui_only_context_and_cancel_do_not_require_config_access_and_english_is_canonical() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/config-doctor/plugin.json"
        ))
        .unwrap();
        manifest.permissions = Some(vec![PermissionNode::UiWebview {}]);
        for method in ["context", "report.cancel"] {
            let request: FrontendRequest =
                serde_json::from_value(json!({"method":method,"params":{}})).unwrap();
            assert!(authorize_method(&manifest, &request).is_ok());
        }
        let request: FrontendRequest = serde_json::from_value(
            json!({"method":"configs.list","params":{"gameId":"registered"}}),
        )
        .unwrap();
        assert!(authorize_method(&manifest, &request).is_err());
        assert_eq!(report_locale("en-US"), "en");
        assert_eq!(report_locale("en"), "en");
        assert_eq!(report_locale("zh-CN"), "zh-CN");
    }
}
