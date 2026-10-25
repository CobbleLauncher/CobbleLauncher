// SPDX-License-Identifier: MIT
//! Native-selected immutable report input; the frontend never receives a file path.
use super::*;
pub(crate) fn permitted(manifest: &PluginManifest) -> Result<(), String> {
    let extensions = &manifest.file_extensions;
    if manifest.capabilities == ["report"] && !extensions.is_empty()
        && extensions.iter().all(|extension| report_media_type(extension).is_some()
            || (extension == "zip" && pack_metadata_protocol(manifest)))
        && manifest.permissions.as_deref().unwrap_or_default().iter().any(|node|
            matches!(node, cobble_plugin_api::PermissionNode::SelectedFileRead { extensions: allowed }
                if extensions.len() == allowed.len() && extensions.iter().all(|extension| allowed.contains(extension)))) {
        Ok(())
    } else { Err("PLUGIN_PERMISSION_DENIED".into()) }
}
pub(crate) async fn pick_for_frontend(
    app: AppHandle,
    manifest: PluginManifest,
    locale: String,
    instance_id: Option<String>,
    cancel: Arc<AtomicBool>,
) -> Result<Option<Report>, String> {
    permitted(&manifest)?;
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
        let extensions: Vec<_> = manifest
            .file_extensions
            .iter()
            .map(String::as_str)
            .collect();
        let Some(file) = rfd::AsyncFileDialog::new()
            .set_title(if locale == "zh-CN" {
                "选择文件"
            } else {
                "Choose file"
            })
            .add_filter(
                if locale == "zh-CN" {
                    "支持的文件"
                } else {
                    "Supported files"
                },
                &extensions,
            )
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
            permitted(&plugin.manifest)?;
            let (bytes, media) = selected_snapshot(&path, &manifest)?;
            let component = package(&plugin)?;
            Ok::<_, String>((plugin, component, bytes, media))
        })
        .await
        .map_err(|_| "PLUGIN_FRONTEND_FAILED")??;
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
