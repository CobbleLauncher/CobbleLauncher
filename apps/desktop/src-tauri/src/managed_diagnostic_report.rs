// SPDX-License-Identifier: MIT
//! Immutable, revision-checked diagnostic snapshots use the existing Wasm report worker.
use super::*;
pub(crate) async fn pick_for_frontend(
    app: AppHandle,
    manifest: PluginManifest,
    locale: String,
    instance_id: Option<String>,
    cancel: Arc<AtomicBool>,
) -> Result<Option<Report>, String> {
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
        let Some(file) = rfd::AsyncFileDialog::new()
            .set_title("Choose log or crash report")
            .add_filter("Logs", &["log", "txt"])
            .pick_file()
            .await
        else {
            return Ok(None);
        };
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        let path = file.path().to_owned();
        let (plugin, component, bytes, media) = tauri::async_runtime::spawn_blocking(move || {
            let plugin = selected_plugin(&manifest.id)?;
            if manifest != plugin.manifest {
                return Err("PLUGIN_VIEW_STALE".into());
            }
            let (bytes, media) = selected_snapshot(&path, &manifest)?;
            let component = package(&plugin)?;
            Ok::<_, String>((plugin, component, bytes, media))
        })
        .await
        .map_err(|_| "DIAGNOSTIC_IO")??;
        if let Some(id) = &instance_id {
            crate::games::ensure_registered_game(&app, id)?;
        }
        execute_report(
            plugin,
            component,
            bytes,
            media,
            cancel.clone(),
            locale,
            instance_id,
        )
        .await
    }
    .await;
    state()
        .lock()
        .map_err(|_| "PLUGIN_REPORT_BUSY")?
        .finish(&cancel, result)
}
pub(crate) async fn run_for_frontend(
    app: AppHandle,
    game_id: String,
    diagnostic_id: String,
    revision: String,
    manifest: PluginManifest,
    locale: String,
    cancel: Arc<AtomicBool>,
) -> Result<Option<Report>, String> {
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
        let directory = crate::games::game_directory(&app, &game_id)?;
        let expected = manifest.clone();
        let (plugin, component, document) = tauri::async_runtime::spawn_blocking(move || {
            let plugin = selected_plugin(&expected.id)?;
            if plugin.manifest != expected {
                return Err("PLUGIN_VIEW_STALE".into());
            }
            let document = crate::games::game_diagnostics::read_at(&directory, &diagnostic_id)?;
            if document.revision != revision {
                return Err("DIAGNOSTIC_CHANGED".into());
            }
            let extension = document
                .name
                .rsplit_once('.')
                .map(|(_, extension)| extension)
                .ok_or("DIAGNOSTIC_NOT_FOUND")?;
            if !plugin
                .manifest
                .file_extensions
                .iter()
                .any(|allowed| allowed == extension)
            {
                return Err("PLUGIN_PERMISSION_DENIED".into());
            }
            let component = package(&plugin)?;
            Ok::<_, String>((plugin, component, document))
        })
        .await
        .map_err(|_| "DIAGNOSTIC_IO")??;
        crate::games::ensure_registered_game(&app, &game_id)?;
        execute_report(
            plugin,
            component,
            document.text.into_bytes(),
            "text/plain",
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
