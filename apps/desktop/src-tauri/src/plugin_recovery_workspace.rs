// SPDX-License-Identifier: MIT
//! Read-only recovery evidence and requests to the host's existing confirmation flows.
use super::plugin_views::ViewAuthorization;
use cobble_plugin_api::PermissionNode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter};
#[path = "plugin_recovery_history.rs"]
mod history;
#[path = "plugin_recovery_receipts.rs"]
mod receipts;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSummary {
    java_mode: String,
    selected_java_major: Option<u32>,
    memory_mb: u32,
    jvm_argument_count: usize,
    settings_revision: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoaderSummary {
    minecraft_version: String,
    kind: Option<String>,
    version: Option<String>,
}
impl LoaderSummary {
    fn valid(&self) -> bool {
        [
            &self.minecraft_version,
            self.kind.as_deref().unwrap_or(""),
            self.version.as_deref().unwrap_or(""),
        ]
        .iter()
        .all(|value| {
            value.len() <= 120
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
                })
        })
    }
}
pub fn loader_summary(
    minecraft_version: String,
    kind: Option<String>,
    version: Option<String>,
) -> LoaderSummary {
    LoaderSummary {
        minecraft_version,
        kind,
        version,
    }
}
impl RuntimeSummary {
    fn valid(&self) -> bool {
        matches!(self.java_mode.as_str(), "inherit" | "automatic" | "custom")
            && self.memory_mb > 0
            && self.jvm_argument_count <= 128
            && self
                .selected_java_major
                .is_none_or(|major| (1..=1024).contains(&major))
            && self.settings_revision.len() == 64
            && self
                .settings_revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
    }
    fn public(&self) -> Value {
        json!({"javaMode":self.java_mode,"selectedJavaMajor":self.selected_java_major,"memoryMb":self.memory_mb,"jvmArgumentCount":self.jvm_argument_count})
    }
}
/// Capture observable configuration at launch preparation, without persisting paths or arguments.
pub fn capture_runtime(
    overrides: &cobble_minecraft::LaunchOverrides,
    selected_java_major: Option<u32>,
) -> Result<RuntimeSummary, String> {
    let defaults = cobble_minecraft::read_launcher_preferences()
        .map_err(|_| "RECOVERY_RUNTIME_UNAVAILABLE")?;
    let java = cobble_core::capture_java_choice(&overrides.java)
        .unwrap_or_else(|_| overrides.java.clone());
    let memory_mb = overrides.memory_mb.unwrap_or(defaults.memory_mb);
    let arguments = overrides.jvm_args.as_ref().unwrap_or(&defaults.jvm_args);
    let java_mode = match &overrides.java {
        cobble_minecraft::JavaOverride::Inherit => "inherit",
        cobble_minecraft::JavaOverride::Automatic => "automatic",
        cobble_minecraft::JavaOverride::Custom { .. } => "custom",
    };
    let bytes = serde_json::to_vec(&json!({"java":java,"memoryMb":memory_mb,"jvmArgs":arguments}))
        .map_err(|_| "RECOVERY_RUNTIME_UNAVAILABLE")?;
    Ok(RuntimeSummary {
        java_mode: java_mode.into(),
        selected_java_major,
        memory_mb,
        jvm_argument_count: arguments.len(),
        settings_revision: cobble_plugin_host::digest(&bytes),
    })
}
/// Called only by the owned-process exit-0 hook. Historical launches are not reconstructed.
pub fn record_clean_exit(
    game_id: &str,
    started_at_ms: u64,
    ended_at_ms: u64,
    summary: RuntimeSummary,
    loader: LoaderSummary,
) -> Result<(), String> {
    let root = receipts::prepare_root()?;
    receipts::write_at(
        &root,
        receipts::Receipt {
            schema_version: 2,
            game_id: game_id.into(),
            started_at: started_at_ms,
            ended_at: ended_at_ms,
            runtime: summary,
            loader: Some(loader),
        },
    )?;
    receipts::clear_failed_at(&root, game_id, ended_at_ms)
}
/// Public timing metadata for the native maintenance list; no runtime settings or paths.
pub(crate) fn last_clean_timing(game_id: &str) -> Result<Option<(u64, u64)>, String> {
    Ok(receipts::read_at(&receipts::root()?, game_id)?
        .map(|receipt| (receipt.started_at, receipt.ended_at)))
}
pub fn record_failed_attempt(
    game_id: &str,
    at: u64,
    runtime: Option<RuntimeSummary>,
    loader: LoaderSummary,
) -> Result<(), String> {
    receipts::write_failed_at(
        &receipts::prepare_root()?,
        receipts::FailedAttempt {
            schema_version: 1,
            game_id: game_id.into(),
            at,
            runtime,
            loader,
        },
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameTarget {
    pub game_id: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RecoveryTarget {
    #[serde(rename = "modUndo")]
    ModUndo {
        #[serde(rename = "operationId")]
        operation_id: String,
    },
    #[serde(rename = "configRestore")]
    ConfigRestore {
        #[serde(rename = "configId")]
        config_id: String,
        #[serde(rename = "backupId")]
        backup_id: String,
    },
    #[serde(rename = "runtimeSettings")]
    RuntimeSettings {},
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenRequest {
    pub game_id: String,
    pub revision: String,
    pub target: RecoveryTarget,
}
fn authorize(auth: &ViewAuthorization, game_id: &str, open: bool) -> Result<(), String> {
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    if game_id.is_empty()
        || game_id.len() > 128
        || game_id.chars().any(char::is_control)
        || auth
            .instance_id
            .as_deref()
            .is_some_and(|bound| bound != game_id)
    {
        return Err("PLUGIN_GAME_SCOPE".into());
    }
    let nodes = auth.manifest.permissions.as_deref().unwrap_or_default();
    if !nodes.contains(&PermissionNode::GameRecoveryRead {})
        || open && !nodes.contains(&PermissionNode::GameRecoveryOpenRequest {})
    {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    Ok(())
}
fn bounded(value: Value) -> Result<Value, String> {
    if serde_json::to_vec(&value)
        .map_err(|_| "RECOVERY_UNAVAILABLE")?
        .len()
        > 1024 * 1024
    {
        return Err("RECOVERY_SNAPSHOT_LIMIT".into());
    }
    Ok(value)
}
fn comparison_value(
    baseline: Option<&receipts::Receipt>,
    attempt: Option<&receipts::FailedAttempt>,
    current_failure: bool,
    mods: &Value,
    configs: &Value,
) -> Value {
    let empty = |reason: &str| json!({"available":false,"reason":reason,"baselineEndedAt":baseline.map(|r|r.ended_at),"failureAt":Value::Null,"runtime":{"changed":null,"before":null,"after":null},"loader":{"changed":null,"before":null,"after":null},"modOperations":[],"configBackups":[],"limited":false});
    let Some(baseline) = baseline else {
        return empty("noBaseline");
    };
    if !current_failure {
        return empty("noFailedAttempt");
    }
    let Some(attempt) = attempt else {
        return empty("noFailedAttempt");
    };
    if attempt.at <= baseline.ended_at {
        return empty("baselineAfterFailure");
    }
    let operations = mods["items"].as_array().map(|items| items.iter().filter(|item| item["createdAt"].as_u64().is_some_and(|at| at > baseline.ended_at && at <= attempt.at)).take(64).map(|item| json!({"id":item["id"],"createdAt":item["createdAt"],"status":item["status"],"statusAtFailure":"unknown","files":item["files"]})).collect::<Vec<_>>()).unwrap_or_default();
    let mut backups = Vec::new();
    if let Some(files) = configs["files"].as_array() {
        for file in files {
            if let Some(items) = file["backups"].as_array() {
                for item in items {
                    if item["createdAt"]
                        .as_u64()
                        .is_some_and(|at| at > baseline.ended_at && at <= attempt.at)
                    {
                        backups.push(json!({"configId":file["id"],"name":file["name"],"backupId":item["id"],"createdAt":item["createdAt"]}));
                    }
                }
            }
        }
    }
    backups.sort_by_key(|item| std::cmp::Reverse(item["createdAt"].as_u64().unwrap_or_default()));
    let limited = mods["available"] != true
        || mods["truncated"] == true
        || configs["available"] != true
        || configs["truncated"] == true
        || backups.len() > 64;
    backups.truncate(64);
    let runtime_before = baseline.runtime.public();
    let runtime_after = attempt.runtime.as_ref().map(RuntimeSummary::public);
    let loader_before = baseline.loader.as_ref().map(|loader| json!(loader));
    json!({"available":true,"reason":null,"baselineEndedAt":baseline.ended_at,"failureAt":attempt.at,
        "runtime":{"changed":attempt.runtime.as_ref().map(|runtime|runtime.settings_revision != baseline.runtime.settings_revision),"before":runtime_before,"after":runtime_after},
        "loader":{"changed":baseline.loader.as_ref().map(|loader|loader.minecraft_version != attempt.loader.minecraft_version || loader.kind != attempt.loader.kind || loader.version != attempt.loader.version),"before":loader_before,"after":attempt.loader},
        "modOperations":operations,"configBackups":backups,"limited":limited})
}
async fn snapshot_value(app: &AppHandle, game_id: &str) -> Result<Value, String> {
    let directory =
        crate::games::game_directory(app, game_id).map_err(|_| "RECOVERY_GAME_UNAVAILABLE")?;
    let runtime = crate::games::game_inspection::data_snapshot(app, game_id, "runtime")
        .await
        .map_err(|_| "RECOVERY_RUNTIME_UNAVAILABLE")?;
    let item = |name: &str| {
        runtime["items"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["name"] == name))
            .map(|item| item["value"].clone())
            .unwrap_or(Value::Null)
    };
    let current = capture_runtime(
        &crate::games::runtime::recovery_overrides(app, game_id)?,
        item("selectedJavaMajor")
            .as_u64()
            .and_then(|n| u32::try_from(n).ok()),
    )?;
    let current_failure = crate::games::runtime::recovery_has_failure(app, game_id)?;
    let selected = game_id.to_owned();
    let metadata = tauri::async_runtime::spawn_blocking(move || {
        let baseline = receipts::read_at(&receipts::root()?, &selected);
        let attempt = receipts::read_failed_at(&receipts::root()?, &selected);
        let mods = history::read(&directory)
            .unwrap_or_else(|_| json!({"available":false,"truncated":false,"items":[]}));
        let configs = crate::games::game_config::recovery_metadata(&directory)
            .ok()
            .and_then(|value| serde_json::to_value(value).ok())
            .map(|mut value| {
                value["available"] = json!(true);
                value
            })
            .unwrap_or_else(|| json!({"available":false,"truncated":false,"files":[]}));
        Ok::<_, String>((baseline, attempt, mods, configs))
    })
    .await
    .map_err(|_| "RECOVERY_UNAVAILABLE")??;
    let (baseline, attempt, mods, configs) = metadata;
    let baseline_available = baseline.is_ok();
    let attempt_available = attempt.is_ok();
    let baseline = baseline.ok().flatten();
    let attempt = attempt.ok().flatten();
    let mut comparison = comparison_value(
        baseline.as_ref(),
        attempt.as_ref(),
        current_failure,
        &mods,
        &configs,
    );
    if !baseline_available {
        comparison["reason"] = json!("baselineUnavailable");
    } else if !attempt_available && current_failure && baseline.is_some() {
        comparison["reason"] = json!("failedAttemptUnavailable");
    }
    let last = baseline.as_ref().map(|value| json!({"startedAt":value.started_at,"endedAt":value.ended_at,"runtime":value.runtime.public()}));
    let mut public_runtime = current.public();
    public_runtime["changedSinceClean"] = json!(
        baseline
            .as_ref()
            .map(|value| value.runtime.settings_revision != current.settings_revision)
    );
    let mut value = json!({"schemaVersion":2,"gameId":game_id,"baselineAvailable":baseline_available,"lastCleanLaunch":last,"runtime":public_runtime,"launchState":item("launchState"),"mods":mods,"configs":configs,"comparison":comparison});
    let revision = cobble_plugin_host::digest(
        &serde_json::to_vec(&(&value, &current.settings_revision))
            .map_err(|_| "RECOVERY_UNAVAILABLE")?,
    );
    value["revision"] = json!(revision);
    bounded(value)
}
pub async fn snapshot(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: GameTarget,
) -> Result<Value, String> {
    authorize(auth, &args.game_id, false)?;
    let value = snapshot_value(&app, &args.game_id).await?;
    authorize(auth, &args.game_id, false)?;
    Ok(value)
}
fn exists(value: &Value, target: &RecoveryTarget) -> bool {
    match target {
        RecoveryTarget::RuntimeSettings {} => true,
        RecoveryTarget::ModUndo { operation_id } => {
            value["mods"]["items"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["id"] == *operation_id && item["status"] == "applied")
            })
        }
        RecoveryTarget::ConfigRestore {
            config_id,
            backup_id,
        } => value["configs"]["files"].as_array().is_some_and(|files| {
            files.iter().any(|file| {
                file["id"] == *config_id
                    && file["backups"]
                        .as_array()
                        .is_some_and(|items| items.iter().any(|item| item["id"] == *backup_id))
            })
        }),
    }
}
pub async fn open_request(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: OpenRequest,
) -> Result<Value, String> {
    authorize(auth, &args.game_id, true)?;
    if args.revision.len() != 64 || !args.revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("RECOVERY_SNAPSHOT_CHANGED".into());
    }
    let value = snapshot_value(&app, &args.game_id).await?;
    if value["revision"] != args.revision {
        return Err("RECOVERY_SNAPSHOT_CHANGED".into());
    }
    if value["launchState"] != "idle" {
        return Err("RECOVERY_GAME_BUSY".into());
    }
    if !exists(&value, &args.target) {
        return Err("RECOVERY_TARGET_UNAVAILABLE".into());
    }
    authorize(auth, &args.game_id, true)?;
    app.emit_to(
        "main",
        "plugin-open-game-recovery",
        json!({"game_id":args.game_id,"plugin_id":auth.manifest.id,"target":args.target}),
    )
    .map_err(|_| "RECOVERY_UNAVAILABLE")?;
    Ok(json!({"requested":true}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparison_uses_failed_attempt_evidence_and_bounded_time_window() {
        assert!(!loader_summary("C:/private/game".into(), None, None).valid());
        let runtime = RuntimeSummary {
            java_mode: "automatic".into(),
            selected_java_major: Some(21),
            memory_mb: 2048,
            jvm_argument_count: 0,
            settings_revision: "a".repeat(64),
        };
        let clean = receipts::Receipt {
            schema_version: 2,
            game_id: "test".into(),
            started_at: 100,
            ended_at: 200,
            runtime: runtime.clone(),
            loader: Some(loader_summary(
                "1.21.1".into(),
                Some("fabric".into()),
                Some("0.16".into()),
            )),
        };
        let failed = receipts::FailedAttempt {
            schema_version: 1,
            game_id: "test".into(),
            at: 400,
            runtime: Some(RuntimeSummary {
                selected_java_major: Some(22),
                settings_revision: "b".repeat(64),
                ..runtime
            }),
            loader: loader_summary("1.21.1".into(), Some("fabric".into()), Some("0.17".into())),
        };
        let mods = json!({"available":true,"truncated":false,"items":[{"id":"before","createdAt":199,"status":"applied","files":[]},{"id":"between","createdAt":250,"status":"undone","files":[{"name":"Mod","before":"old.jar","after":"new.jar"}]},{"id":"after","createdAt":401,"status":"applied","files":[]}]});
        let configs = json!({"available":true,"truncated":false,"files":[{"id":"config","name":"client.toml","backups":[{"id":"one","createdAt":300},{"id":"two","createdAt":401}]}]});
        let value = comparison_value(Some(&clean), Some(&failed), true, &mods, &configs);
        assert_eq!(value["available"], true);
        assert_eq!(value["runtime"]["changed"], true);
        assert_eq!(value["loader"]["changed"], true);
        assert_eq!(value["modOperations"].as_array().unwrap().len(), 1);
        assert_eq!(value["modOperations"][0]["statusAtFailure"], "unknown");
        assert_eq!(value["configBackups"].as_array().unwrap().len(), 1);
        assert_eq!(
            comparison_value(Some(&clean), Some(&failed), false, &mods, &configs)["available"],
            false
        );
        assert_eq!(
            comparison_value(None, Some(&failed), true, &mods, &configs)["reason"],
            "noBaseline"
        );
    }
    #[test]
    fn targets_are_closed_and_known_history_only() {
        assert!(serde_json::from_value::<OpenRequest>(json!({"gameId":"g","revision":"a".repeat(64),"target":{"kind":"modUndo","operationId":"known","path":"/private"}})).is_err());
        let value = json!({"mods":{"items":[{"id":"known","status":"applied"}]},"configs":{"files":[{"id":"config","backups":[{"id":"backup"}]}]}});
        assert!(exists(
            &value,
            &RecoveryTarget::ModUndo {
                operation_id: "known".into()
            }
        ));
        assert!(!exists(
            &value,
            &RecoveryTarget::ModUndo {
                operation_id: "forged".into()
            }
        ));
        assert!(exists(
            &value,
            &RecoveryTarget::ConfigRestore {
                config_id: "config".into(),
                backup_id: "backup".into()
            }
        ));
        assert!(!exists(
            &value,
            &RecoveryTarget::ConfigRestore {
                config_id: "config".into(),
                backup_id: "other".into()
            }
        ));
    }
}
#[cfg(test)]
#[path = "plugin_recovery_authorization_tests.rs"]
mod authorization_tests;
