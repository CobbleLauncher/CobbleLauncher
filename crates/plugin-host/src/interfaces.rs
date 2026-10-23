// SPDX-License-Identifier: MIT
//! Disk-only capability discovery. Declarations never grant execution permissions.
use crate::{PluginRegistry, catalog::CatalogStore};
use cobble_plugin_api::{InstalledPlugin, InterfaceRequirement, PluginError, PluginManifest};
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct InterfaceImplementation {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct InterfaceCandidate {
    pub source_id: String,
    pub plugin_id: String,
    pub name: String,
    pub version: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct InterfaceResolution {
    pub id: String,
    pub version: String,
    pub implementations: Vec<InterfaceImplementation>,
    pub candidates: Vec<InterfaceCandidate>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerInterfaceState {
    Unavailable,
    SelectionRequired,
    Selected,
}
#[derive(Clone, Debug, Serialize)]
pub struct ConsumerInterfaceResolution {
    pub requirement: InterfaceRequirement,
    pub optional: bool,
    pub resolution: InterfaceResolution,
    pub state: ConsumerInterfaceState,
    pub selected: Option<InterfaceImplementation>,
}
/// Remove unavailable required dependencies to a bounded fixed point. Optional peers
/// never affect readiness; no package is installed, enabled or selected here.
pub fn ready_plugins(installed: &[InstalledPlugin]) -> Vec<&InstalledPlugin> {
    let mut ready: Vec<_> = installed
        .iter()
        .filter(|p| p.enabled && p.problem.is_none() && p.manifest.validate().is_ok())
        .collect();
    loop {
        let next: Vec<_> = ready
            .iter()
            .copied()
            .filter(|plugin| {
                plugin
                    .manifest
                    .interfaces
                    .requires
                    .iter()
                    .all(|requirement| {
                        ready
                            .iter()
                            .any(|provider| provider.manifest.supports_interface(requirement))
                    })
            })
            .collect();
        if next.len() == ready.len() {
            return next;
        }
        ready = next;
    }
}
impl PluginRegistry {
    /// Every execution adapter must use this gate, including direct IPC entry points.
    pub fn resolve_enabled(&self, id: &str) -> Result<InstalledPlugin, PluginError> {
        self.directory(id)?;
        let installed = self.list()?;
        let item = installed
            .iter()
            .find(|p| p.manifest.id == id)
            .ok_or_else(|| PluginError::new("PLUGIN_NOT_INSTALLED", "Plugin is not installed"))?;
        if !item.enabled {
            return Err(PluginError::new(
                "PLUGIN_DISABLED",
                "Enable this plugin before use",
            ));
        }
        if let Some(problem) = &item.problem {
            return Err(PluginError::new(
                problem,
                if problem == "PLUGIN_MISSING_INTERFACE" {
                    "A required plugin interface is unavailable"
                } else {
                    "Plugin package is unsupported or invalid"
                },
            ));
        }
        if !ready_plugins(&installed)
            .iter()
            .any(|p| p.manifest.id == id)
        {
            return Err(PluginError::new(
                "PLUGIN_MISSING_INTERFACE",
                "A required plugin interface is unavailable",
            ));
        }
        Ok(item.clone())
    }
    /// Resolve only a dependency declared by this enabled installed consumer.
    /// This chooses no arbitrary implementation and grants no execution permission.
    pub fn resolve_consumer_interface(
        &self,
        consumer_id: &str,
        interface_id: &str,
        selected_plugin_id: Option<&str>,
        catalogs: Option<&CatalogStore>,
    ) -> Result<ConsumerInterfaceResolution, PluginError> {
        let consumer = self.resolve_enabled(consumer_id)?;
        self.resolve_manifest_interface(
            &consumer.manifest,
            interface_id,
            selected_plugin_id,
            catalogs,
        )
    }
    /// Inspect a supplied package manifest against this registry. This does not
    /// install, enable, authorize or execute the supplied consumer.
    pub fn resolve_manifest_interface(
        &self,
        consumer: &PluginManifest,
        interface_id: &str,
        selected_plugin_id: Option<&str>,
        catalogs: Option<&CatalogStore>,
    ) -> Result<ConsumerInterfaceResolution, PluginError> {
        consumer.validate()?;
        let (requirement, optional) = consumer
            .interfaces
            .requires
            .iter()
            .find(|r| r.id == interface_id)
            .map(|r| (r, false))
            .or_else(|| {
                consumer
                    .interfaces
                    .optional
                    .iter()
                    .find(|r| r.id == interface_id)
                    .map(|r| (r, true))
            })
            .ok_or_else(|| {
                PluginError::new(
                    "PLUGIN_UNDECLARED_INTERFACE",
                    "Consumer did not declare this interface dependency",
                )
            })?;
        let resolution = self.resolve_interface(requirement, catalogs)?;
        let selected = if let Some(id) = selected_plugin_id {
            Some(
                resolution
                    .implementations
                    .iter()
                    .find(|i| i.plugin_id == id)
                    .cloned()
                    .ok_or_else(|| {
                        PluginError::new(
                            "PLUGIN_INVALID_INTERFACE_SELECTION",
                            "Selected implementation is unavailable or incompatible",
                        )
                    })?,
            )
        } else if resolution.implementations.len() == 1 {
            resolution.implementations.first().cloned()
        } else {
            None
        };
        let state = if selected.is_some() {
            ConsumerInterfaceState::Selected
        } else if resolution.implementations.is_empty() {
            if !optional {
                return Err(PluginError::new(
                    "PLUGIN_MISSING_INTERFACE",
                    "A required plugin interface is unavailable",
                ));
            }
            ConsumerInterfaceState::Unavailable
        } else {
            ConsumerInterfaceState::SelectionRequired
        };
        Ok(ConsumerInterfaceResolution {
            requirement: requirement.clone(),
            optional,
            resolution,
            state,
            selected,
        })
    }
    pub fn resolve_interface(
        &self,
        requirement: &InterfaceRequirement,
        catalogs: Option<&CatalogStore>,
    ) -> Result<InterfaceResolution, PluginError> {
        requirement.validate()?;
        let installed = self.list()?;
        let mut implementations: Vec<_> = ready_plugins(&installed)
            .into_iter()
            .filter(|p| p.manifest.supports_interface(requirement))
            .map(|p| InterfaceImplementation {
                plugin_id: p.manifest.id.clone(),
                name: p.manifest.name.clone(),
                version: p
                    .manifest
                    .provided_interfaces()
                    .into_iter()
                    .find(|i| requirement.matches(i))
                    .expect("matched interface")
                    .version,
            })
            .collect();
        let mut candidates = Vec::new();
        if let Some(catalogs) = catalogs {
            for source in catalogs
                .list_sources()
                .unwrap_or_default()
                .into_iter()
                .filter(|s| s.enabled)
            {
                // A stale/broken subscription cannot suppress installed implementations.
                let Ok(Some(snapshot)) = catalogs.read_snapshot(&source.id) else {
                    continue;
                };
                for plugin in snapshot.index.plugins {
                    if plugin.supported()
                        && plugin.manifest.supports_interface(requirement)
                        && !installed
                            .iter()
                            .any(|p| p.manifest.id == plugin.manifest.id)
                    {
                        candidates.push(InterfaceCandidate {
                            source_id: source.id.clone(),
                            plugin_id: plugin.manifest.id,
                            name: plugin.manifest.name,
                            version: plugin.manifest.version,
                        });
                    }
                }
            }
        }
        implementations.sort_by(|a, b| a.plugin_id.cmp(&b.plugin_id));
        candidates.sort_by(|a, b| {
            (&a.source_id, &a.plugin_id, &a.version).cmp(&(&b.source_id, &b.plugin_id, &b.version))
        });
        Ok(InterfaceResolution {
            id: requirement.id.clone(),
            version: requirement.version.clone(),
            implementations,
            candidates,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{fs, path::Path};
    fn manifest(id: &str, provider: bool) -> PluginManifest {
        serde_json::from_value(json!({"id":id,"name":id,"version":"1.0.0","publisher":"Original fixture","api_version":if provider{"0.2.0"}else{"1.0.0"},"capabilities":[if provider{"blueprint-provider"}else{"viewer"}],"file_extensions":if provider{vec![]}else{vec!["schem"]},"provider_origins":if provider{vec!["https://community.example"]}else{vec![]},"component_sha256":crate::digest(b"\0asm\x0d\0\x01\0")})).unwrap()
    }
    fn install(root: &Path, registry: &PluginRegistry, manifest: &PluginManifest) {
        let package = root.join(&manifest.id);
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("plugin.json"),
            serde_json::to_vec(manifest).unwrap(),
        )
        .unwrap();
        fs::write(package.join("component.wasm"), b"\0asm\x0d\0\x01\0").unwrap();
        registry.install_directory(&package).unwrap();
    }
    #[test]
    fn consumer_selection_is_declared_explicit_and_deterministic() {
        let root = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(root.path().join("registry"));
        let mut consumer = manifest("consumer", true);
        consumer
            .interfaces
            .optional
            .push(InterfaceRequirement::viewer());
        let absent = registry
            .resolve_manifest_interface(&consumer, cobble_plugin_api::VIEWER_INTERFACE, None, None)
            .unwrap();
        assert!(absent.optional);
        assert_eq!(absent.state, ConsumerInterfaceState::Unavailable);
        assert!(absent.selected.is_none());
        install(root.path(), &registry, &consumer);
        assert_eq!(
            registry
                .resolve_consumer_interface(
                    "consumer",
                    cobble_plugin_api::REPORT_INTERFACE,
                    None,
                    None
                )
                .unwrap_err()
                .code,
            "PLUGIN_UNDECLARED_INTERFACE"
        );
        for id in ["z-viewer", "a-viewer"] {
            install(root.path(), &registry, &manifest(id, false));
        }
        let multiple = registry
            .resolve_consumer_interface("consumer", cobble_plugin_api::VIEWER_INTERFACE, None, None)
            .unwrap();
        assert_eq!(multiple.state, ConsumerInterfaceState::SelectionRequired);
        assert!(multiple.selected.is_none());
        assert_eq!(
            multiple
                .resolution
                .implementations
                .iter()
                .map(|i| i.plugin_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a-viewer", "z-viewer"]
        );
        let selected = registry
            .resolve_consumer_interface(
                "consumer",
                cobble_plugin_api::VIEWER_INTERFACE,
                Some("z-viewer"),
                None,
            )
            .unwrap();
        assert_eq!(selected.state, ConsumerInterfaceState::Selected);
        assert_eq!(selected.selected.unwrap().plugin_id, "z-viewer");
        assert_eq!(
            registry
                .resolve_consumer_interface(
                    "consumer",
                    cobble_plugin_api::VIEWER_INTERFACE,
                    Some("consumer"),
                    None
                )
                .unwrap_err()
                .code,
            "PLUGIN_INVALID_INTERFACE_SELECTION"
        );
        registry.set_enabled("z-viewer", false).unwrap();
        assert_eq!(
            registry
                .resolve_consumer_interface(
                    "consumer",
                    cobble_plugin_api::VIEWER_INTERFACE,
                    None,
                    None
                )
                .unwrap()
                .selected
                .unwrap()
                .plugin_id,
            "a-viewer"
        );
        assert_eq!(
            registry
                .resolve_consumer_interface(
                    "consumer",
                    cobble_plugin_api::VIEWER_INTERFACE,
                    Some("z-viewer"),
                    None
                )
                .unwrap_err()
                .code,
            "PLUGIN_INVALID_INTERFACE_SELECTION"
        );
        registry.set_enabled("a-viewer", false).unwrap();
        assert_eq!(
            registry
                .resolve_consumer_interface(
                    "consumer",
                    cobble_plugin_api::VIEWER_INTERFACE,
                    None,
                    None
                )
                .unwrap()
                .state,
            ConsumerInterfaceState::Unavailable
        );
    }
    #[test]
    fn missing_required_interface_has_the_same_clear_error_for_package_and_installed_consumer() {
        let root = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(root.path().join("registry"));
        let mut consumer = manifest("required-consumer", true);
        consumer
            .interfaces
            .requires
            .push(InterfaceRequirement::viewer());
        assert_eq!(
            registry
                .resolve_manifest_interface(
                    &consumer,
                    cobble_plugin_api::VIEWER_INTERFACE,
                    None,
                    None
                )
                .unwrap_err()
                .code,
            "PLUGIN_MISSING_INTERFACE"
        );
        install(root.path(), &registry, &consumer);
        assert_eq!(
            registry
                .resolve_consumer_interface(
                    "required-consumer",
                    cobble_plugin_api::VIEWER_INTERFACE,
                    None,
                    None
                )
                .unwrap_err()
                .code,
            "PLUGIN_MISSING_INTERFACE"
        );
        install(
            root.path(),
            &registry,
            &manifest("independent-viewer", false),
        );
        let selected = registry
            .resolve_consumer_interface(
                "required-consumer",
                cobble_plugin_api::VIEWER_INTERFACE,
                None,
                None,
            )
            .unwrap();
        assert!(!selected.optional);
        assert_eq!(selected.selected.unwrap().plugin_id, "independent-viewer");
        registry.set_enabled("required-consumer", false).unwrap();
        assert_eq!(
            registry
                .resolve_consumer_interface(
                    "required-consumer",
                    cobble_plugin_api::VIEWER_INTERFACE,
                    None,
                    None
                )
                .unwrap_err()
                .code,
            "PLUGIN_DISABLED"
        );
    }
}
