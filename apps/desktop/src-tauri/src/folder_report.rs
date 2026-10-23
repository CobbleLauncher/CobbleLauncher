// SPDX-License-Identifier: MIT
//! Explicit one-invocation selected-folder snapshot. No filesystem handles enter Wasm.
use super::*;
use cobble_plugin_api::{
    CONFIG_FOLDER_MEDIA_TYPE, ConfigFolderFile, ConfigFolderSnapshot,
    MAX_CONFIG_FOLDER_ENVELOPE_BYTES, MAX_CONFIG_FOLDER_FILE_BYTES, MAX_CONFIG_FOLDER_TEXT_BYTES,
};
use cobble_plugin_host::resource_grants::{
    FolderGrants, GrantError, PluginIdentity, ResourceHandle,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
#[derive(Serialize)]
pub struct FolderReport {
    pub report: Report,
    pub files: u32,
    pub skipped: u32,
    pub truncated: bool,
}
static WINDOWS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
fn windows() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    WINDOWS.get_or_init(Default::default)
}
pub(crate) fn revoke_window(window: &str) {
    if let Ok(mut windows) = windows().lock()
        && let Some(cancel) = windows.remove(window)
    {
        cancel.store(true, Ordering::Release);
    }
}
pub(super) struct WindowLease {
    window: String,
    cancel: Arc<AtomicBool>,
}
impl WindowLease {
    pub(super) fn new(window: &str, cancel: Arc<AtomicBool>) -> Result<Self, String> {
        let mut windows = windows().lock().map_err(|_| "FOLDER_REPORT_BUSY")?;
        if windows.len() >= 4 && !windows.contains_key(window) {
            return Err("FOLDER_REPORT_BUSY".into());
        }
        if let Some(old) = windows.insert(window.into(), cancel.clone()) {
            old.store(true, Ordering::Release);
        }
        Ok(Self {
            window: window.into(),
            cancel,
        })
    }
}
impl Drop for WindowLease {
    fn drop(&mut self) {
        if let Ok(mut windows) = windows().lock()
            && windows
                .get(&self.window)
                .is_some_and(|flag| Arc::ptr_eq(flag, &self.cancel))
        {
            windows.remove(&self.window);
        }
    }
}
struct GrantedSnapshot {
    grants: FolderGrants,
    owner: PluginIdentity,
    root: ResourceHandle,
    snapshot: ConfigFolderSnapshot,
}
impl Drop for GrantedSnapshot {
    fn drop(&mut self) {
        let _ = self.grants.revoke(&self.owner, self.root);
    }
}
pub(super) fn empty_snapshot() -> ConfigFolderSnapshot {
    ConfigFolderSnapshot {
        schema_version: 1,
        files: Vec::new(),
        skipped: 0,
        truncated: false,
    }
}
fn file_media(name: &str, extensions: &[String]) -> Option<&'static str> {
    if name.is_empty()
        || name.len() > 240
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
    {
        return None;
    }
    let extension = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    if !extensions.contains(&extension)
        || !matches!(
            extension.as_str(),
            "json" | "toml" | "properties" | "yaml" | "yml"
        )
    {
        return None;
    }
    cobble_plugin_api::config_folder_media_type(&extension)
}
fn read_file(
    grants: &mut FolderGrants,
    owner: &PluginIdentity,
    handle: ResourceHandle,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, GrantError> {
    let mut bytes = Vec::new();
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(GrantError::Denied);
        }
        let maximum = (MAX_CONFIG_FOLDER_FILE_BYTES + 1 - bytes.len()).min(65536) as u32;
        let chunk = grants.read_at(owner, handle, bytes.len() as u64, maximum)?;
        if chunk.is_empty() {
            return Ok(bytes);
        }
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX_CONFIG_FOLDER_FILE_BYTES {
            return Ok(bytes);
        }
    }
}
fn snapshot_selected(
    manifest: &PluginManifest,
    path: &Path,
    cancel: &AtomicBool,
) -> Result<GrantedSnapshot, String> {
    if cancel.load(Ordering::Acquire) {
        return Err("CANCEL_REQUESTED".into());
    }
    empty_snapshot()
        .validate(manifest)
        .map_err(|_| "FOLDER_REPORT_PERMISSION_REQUIRED")?;
    let owner = PluginIdentity {
        publisher: manifest.publisher.clone(),
        plugin_id: manifest.id.clone(),
        manifest_sha256: cobble_plugin_host::digest(
            &serde_json::to_vec(manifest).map_err(|_| "FOLDER_REPORT_FAILED")?,
        ),
    };
    let mut grants = FolderGrants::default();
    let root = grants
        .grant_selected(owner.clone(), path)
        .map_err(|_| "FOLDER_REPORT_DENIED")?;
    let mut input = GrantedSnapshot {
        grants,
        owner,
        root,
        snapshot: empty_snapshot(),
    };
    let extensions: Vec<_> = manifest
        .file_extensions
        .iter()
        .filter(|ext| {
            matches!(
                ext.as_str(),
                "json" | "toml" | "properties" | "yaml" | "yml"
            )
        })
        .cloned()
        .collect();
    let listing = input
        .grants
        .list_files_bounded(&input.owner, &root, &extensions, 64, 1024)
        .map_err(|_| "FOLDER_REPORT_DENIED")?;
    input.snapshot.skipped = listing.skipped;
    input.snapshot.truncated = listing.truncated;
    let mut text_bytes = 0;
    let mut envelope_bytes = serde_json::to_vec(&input.snapshot)
        .map_err(|_| "FOLDER_REPORT_FAILED")?
        .len()
        + 32;
    let mut names = HashSet::new();
    let count = listing.entries.len();
    for (index, entry) in listing.entries.into_iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err("CANCEL_REQUESTED".into());
        }
        let Some(media_type) = file_media(&entry.name, &extensions) else {
            input.snapshot.skipped += 1;
            continue;
        };
        if !names.insert(entry.name.to_lowercase()) {
            input.snapshot.skipped += 1;
            continue;
        }
        let bytes = match read_file(&mut input.grants, &input.owner, entry.handle, cancel) {
            Ok(bytes) => bytes,
            Err(GrantError::Limit) => {
                input.snapshot.skipped += (count - index) as u32;
                input.snapshot.truncated = true;
                break;
            }
            Err(_) => {
                if cancel.load(Ordering::Acquire) {
                    return Err("CANCEL_REQUESTED".into());
                }
                input.snapshot.skipped += 1;
                continue;
            }
        };
        if bytes.len() > MAX_CONFIG_FOLDER_FILE_BYTES {
            input.snapshot.skipped += 1;
            input.snapshot.truncated = true;
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            input.snapshot.skipped += 1;
            continue;
        };
        if text_bytes + text.len() > MAX_CONFIG_FOLDER_TEXT_BYTES {
            input.snapshot.skipped += 1;
            input.snapshot.truncated = true;
            continue;
        }
        let file = ConfigFolderFile {
            relative_name: entry.name,
            media_type: media_type.into(),
            text,
        };
        let encoded = serde_json::to_vec(&file)
            .map_err(|_| "FOLDER_REPORT_FAILED")?
            .len()
            + 1;
        if envelope_bytes + encoded > MAX_CONFIG_FOLDER_ENVELOPE_BYTES {
            input.snapshot.skipped += 1;
            input.snapshot.truncated = true;
            continue;
        }
        envelope_bytes += encoded;
        text_bytes += file.text.len();
        input.snapshot.files.push(file);
    }
    input
        .snapshot
        .validate(manifest)
        .map_err(|_| "FOLDER_REPORT_FAILED")?;
    Ok(input)
}
#[tauri::command]
pub async fn analyze_plugin_folder_report(
    app: AppHandle,
    window: WebviewWindow,
    plugin_id: String,
    locale: Option<String>,
    instance_id: Option<String>,
) -> Result<Option<FolderReport>, String> {
    if let Some(id) = &instance_id {
        crate::games::ensure_registered_game(&app, id)?
    }
    let locale = locale.unwrap_or_else(|| "en".into());
    super::super::extension_contributions::validate_locale(&locale)?;
    let cancel = state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .begin(plugin_id.clone())?;
    let result = async {
        let _lease = WindowLease::new(window.label(), cancel.clone())?;
        let id = plugin_id.clone();
        let (plugin, component) = tauri::async_runtime::spawn_blocking(move || {
            let plugin = selected_plugin(&id)?;
            empty_snapshot()
                .validate(&plugin.manifest)
                .map_err(|_| "FOLDER_REPORT_PERMISSION_REQUIRED")?;
            let component = package(&plugin)?;
            Ok::<_, String>((plugin, component))
        })
        .await
        .map_err(|_| "FOLDER_REPORT_FAILED")??;
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        let Some(folder) = rfd::AsyncFileDialog::new()
            .set_parent(&window)
            .set_title(if locale == "zh-CN" {
                "选择要检查的配置文件夹"
            } else {
                "Select configuration folder"
            })
            .pick_folder()
            .await
        else {
            return Ok(None);
        };
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        verify_current(&plugin.manifest)?;
        let path = folder.path().to_path_buf();
        let manifest = plugin.manifest.clone();
        let read_cancel = cancel.clone();
        let input = tauri::async_runtime::spawn_blocking(move || {
            snapshot_selected(&manifest, &path, &read_cancel)
        })
        .await
        .map_err(|_| "FOLDER_REPORT_FAILED")??;
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        let files = input.snapshot.files.len() as u32;
        let skipped = input.snapshot.skipped;
        let truncated = input.snapshot.truncated;
        let bytes = serde_json::to_vec(&input.snapshot).map_err(|_| "FOLDER_REPORT_FAILED")?;
        let report = execute_report(
            plugin,
            component,
            bytes,
            CONFIG_FOLDER_MEDIA_TYPE,
            cancel.clone(),
            locale,
            instance_id,
        )
        .await?;
        drop(input);
        Ok(report.map(|report| FolderReport {
            report,
            files,
            skipped,
            truncated,
        }))
    }
    .await;
    state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .finish(&cancel, result)
}
#[cfg(test)]
#[path = "folder_report_tests.rs"]
mod tests;
