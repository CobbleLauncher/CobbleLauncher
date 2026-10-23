// SPDX-License-Identifier: MIT
//! Bounded host audit metadata. Raw requests, errors and filesystem paths never enter records.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
const MAX_RECORDS: usize = 512;
const MAX_BYTES: u64 = 262_144;
static LOCK: Mutex<()> = Mutex::new(());
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Allowed,
    Denied,
    Completed,
    Failed,
    Cancelled,
}
pub enum Target<'a> {
    None,
    Game(&'a str),
    Origin(&'a str),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    timestamp_ms: u64,
    plugin_id: String,
    capability: String,
    action: String,
    outcome: Outcome,
    target: Option<String>,
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}
fn target(value: Target<'_>) -> Option<String> {
    match value {
        Target::None => None,
        Target::Game(id) => identifier(id).then(|| format!("game:{id}")),
        Target::Origin(raw) => tauri::Url::parse(raw)
            .ok()
            .filter(|url| url.scheme() == "https" && url.host_str().is_some())
            .map(|url| url.origin().ascii_serialization()),
    }
}
fn read(path: &Path) -> Vec<Entry> {
    let Ok(file) = fs::File::open(path) else {
        return vec![];
    };
    if file.metadata().map_or(true, |m| m.len() > MAX_BYTES) {
        return vec![];
    }
    let mut text = String::new();
    if file.take(MAX_BYTES).read_to_string(&mut text).is_err() {
        return vec![];
    }
    text.lines()
        .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
        .filter(|e| {
            e.timestamp_ms <= 8_640_000_000_000_000
                && identifier(&e.plugin_id)
                && identifier(&e.capability)
                && identifier(&e.action)
                && e.target.as_ref().is_none_or(|v| {
                    v.strip_prefix("game:").is_some_and(identifier)
                        || target(Target::Origin(v)).as_deref() == Some(v.as_str())
                })
        })
        .take(MAX_RECORDS)
        .collect()
}
fn append(path: &Path, entry: Entry) -> Result<(), std::io::Error> {
    let mut entries = read(path);
    entries.push(entry);
    let mut counts = std::collections::HashMap::new();
    entries = entries
        .into_iter()
        .rev()
        .filter(|e| {
            let count = counts.entry(e.plugin_id.clone()).or_insert(0);
            *count += 1;
            *count <= 100
        })
        .take(MAX_RECORDS)
        .collect();
    entries.reverse();
    let mut text = String::new();
    for entry in entries {
        text.push_str(&serde_json::to_string(&entry)?);
        text.push('\n');
    }
    while text.len() > MAX_BYTES as usize {
        let Some(end) = text.find('\n') else { break };
        text.drain(..=end);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}
/// Identity must come from an installed, native-authorized invocation, never request parameters.
pub fn record(
    plugin_id: &str,
    capability: &'static str,
    action: &'static str,
    resource: Target<'_>,
    outcome: Outcome,
) {
    if !identifier(plugin_id) || !identifier(capability) || !identifier(action) {
        return;
    }
    let entry = Entry {
        timestamp_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        plugin_id: plugin_id.into(),
        capability: capability.into(),
        action: action.into(),
        outcome,
        target: target(resource),
    };
    tracing::info!(event = "plugin_access", plugin_id, capability, action, outcome = ?outcome, target = ?entry.target);
    if let (Ok(_guard), Ok(root)) = (LOCK.lock(), super::root("logs")) {
        if append(&root.join("plugin-access.jsonl"), entry).is_err() {
            tracing::warn!(event = "plugin_access_log_unavailable");
        }
    }
}
#[tauri::command]
pub fn plugin_access_log(
    window: tauri::WebviewWindow,
    plugin_id: String,
) -> Result<Vec<Entry>, String> {
    if window.label() != "main" {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    super::valid_id(&plugin_id)?;
    let installed = cobble_plugin_host::PluginRegistry::new(super::root("plugins")?)
        .list()
        .map_err(|_| "PLUGIN_REGISTRY_UNAVAILABLE")?;
    if !installed.iter().any(|p| p.manifest.id == plugin_id) {
        return Err("PLUGIN_NOT_INSTALLED".into());
    }
    let _guard = LOCK.lock().map_err(|_| "PLUGIN_ACCESS_LOG_UNAVAILABLE")?;
    Ok(read(&super::root("logs")?.join("plugin-access.jsonl"))
        .into_iter()
        .rev()
        .filter(|entry| entry.plugin_id == plugin_id)
        .take(100)
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn targets_strip_secrets_and_paths() {
        assert_eq!(
            target(Target::Origin(
                "https://user:secret@example.com/private?token=secret#invite"
            )),
            Some("https://example.com".into())
        );
        assert_eq!(target(Target::Game("C:/private")), None);
        assert_eq!(target(Target::Origin("file:///private")), None);
    }
    #[test]
    fn retention_is_bounded_and_corrupt_lines_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        fs::write(&path, "invalid\n").unwrap();
        for n in 0..110 {
            append(
                &path,
                Entry {
                    timestamp_ms: n,
                    plugin_id: "doctor".into(),
                    capability: "game-config-read".into(),
                    action: "configs.read".into(),
                    outcome: Outcome::Completed,
                    target: None,
                },
            )
            .unwrap();
        }
        let entries = read(&path);
        assert_eq!(entries.len(), 100);
        assert_eq!(entries[0].timestamp_ms, 10);
        fs::write(&path, "x".repeat(MAX_BYTES as usize + 1)).unwrap();
        assert!(read(&path).is_empty());
    }
}
