// SPDX-License-Identifier: MIT
//! Crash Doctor consumes verified trial copies; no source Mod is changed here.
use super::*;
use std::collections::{BTreeSet, HashSet};
use std::io::{Read, Write};
#[path = "trial_experiment_graph.rs"]
mod graph;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewExperimentRequest {
    pub game_id: String,
    pub mod_ids: Vec<String>,
    #[serde(default)]
    pub include_worlds: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CleanupRequest {
    pub game_id: String,
    pub copy_game_id: String,
}
fn basename(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && !id.contains(['/', '\\', ':'])
        && !id.chars().any(char::is_control)
        && id.ends_with(".jar")
}
fn regular(path: &std::path::Path, directory: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "EXPERIMENT_COPY_CHANGED")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("EXPERIMENT_COPY_CHANGED".into());
        }
    }
    if metadata.file_type().is_symlink()
        || (if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        })
    {
        return Err("EXPERIMENT_COPY_CHANGED".into());
    }
    Ok(())
}
/// Reverse required edges reach all dependents, including every member of a cycle.
fn closure(value: &Value, requested: &[String]) -> Result<Vec<String>, String> {
    let items = value["items"]
        .as_array()
        .ok_or("EXPERIMENT_METADATA_UNAVAILABLE")?;
    if value["available"] != true || value["truncated"] == true || items.len() > 512 {
        return Err("EXPERIMENT_METADATA_UNAVAILABLE".into());
    }
    let enabled = items
        .iter()
        .filter(|item| item["disabled"] == false)
        .collect::<Vec<_>>();
    if enabled.iter().any(|item| {
        item["declaredIds"].as_array().is_none_or(Vec::is_empty)
            || item["dependencies"]
                .as_array()
                .is_none_or(|items| items.len() > 512)
    }) {
        return Err("EXPERIMENT_DEPENDENCY_METADATA_UNAVAILABLE".into());
    }
    let mut disabled = BTreeSet::new();
    for id in requested {
        if !basename(id) || !enabled.iter().any(|item| item["id"] == *id) {
            return Err("EXPERIMENT_MOD_UNAVAILABLE".into());
        }
        disabled.insert(id.clone());
    }
    loop {
        let aliases = enabled
            .iter()
            .filter(|item| item["id"].as_str().is_some_and(|id| disabled.contains(id)))
            .flat_map(|item| {
                item["declaredIds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
            })
            .collect::<HashSet<_>>();
        let dependents = enabled
            .iter()
            .filter(|item| {
                item["dependencies"].as_array().is_some_and(|deps| {
                    deps.iter().any(|dep| {
                        dep["optional"] != true
                            && dep["id"].as_str().is_some_and(|id| aliases.contains(id))
                    })
                })
            })
            .filter_map(|item| item["id"].as_str())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let before = disabled.len();
        disabled.extend(dependents);
        if disabled.len() == before {
            break;
        }
    }
    Ok(disabled.into_iter().collect())
}
pub async fn preview(
    app: AppHandle,
    auth: &Auth,
    args: PreviewExperimentRequest,
) -> Result<Value, String> {
    authorize(auth, Some(&args.game_id))?;
    let permissions = auth.manifest.permissions.as_deref().unwrap_or_default();
    if !permissions.contains(&PermissionNode::GameRecoveryRead {})
        || !permissions.iter().any(|node| matches!(node, PermissionNode::GameInspectionRead { scopes } if scopes.contains(&InspectionScope::Mods))) {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    if args.mod_ids.is_empty() || args.mod_ids.len() > 512 {
        return Err("EXPERIMENT_MOD_UNAVAILABLE".into());
    }
    let name = workspace::snapshot(&app, &args.game_id)?.name;
    let suffix = if auth.locale == "zh-CN" {
        " · 诊断试验"
    } else {
        " · diagnosis trial"
    };
    let name = format!("{}{suffix}", name.chars().take(55).collect::<String>());
    let mut value = super::preview(
        app.clone(),
        auth,
        PreviewRequest {
            game_id: args.game_id.clone(),
            name,
            include_worlds: args.include_worlds,
        },
    )
    .await?;
    let plan_id = value["planId"]
        .as_str()
        .ok_or("TRIAL_COPY_PLAN_EXPIRED")?
        .to_owned();
    let p = plan(auth, &plan_id)?;
    let metadata =
        crate::games::game_inspection::data_snapshot(&app, &args.game_id, "mods").await?;
    let (current, _lease) = workspace::acquire(&app, &args.game_id)?;
    if current.directory != p.game.directory {
        return Err("TRIAL_COPY_STALE".into());
    }
    let cancel = auth.cancel.clone();
    let source = p.game.directory.clone();
    let worlds = p.include_worlds;
    let (metadata, inventory) = tauri::async_runtime::spawn_blocking(move || {
        let metadata = graph::complete(&source, metadata, &cancel)?;
        let inventory = disk::inventory_instance(&source, worlds, &cancel).map_err(safe_error)?;
        Ok::<_, String>((metadata, inventory))
    })
    .await
    .map_err(|_| "TRIAL_COPY_IO_FAILED")??;
    if inventory != p.inventory {
        return Err("TRIAL_COPY_STALE".into());
    }
    let disabled = closure(&metadata, &args.mod_ids)?;
    for id in &disabled {
        if !basename(id)
            || !inventory
                .files
                .contains_key(&PathBuf::from("mods").join(id))
            || inventory
                .files
                .contains_key(&PathBuf::from("mods").join(format!("{id}.disabled")))
        {
            return Err("EXPERIMENT_MOD_UNAVAILABLE".into());
        }
    }
    authorize(auth, Some(&args.game_id))?;
    let names = metadata["items"]
        .as_array()
        .ok_or("EXPERIMENT_METADATA_UNAVAILABLE")?;
    value["disabledMods"] = json!(disabled.iter().map(|id| json!({"id":id,"name":names.iter().find(|item| item["id"] == *id).map(|item|item["name"].clone()).unwrap_or_else(||json!(id))})).collect::<Vec<_>>());
    value["requestedModIds"] = json!(args.mod_ids);
    value["copyGameId"] = json!(p.new_id);
    let mut s = store().lock().map_err(|_| "TRIAL_COPY_UNAVAILABLE")?;
    let stored = s.plans.get_mut(&plan_id).ok_or("TRIAL_COPY_PLAN_EXPIRED")?;
    stored.disabled_mods = disabled;
    Ok(value)
}
pub(super) fn apply(p: &Plan) -> Result<(), String> {
    if p.disabled_mods.is_empty() {
        return Ok(());
    }
    regular(&p.target, true)?;
    regular(&p.target.join("mods"), true)?;
    let target = fs::canonicalize(&p.target).map_err(|_| "EXPERIMENT_COPY_CHANGED")?;
    let mods = fs::canonicalize(p.target.join("mods")).map_err(|_| "EXPERIMENT_COPY_CHANGED")?;
    if mods.parent() != Some(target.as_path()) {
        return Err("EXPERIMENT_COPY_CHANGED".into());
    }
    for id in &p.disabled_mods {
        if !basename(id) {
            return Err("EXPERIMENT_MOD_UNAVAILABLE".into());
        }
        let source = mods.join(id);
        regular(&source, false)?;
        let metadata = fs::symlink_metadata(&source).map_err(|_| "EXPERIMENT_COPY_CHANGED")?;
        let destination = mods.join(format!("{id}.disabled"));
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || destination.exists()
            || fs::canonicalize(&source)
                .map_err(|_| "EXPERIMENT_COPY_CHANGED")?
                .parent()
                != Some(mods.as_path())
        {
            return Err("EXPERIMENT_COPY_CHANGED".into());
        }
        fs::rename(&source, destination).map_err(|_| "EXPERIMENT_COPY_CHANGED")?;
    }
    Ok(())
}
fn receipt_path(copy_id: &str) -> Result<PathBuf, String> {
    if copy_id.len() != 32 || !copy_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("EXPERIMENT_COPY_UNKNOWN".into());
    }
    let library = library_file().map_err(|_| "EXPERIMENT_UNAVAILABLE")?;
    let path = library
        .parent()
        .ok_or("EXPERIMENT_UNAVAILABLE")?
        .join("trial-receipts")
        .join(format!("{copy_id}.json"));
    let parent = path.parent().ok_or("EXPERIMENT_UNAVAILABLE")?;
    if parent.exists() {
        regular(parent, true)?;
    }
    if path.exists() {
        regular(&path, false)?;
    }
    Ok(path)
}
pub(super) fn record(p: &Plan) -> Result<(), String> {
    if p.disabled_mods.is_empty() {
        return Ok(());
    }
    let path = receipt_path(&p.new_id)?;
    let parent = path.parent().ok_or("EXPERIMENT_UNAVAILABLE")?;
    fs::create_dir_all(parent).map_err(|_| "EXPERIMENT_UNAVAILABLE")?;
    let mut pending =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "EXPERIMENT_UNAVAILABLE")?;
    let value = json!({"owner":p.owner,"source":p.game.id,"copy":p.new_id,"directory":p.target});
    pending
        .write_all(&serde_json::to_vec(&value).map_err(|_| "EXPERIMENT_UNAVAILABLE")?)
        .map_err(|_| "EXPERIMENT_UNAVAILABLE")?;
    pending
        .as_file()
        .sync_all()
        .map_err(|_| "EXPERIMENT_UNAVAILABLE")?;
    pending
        .persist(path)
        .map_err(|_| "EXPERIMENT_UNAVAILABLE")?;
    Ok(())
}
pub async fn cleanup(app: AppHandle, auth: &Auth, args: CleanupRequest) -> Result<Value, String> {
    authorize(auth, Some(&args.game_id))?;
    if args.game_id == args.copy_game_id {
        return Err("EXPERIMENT_COPY_UNKNOWN".into());
    }
    let path = receipt_path(&args.copy_game_id)?;
    let mut bytes = vec![];
    fs::File::open(&path)
        .map_err(|_| "EXPERIMENT_COPY_UNKNOWN")?
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| "EXPERIMENT_COPY_UNKNOWN")?;
    if bytes.len() > 8192 {
        return Err("EXPERIMENT_COPY_UNKNOWN".into());
    }
    let receipt: Value = serde_json::from_slice(&bytes).map_err(|_| "EXPERIMENT_COPY_UNKNOWN")?;
    let copy = workspace::snapshot(&app, &args.copy_game_id)?;
    if receipt["owner"] != auth.manifest.id
        || receipt["source"] != args.game_id
        || receipt["copy"] != args.copy_game_id
        || receipt["directory"] != json!(copy.directory)
    {
        return Err("EXPERIMENT_COPY_UNKNOWN".into());
    }
    if !crate::plugins::plugin_confirm::ask(
        &app,
        auth,
        if auth.locale == "zh-CN" {
            "删除诊断试验副本"
        } else {
            "Delete diagnosis trial copy"
        },
        &format!(
            "{}\n{}",
            copy.name,
            if auth.locale == "zh-CN" {
                "将永久删除此副本及其中的存档。"
            } else {
                "Permanently delete this copy and its world saves."
            }
        ),
    )
    .await?
    {
        return Ok(json!({"removed":false}));
    }
    authorize(auth, Some(&args.game_id))?;
    crate::games::management::remove_game(app, args.copy_game_id, true)
        .await
        .map_err(|_| "EXPERIMENT_CLEANUP_FAILED")?;
    let _ = fs::remove_file(path);
    Ok(json!({"removed":true}))
}
pub async fn open(app: AppHandle, auth: &Auth, args: CleanupRequest) -> Result<Value, String> {
    authorize(auth, Some(&args.game_id))?;
    if args.game_id == args.copy_game_id {
        return Err("EXPERIMENT_COPY_UNKNOWN".into());
    }
    let path = receipt_path(&args.copy_game_id)?;
    let mut bytes = vec![];
    fs::File::open(&path)
        .map_err(|_| "EXPERIMENT_COPY_UNKNOWN")?
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| "EXPERIMENT_COPY_UNKNOWN")?;
    if bytes.len() > 8192 {
        return Err("EXPERIMENT_COPY_UNKNOWN".into());
    }
    let receipt: Value = serde_json::from_slice(&bytes).map_err(|_| "EXPERIMENT_COPY_UNKNOWN")?;
    let copy = workspace::snapshot(&app, &args.copy_game_id)?;
    if receipt["owner"] != auth.manifest.id
        || receipt["source"] != args.game_id
        || receipt["copy"] != args.copy_game_id
        || receipt["directory"] != json!(copy.directory)
    {
        return Err("EXPERIMENT_COPY_UNKNOWN".into());
    }
    app.emit_to(
        "main",
        "plugin-game-open-request",
        json!({"gameId":args.copy_game_id}),
    )
    .map_err(|_| "EXPERIMENT_UNAVAILABLE")?;
    Ok(json!({"requested":true}))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: &str, deps: &[&str]) -> Value {
        json!({"id":format!("{id}.jar"),"disabled":false,"declaredIds":[id],"dependencies":deps.iter().map(|id|json!({"id":id,"optional":false})).collect::<Vec<_>>()})
    }
    #[test]
    fn cycles_and_transitive_dependents_are_indivisible() {
        let value = json!({"available":true,"truncated":false,"items":[item("a", &["b"]),item("b", &["a"]),item("c", &["a"]),item("independent", &[])]});
        assert_eq!(
            closure(&value, &["a.jar".into()]).unwrap(),
            vec!["a.jar", "b.jar", "c.jar"]
        );
        assert!(closure(&value, &["../a.jar".into()]).is_err());
    }
    #[test]
    fn partial_or_unknown_metadata_cannot_claim_dependency_safe_trial() {
        let mut value = json!({"available":true,"truncated":false,"items":[item("a", &[])]});
        value["items"][0]["declaredIds"] = json!([]);
        assert_eq!(
            closure(&value, &["a.jar".into()]).unwrap_err(),
            "EXPERIMENT_DEPENDENCY_METADATA_UNAVAILABLE"
        );
        value["truncated"] = json!(true);
        assert!(closure(&value, &["a.jar".into()]).is_err());
    }
    #[test]
    fn experiment_disables_independent_copy_only_and_worlds_stay_excluded() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("copy");
        fs::create_dir_all(source.join("mods")).unwrap();
        fs::create_dir_all(source.join("saves/world")).unwrap();
        fs::write(source.join("mods/a.jar"), b"original Mod bytes").unwrap();
        fs::write(source.join("saves/world/level.dat"), b"original world").unwrap();
        let cancel = AtomicBool::new(false);
        let inventory = disk::inventory_instance(&source, false, &cancel).unwrap();
        disk::copy_instance_verified(&source, &target, &inventory, false, &cancel, |_| {}).unwrap();
        let plan = Plan {
            owner: "crash-doctor".into(),
            session: "fixture".into(),
            game: workspace::GameSnapshot {
                id: "source".into(),
                name: "Source".into(),
                version: "1.21.1".into(),
                loader_kind: Some("fabric".into()),
                loader_version: Some("0.16".into()),
                directory: source.clone(),
            },
            name: "Copy".into(),
            include_worlds: false,
            target: target.clone(),
            new_id: "a".repeat(32),
            inventory: inventory.clone(),
            disabled_mods: vec!["a.jar".into()],
            created: Instant::now(),
        };
        apply(&plan).unwrap();
        assert!(target.join("mods/a.jar.disabled").is_file());
        assert!(!target.join("mods/a.jar").exists());
        assert!(!target.join("saves").exists());
        assert_eq!(
            disk::inventory_instance(&source, false, &cancel).unwrap(),
            inventory
        );
        assert_eq!(
            fs::read(source.join("saves/world/level.dat")).unwrap(),
            b"original world"
        );
        fs::write(target.join("mods/a.jar.disabled"), b"copy edit").unwrap();
        assert_eq!(
            fs::read(source.join("mods/a.jar")).unwrap(),
            b"original Mod bytes"
        );
    }
}
