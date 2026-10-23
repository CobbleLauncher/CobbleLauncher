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
#[derive(Deserialize)]
#[serde(tag = "method", content = "params", deny_unknown_fields)]
pub enum FrontendRequest {
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
fn authorize_method(manifest: &PluginManifest, request: &FrontendRequest) -> Result<(), String> {
    match request {
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
        FrontendRequest::Context(_) | FrontendRequest::Cancel(_) => vec![],
    }
}
fn access_outcome(result: &Result<Value, String>) -> super::plugin_access_log::Outcome {
    use super::plugin_access_log::Outcome;
    match result {
        Ok(_) => Outcome::Completed,
        Err(code)
            if matches!(
                code.as_str(),
                "PLUGIN_PERMISSION_DENIED"
                    | "PLUGIN_GAME_SCOPE"
                    | "FOLDER_REPORT_PERMISSION_REQUIRED"
            ) =>
        {
            Outcome::Denied
        }
        Err(code) if matches!(code.as_str(), "CANCEL_REQUESTED" | "PLUGIN_VIEW_CLOSED") => {
            Outcome::Cancelled
        }
        Err(_) => Outcome::Failed,
    }
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
        FrontendRequest::Context(_) => {
            let mut inspection_games=if auth.manifest.permissions.as_deref().unwrap_or_default().iter().any(|node|matches!(node,PermissionNode::GameInspectionRead {scopes} if !scopes.is_empty())){crate::games::frontend_game_summaries(&app)?}else{vec![]};
            if let Some(id)=&auth.instance_id {inspection_games.retain(|game|game["id"].as_str()==Some(id));}
            let mut games = if permission(&auth.manifest).is_ok() {
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
                json!({"locale":locale,"theme":auth.theme,"instanceId":auth.instance_id,"keybindingTargetId":auth.keybinding_target_id,"games":games,"keybindingGames":keybinding_games,"inspectionGames":inspection_games,"recentGames":if diagnostic_permission(&auth.manifest).is_ok() || keybinding_permission(&auth.manifest, false).is_ok() { crate::games::game_diagnostics::recent(&app, auth.instance_id.as_deref())? } else { vec![] }}),
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
