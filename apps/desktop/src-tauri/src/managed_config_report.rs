// SPDX-License-Identifier: MIT
//! Explicitly selected managed configs use the same immutable folder report protocol.
use super::folder_report::{FolderReport, WindowLease, empty_snapshot};
use super::*;
use cobble_plugin_api::{
    CONFIG_FOLDER_MEDIA_TYPE, ConfigFolderFile, ConfigFolderSnapshot,
    MAX_CONFIG_FOLDER_ENVELOPE_BYTES, MAX_CONFIG_FOLDER_FILE_BYTES, MAX_CONFIG_FOLDER_TEXT_BYTES,
};
use std::collections::HashSet;
fn snapshot(
    directory: &Path,
    ids: &[String],
    manifest: &PluginManifest,
    cancel: &AtomicBool,
) -> Result<ConfigFolderSnapshot, String> {
    let mut input = empty_snapshot();
    input
        .validate(manifest)
        .map_err(|_| "FOLDER_REPORT_PERMISSION_REQUIRED")?;
    if ids.is_empty()
        || ids.len() > 64
        || ids
            .iter()
            .any(|id| id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()))
        || ids.iter().collect::<HashSet<_>>().len() != ids.len()
    {
        return Err("CONFIG_SELECTION_INVALID".into());
    }
    let mut total = 0;
    for id in ids {
        if cancel.load(Ordering::Acquire) {
            return Err("CANCEL_REQUESTED".into());
        }
        let Ok((name, bytes)) = crate::games::game_config::report_current_config(directory, id)
        else {
            input.skipped += 1;
            continue;
        };
        let Some(extension) = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()) else {
            input.skipped += 1;
            continue;
        };
        let Some(media) = cobble_plugin_api::config_folder_media_type(&extension)
            .filter(|_| manifest.file_extensions.contains(&extension))
        else {
            input.skipped += 1;
            continue;
        };
        if bytes.len() > MAX_CONFIG_FOLDER_FILE_BYTES
            || total + bytes.len() > MAX_CONFIG_FOLDER_TEXT_BYTES
        {
            input.skipped += 1;
            input.truncated = true;
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            input.skipped += 1;
            continue;
        };
        let length = text.len();
        input.files.push(ConfigFolderFile {
            relative_name: name,
            media_type: media.into(),
            text,
        });
        if input.validate_bounds().is_err()
            || serde_json::to_vec(&input)
                .map_err(|_| "FOLDER_REPORT_FAILED")?
                .len()
                > MAX_CONFIG_FOLDER_ENVELOPE_BYTES
        {
            input.files.pop();
            input.skipped += 1;
            input.truncated = true;
            continue;
        }
        total += length;
    }
    input
        .validate(manifest)
        .map_err(|_| "FOLDER_REPORT_FAILED")?;
    Ok(input)
}
#[tauri::command]
pub async fn analyze_game_config_batch_report(
    app: AppHandle,
    window: WebviewWindow,
    game_id: String,
    config_ids: Vec<String>,
    plugin_id: String,
    locale: Option<String>,
) -> Result<Option<FolderReport>, String> {
    let locale = locale.unwrap_or_else(|| "en".into());
    super::super::extension_contributions::validate_locale(&locale)?;
    crate::games::ensure_registered_game(&app, &game_id)?;
    let cancel = state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .begin(plugin_id.clone())?;
    let result = async {
        let _lease = WindowLease::new(window.label(), cancel.clone())?;
        execute_batch(
            app,
            game_id,
            config_ids,
            plugin_id,
            locale,
            cancel.clone(),
            None,
        )
        .await
    }
    .await;
    state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .finish(&cancel, result)
}
pub(crate) async fn run_for_frontend(
    app: AppHandle,
    game_id: String,
    config_ids: Vec<String>,
    manifest: PluginManifest,
    locale: String,
    cancel: Arc<AtomicBool>,
) -> Result<Option<FolderReport>, String> {
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
    let result = execute_batch(
        app,
        game_id,
        config_ids,
        manifest.id.clone(),
        locale,
        cancel.clone(),
        Some(manifest),
    )
    .await;
    state()
        .lock()
        .map_err(|_| "PLUGIN_REPORT_BUSY")?
        .finish(&cancel, result)
}
async fn execute_batch(
    app: AppHandle,
    game_id: String,
    config_ids: Vec<String>,
    plugin_id: String,
    locale: String,
    cancel: Arc<AtomicBool>,
    expected: Option<PluginManifest>,
) -> Result<Option<FolderReport>, String> {
    let directory = crate::games::game_directory(&app, &game_id)?;
    let id = plugin_id.clone();
    let read_cancel = cancel.clone();
    let (plugin, component, input) = tauri::async_runtime::spawn_blocking(move || {
        let plugin = selected_plugin(&id)?;
        if expected
            .as_ref()
            .is_some_and(|manifest| manifest != &plugin.manifest)
        {
            return Err("PLUGIN_VIEW_STALE".into());
        }
        let input = snapshot(&directory, &config_ids, &plugin.manifest, &read_cancel)?;
        let component = package(&plugin)?;
        Ok::<_, String>((plugin, component, input))
    })
    .await
    .map_err(|_| "FOLDER_REPORT_FAILED")??;
    crate::games::ensure_registered_game(&app, &game_id)?;
    let files = input.files.len() as u32;
    let skipped = input.skipped;
    let truncated = input.truncated;
    let report = execute_report(
        plugin,
        component,
        serde_json::to_vec(&input).map_err(|_| "FOLDER_REPORT_FAILED")?,
        CONFIG_FOLDER_MEDIA_TYPE,
        cancel.clone(),
        locale,
        Some(game_id),
    )
    .await?;
    Ok(report.map(|report| FolderReport {
        report,
        files,
        skipped,
        truncated,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_ids_resolve_nested_configs_without_paths_or_mutation() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config/nested")).unwrap();
        std::fs::write(dir.path().join("config/nested/test.json"), b"{}").unwrap();
        let manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/config-doctor/plugin.json"
        ))
        .unwrap();
        let name = if cfg!(windows) {
            "nested\\test.json"
        } else {
            "nested/test.json"
        };
        let ids = vec![cobble_plugin_host::digest(name.as_bytes()), "0".repeat(64)];
        let result = snapshot(dir.path(), &ids, &manifest, &AtomicBool::new(false)).unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].relative_name, "nested/test.json");
        assert_eq!(result.skipped, 1);
        assert!(!result.truncated);
        assert!(
            snapshot(
                dir.path(),
                &["../test.json".into()],
                &manifest,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read(dir.path().join("config/nested/test.json")).unwrap(),
            b"{}"
        );
    }
}
