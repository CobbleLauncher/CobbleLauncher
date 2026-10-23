// SPDX-License-Identifier: MIT
//! Typed Viewer selection for local files and Provider resource handoffs.
use super::*;
use cobble_plugin_host::{PluginRegistry, interfaces::InterfaceResolution};
static DEFAULT_BOOTSTRAP: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub(super) fn seed_default(
    registry: &PluginRegistry,
    bundled: &std::path::Path,
    marker: &std::path::Path,
) -> Result<(), String> {
    if !bundled.exists() || marker.exists() {
        return Ok(());
    }
    let _gate = DEFAULT_BOOTSTRAP
        .lock()
        .map_err(|_| "Default Viewer initialization unavailable")?;
    if marker.exists() {
        return Ok(());
    }
    if !registry
        .list()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|plugin| plugin.manifest.id == "schematic-viewer")
    {
        let (manifest, _) =
            super::plugin_signatures::read_package(bundled).map_err(|e| e.to_string())?;
        if manifest.id != "schematic-viewer" {
            return Err("Bundled default Viewer identity differs".into());
        }
        registry
            .install_directory(bundled)
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(marker, b"1").map_err(|e| e.to_string())
}
#[derive(Serialize)]
pub struct ViewerChoice {
    pub id: String,
    pub name: String,
    pub file_extensions: Vec<String>,
}
#[derive(Serialize)]
pub struct ViewerChoices {
    pub viewer_id: Option<String>,
    pub viewers: Vec<ViewerChoice>,
    pub disabled_viewers: Vec<ViewerChoice>,
    pub interface: InterfaceResolution,
}
fn describe(item: &InstalledPlugin) -> ViewerChoice {
    ViewerChoice {
        id: item.manifest.id.clone(),
        name: item.manifest.name.clone(),
        file_extensions: item.manifest.file_extensions.clone(),
    }
}
fn declares_viewer(manifest: &PluginManifest) -> bool {
    manifest
        .interfaces
        .requires
        .iter()
        .chain(&manifest.interfaces.optional)
        .any(|requirement| requirement.id == cobble_plugin_api::VIEWER_INTERFACE)
}
pub(super) fn choices_in(
    registry: &PluginRegistry,
    provider_id: Option<&str>,
    format: Option<BlueprintFormat>,
) -> Result<ViewerChoices, String> {
    let (requirement, mut resolution) = if let Some(provider) = provider_id {
        let consumer = registry
            .resolve_enabled(provider)
            .map_err(|e| e.to_string())?;
        if !consumer
            .manifest
            .supports_interface(&cobble_plugin_api::InterfaceRequirement::blueprint_provider())
        {
            return Err("Selected plugin is not a Blueprint Provider".into());
        }
        if declares_viewer(&consumer.manifest) {
            let resolved = registry
                .resolve_consumer_interface(
                    provider,
                    cobble_plugin_api::VIEWER_INTERFACE,
                    None,
                    None,
                )
                .map_err(|e| e.to_string())?;
            (resolved.requirement, resolved.resolution)
        } else {
            let requirement = cobble_plugin_api::InterfaceRequirement::viewer();
            let resolution = registry
                .resolve_interface(&requirement, None)
                .map_err(|e| e.to_string())?;
            (requirement, resolution)
        }
    } else {
        let requirement = cobble_plugin_api::InterfaceRequirement::viewer();
        let resolution = registry
            .resolve_interface(&requirement, None)
            .map_err(|e| e.to_string())?;
        (requirement, resolution)
    };
    let installed = registry.list().map_err(|e| e.to_string())?;
    let compatible = |item: &&InstalledPlugin| {
        item.manifest.capabilities == ["viewer"]
            && item.manifest.supports_interface(&requirement)
            && item
                .manifest
                .file_extensions
                .iter()
                .any(|extension| format.is_none_or(|format| extension == format.extension()))
    };
    let viewers = installed
        .iter()
        .filter(compatible)
        .filter(|item| {
            resolution
                .implementations
                .iter()
                .any(|implementation| implementation.plugin_id == item.manifest.id)
        })
        .map(describe)
        .collect::<Vec<_>>();
    resolution.implementations.retain(|implementation| {
        viewers
            .iter()
            .any(|viewer| viewer.id == implementation.plugin_id)
    });
    let disabled_viewers = installed
        .iter()
        .filter(compatible)
        .filter(|item| !item.enabled && item.problem.is_none())
        .map(describe)
        .collect();
    Ok(ViewerChoices {
        viewer_id: (viewers.len() == 1).then(|| viewers[0].id.clone()),
        viewers,
        disabled_viewers,
        interface: resolution,
    })
}
pub(super) fn select_in(
    registry: &PluginRegistry,
    provider_id: Option<&str>,
    format: BlueprintFormat,
    selected: Option<&str>,
) -> Result<InstalledPlugin, String> {
    let choices = choices_in(registry, provider_id, Some(format))?;
    let id = match selected {
        Some(id) if choices.viewers.iter().any(|viewer| viewer.id == id) => id,
        Some(_) => return Err(
            "PLUGIN_INVALID_INTERFACE_SELECTION: selected Viewer is unavailable or incompatible"
                .into(),
        ),
        None => choices.viewer_id.as_deref().ok_or_else(|| {
            if choices.viewers.is_empty() {
                "PLUGIN_MISSING_INTERFACE: enable a compatible Viewer"
            } else {
                "PLUGIN_VIEWER_SELECTION_REQUIRED: select a compatible Viewer"
            }
            .to_string()
        })?,
    };
    if let Some(provider) = provider_id
        && declares_viewer(
            &registry
                .resolve_enabled(provider)
                .map_err(|e| e.to_string())?
                .manifest,
        )
    {
        registry
            .resolve_consumer_interface(
                provider,
                cobble_plugin_api::VIEWER_INTERFACE,
                Some(id),
                None,
            )
            .map_err(|e| e.to_string())?;
    }
    registry.resolve_enabled(id).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_blueprint_viewer_choices(
    provider_id: Option<String>,
    format: Option<String>,
) -> Result<ViewerChoices, String> {
    let format = match format.as_deref() {
        Some("schem") => Some(BlueprintFormat::Schem),
        Some("litematic") => Some(BlueprintFormat::Litematic),
        None => None,
        _ => return Err("Unsupported blueprint format".into()),
    };
    choices_in(&registry()?, provider_id.as_deref(), format)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn install(
        root: &std::path::Path,
        registry: &PluginRegistry,
        id: &str,
        provider: bool,
        extensions: &[&str],
    ) {
        let source = root.join(id);
        std::fs::create_dir(&source).unwrap();
        let bytes = b"\0asm\x0d\0\x01\0";
        let mut value = serde_json::json!({"id":id,"name":id,"version":"1.0.0","api_version":if provider{"0.2.0"}else{"1.0.0"},"publisher":"Original fixture","capabilities":[if provider{"blueprint-provider"}else{"viewer"}],"file_extensions":extensions,"component_sha256":cobble_plugin_host::digest(bytes)});
        if provider {
            value["provider_origins"] = serde_json::json!(["https://community.example"]);
            value["interfaces"] =
                serde_json::json!({"optional":[{"id":"launcher:viewer","version":"^1.0.0"}]});
        }
        std::fs::write(
            source.join("plugin.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        std::fs::write(source.join("component.wasm"), bytes).unwrap();
        registry.install_directory(&source).unwrap();
    }
    #[test]
    fn first_default_bootstrap_is_serialized_and_never_reseeds_disabled_or_removed_choice() {
        let root = tempfile::tempdir().unwrap();
        let source_registry = PluginRegistry::new(root.path().join("source-registry"));
        install(
            root.path(),
            &source_registry,
            "schematic-viewer",
            false,
            &["schem"],
        );
        let bundled = root.path().join("schematic-viewer");
        let destination = root.path().join("destination");
        let marker = destination.join(".bundled-seeded");
        let workers = (0..4)
            .map(|_| {
                let bundled = bundled.clone();
                let destination = destination.clone();
                let marker = marker.clone();
                std::thread::spawn(move || {
                    seed_default(&PluginRegistry::new(destination), &bundled, &marker)
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap().unwrap();
        }
        let registry = PluginRegistry::new(destination);
        assert_eq!(registry.list().unwrap().len(), 1);
        registry.set_enabled("schematic-viewer", false).unwrap();
        seed_default(&registry, &bundled, &marker).unwrap();
        assert!(!registry.list().unwrap()[0].enabled);
        registry.uninstall("schematic-viewer").unwrap();
        seed_default(&registry, &bundled, &marker).unwrap();
        assert!(registry.list().unwrap().is_empty());
    }
    #[test]
    fn format_and_typed_capability_require_explicit_multiple_choice_and_no_provider_for_local() {
        let root = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(root.path().join("registry"));
        install(root.path(), &registry, "source-alpha", true, &[]);
        install(root.path(), &registry, "preview-alpha", false, &["schem"]);
        install(
            root.path(),
            &registry,
            "preview-beta",
            false,
            &["litematic"],
        );
        assert_eq!(
            select_in(
                &registry,
                Some("source-alpha"),
                BlueprintFormat::Schem,
                None
            )
            .unwrap()
            .manifest
            .id,
            "preview-alpha"
        );
        install(root.path(), &registry, "preview-gamma", false, &["schem"]);
        assert!(
            select_in(
                &registry,
                Some("source-alpha"),
                BlueprintFormat::Schem,
                None
            )
            .unwrap_err()
            .contains("SELECTION_REQUIRED")
        );
        assert_eq!(
            select_in(
                &registry,
                Some("source-alpha"),
                BlueprintFormat::Schem,
                Some("preview-gamma")
            )
            .unwrap()
            .manifest
            .id,
            "preview-gamma"
        );
        assert!(
            select_in(
                &registry,
                Some("source-alpha"),
                BlueprintFormat::Schem,
                Some("preview-beta")
            )
            .is_err()
        );
        install(root.path(), &registry, "legacy-source", true, &[]);
        let source = root.path().join("legacy-source");
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&std::fs::read(source.join("plugin.json")).unwrap()).unwrap();
        legacy.as_object_mut().unwrap().remove("interfaces");
        std::fs::write(
            source.join("plugin.json"),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .unwrap();
        registry.update_directory(&source).unwrap();
        assert_eq!(
            select_in(
                &registry,
                Some("legacy-source"),
                BlueprintFormat::Litematic,
                None
            )
            .unwrap()
            .manifest
            .id,
            "preview-beta"
        );
        registry.set_enabled("preview-alpha", false).unwrap();
        assert_eq!(
            choices_in(
                &registry,
                Some("source-alpha"),
                Some(BlueprintFormat::Schem)
            )
            .unwrap()
            .disabled_viewers
            .len(),
            1
        );
        registry.set_enabled("source-alpha", false).unwrap();
        assert!(
            select_in(
                &registry,
                Some("source-alpha"),
                BlueprintFormat::Schem,
                None
            )
            .is_err()
        );
        assert_eq!(
            select_in(&registry, None, BlueprintFormat::Litematic, None)
                .unwrap()
                .manifest
                .id,
            "preview-beta"
        );
    }
}
