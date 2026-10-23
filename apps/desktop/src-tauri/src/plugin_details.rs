// SPDX-License-Identifier: MIT
//! Read-only installed extension details and declared interface combinations.
use cobble_plugin_api::{InstalledPlugin, InterfaceRequirement, PluginManifest, ProvidedInterface};
use cobble_plugin_host::{PluginRegistry, catalog::CatalogStore};
use serde::Serialize;
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyState {
    Available,
    SelectionRequired,
    OptionalMissing,
    RequiredMissing,
    Unsupported,
}
#[derive(Serialize)]
pub struct Implementation {
    plugin_id: String,
    name: String,
    version: String,
    enabled: bool,
    available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    problem: Option<String>,
}
#[derive(Serialize)]
pub struct Candidate {
    source_id: String,
    source_name: String,
    plugin_id: String,
    name: String,
    version: String,
    project_url: Option<String>,
}
#[derive(Serialize)]
pub struct Dependency {
    requirement: InterfaceRequirement,
    optional: bool,
    state: DependencyState,
    implementations: Vec<Implementation>,
    candidates: Vec<Candidate>,
    candidates_truncated: bool,
}
#[derive(Serialize)]
pub struct Companion {
    plugin_id: String,
    name: String,
    version: String,
    requirement: InterfaceRequirement,
    optional: bool,
    installed: bool,
    enabled: bool,
    available: bool,
    source_id: Option<String>,
    source_name: Option<String>,
    project_url: Option<String>,
}
fn peer_match(
    consumer: &PluginManifest,
    provided: &PluginManifest,
) -> Option<(InterfaceRequirement, bool)> {
    if consumer.validate().is_err() {
        return None;
    }
    consumer
        .interfaces
        .requires
        .iter()
        .map(|r| (r, false))
        .chain(consumer.interfaces.optional.iter().map(|r| (r, true)))
        .find(|(requirement, _)| provided.supports_interface(requirement))
        .map(|(r, optional)| (r.clone(), optional))
}
#[derive(Serialize)]
pub struct PluginDetails {
    plugin: InstalledPlugin,
    signature: Option<cobble_plugin_host::SignatureStatus>,
    provides: Vec<ProvidedInterface>,
    dependencies: Vec<Dependency>,
    companions: Vec<Companion>,
    companions_truncated: bool,
}
fn inspect(
    registry: &PluginRegistry,
    catalogs: Option<&CatalogStore>,
    id: &str,
) -> Result<PluginDetails, String> {
    super::valid_id(id)?;
    let installed = registry.list().map_err(|error| error.to_string())?;
    let plugin = installed
        .iter()
        .find(|plugin| plugin.manifest.id == id)
        .cloned()
        .ok_or("扩展已不存在")?;
    // Cached subscription metadata is read once; URLs are display hints only.
    let mut hints = std::collections::HashMap::new();
    let mut catalog_entries = Vec::new();
    if let Some(store) = catalogs {
        for source in store
            .list_sources()
            .unwrap_or_default()
            .into_iter()
            .filter(|source| source.enabled)
        {
            if let Ok(Some(snapshot)) = store.read_snapshot(&source.id) {
                for candidate in snapshot.index.plugins {
                    hints.insert(
                        (source.id.clone(), candidate.manifest.id.clone()),
                        (source.name.clone(), candidate.project_url.clone()),
                    );
                    catalog_entries.push((source.id.clone(), source.name.clone(), candidate));
                }
            }
        }
    }
    let mut dependencies = Vec::new();
    for (requirement, optional) in plugin
        .manifest
        .interfaces
        .requires
        .iter()
        .map(|r| (r, false))
        .chain(
            plugin
                .manifest
                .interfaces
                .optional
                .iter()
                .map(|r| (r, true)),
        )
        .take(16)
    {
        let resolution = registry.resolve_interface(requirement, catalogs);
        let active = resolution
            .as_ref()
            .map(|r| r.implementations.as_slice())
            .unwrap_or_default();
        let state = if resolution.is_err() {
            DependencyState::Unsupported
        } else {
            match (active.len(), optional) {
                (0, true) => DependencyState::OptionalMissing,
                (0, false) => DependencyState::RequiredMissing,
                (1, _) => DependencyState::Available,
                _ => DependencyState::SelectionRequired,
            }
        };
        let mut implementations: Vec<_> = installed
            .iter()
            .filter(|p| p.manifest.supports_interface(requirement))
            .map(|p| Implementation {
                plugin_id: p.manifest.id.clone(),
                name: p.manifest.name.clone(),
                version: p
                    .manifest
                    .provided_interfaces()
                    .into_iter()
                    .find(|i| requirement.matches(i))
                    .expect("matched interface")
                    .version,
                enabled: p.enabled,
                available: active.iter().any(|i| i.plugin_id == p.manifest.id),
                problem: p.problem.clone(),
            })
            .collect();
        implementations.sort_by(|a, b| a.plugin_id.cmp(&b.plugin_id));
        let mut candidates = Vec::new();
        let mut candidates_truncated = false;
        if let Ok(resolution) = resolution {
            candidates_truncated = resolution.candidates.len() > 64;
            for candidate in resolution.candidates.into_iter().take(64) {
                let (source_name, project_url) = hints
                    .get(&(candidate.source_id.clone(), candidate.plugin_id.clone()))
                    .cloned()
                    .unwrap_or_default();
                candidates.push(Candidate {
                    source_id: candidate.source_id,
                    source_name,
                    plugin_id: candidate.plugin_id,
                    name: candidate.name,
                    version: candidate.version,
                    project_url,
                });
            }
        }
        dependencies.push(Dependency {
            requirement: requirement.clone(),
            optional,
            state,
            implementations,
            candidates,
            candidates_truncated,
        });
    }
    let mut companions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let ready = cobble_plugin_host::interfaces::ready_plugins(&installed);
    for peer in &installed {
        if peer.manifest.id == id {
            continue;
        }
        if let Some((requirement, optional)) = peer_match(&peer.manifest, &plugin.manifest) {
            seen.insert((None, peer.manifest.id.clone()));
            companions.push(Companion {
                plugin_id: peer.manifest.id.clone(),
                name: peer.manifest.name.clone(),
                version: peer.manifest.version.clone(),
                requirement,
                optional,
                installed: true,
                enabled: peer.enabled,
                available: ready.iter().any(|p| p.manifest.id == peer.manifest.id),
                source_id: None,
                source_name: None,
                project_url: None,
            });
        }
    }
    for (source_id, source_name, peer) in catalog_entries {
        if peer.manifest.id == id
            || !peer.supported()
            || installed.iter().any(|p| p.manifest.id == peer.manifest.id)
        {
            continue;
        }
        if let Some((requirement, optional)) = peer_match(&peer.manifest, &plugin.manifest)
            && seen.insert((Some(source_id.clone()), peer.manifest.id.clone()))
        {
            companions.push(Companion {
                plugin_id: peer.manifest.id,
                name: peer.manifest.name,
                version: peer.manifest.version,
                requirement,
                optional,
                installed: false,
                enabled: false,
                available: false,
                source_id: Some(source_id),
                source_name: Some(source_name),
                project_url: peer.project_url,
            });
        }
    }
    companions.sort_by(|a, b| {
        (!a.installed, &a.source_id, &a.plugin_id).cmp(&(!b.installed, &b.source_id, &b.plugin_id))
    });
    let companions_truncated = companions.len() > 64;
    companions.truncate(64);
    let provides = plugin.manifest.provided_interfaces();
    Ok(PluginDetails {
        signature: registry.signature_status(id).ok(),
        plugin,
        provides,
        dependencies,
        companions,
        companions_truncated,
    })
}
#[tauri::command]
pub fn get_plugin_details(id: String, locale: String) -> Result<PluginDetails, String> {
    super::extension_contributions::validate_locale(&locale)?;
    // Unlike initial registry seeding, inspection must never install bundled code.
    let registry = PluginRegistry::new(super::root("plugins")?)
        .with_trusted_roots(super::plugin_signatures::trusted_roots()?);
    let catalogs = crate::plugin_catalogs::store().ok();
    inspect(&registry, catalogs.as_ref(), &id)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{fs, path::Path};
    fn package(
        root: &Path,
        registry: &PluginRegistry,
        id: &str,
        viewer: bool,
        interfaces: serde_json::Value,
    ) {
        let path = root.join(id);
        fs::create_dir_all(&path).unwrap();
        let manifest = json!({"id":id,"name":id,"description":"Original description","tags":["fixture"],"version":"1.0.0","publisher":"Fixture","api_version":if viewer{"1.0.0"}else{"0.2.0"},"capabilities":[if viewer{"viewer"}else{"blueprint-provider"}],"file_extensions":if viewer{vec!["schem"]}else{vec![]},"provider_origins":if viewer{vec![]}else{vec!["https://community.example"]},"interfaces":interfaces,"component_sha256":cobble_plugin_host::digest(b"\0asm\x0d\0\x01\0")});
        fs::write(
            path.join("plugin.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(path.join("component.wasm"), b"\0asm\x0d\0\x01\0").unwrap();
        registry.install_directory(&path).unwrap();
    }
    #[test]
    fn details_keep_disabled_metadata_and_optional_peer_enable_choices() {
        let root = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(root.path().join("registry"));
        package(
            root.path(),
            &registry,
            "consumer",
            false,
            json!({"optional":[{"id":"launcher:viewer","version":"^1.0.0"}]}),
        );
        package(root.path(), &registry, "viewer", true, json!({}));
        registry.set_enabled("viewer", false).unwrap();
        registry.set_enabled("consumer", false).unwrap();
        let details = inspect(&registry, None, "consumer").unwrap();
        assert!(!details.plugin.enabled);
        assert!(details.plugin.manifest.description.is_some());
        assert_eq!(details.plugin.manifest.tags, ["fixture"]);
        assert!(matches!(
            details.dependencies[0].state,
            DependencyState::OptionalMissing
        ));
        assert!(!details.dependencies[0].implementations[0].enabled);
        assert!(!details.dependencies[0].implementations[0].available);
        let viewer_details = inspect(&registry, None, "viewer").unwrap();
        assert!(
            viewer_details.dependencies.is_empty(),
            "local Viewer does not require an online provider"
        );
        assert_eq!(viewer_details.companions.len(), 1);
        assert_eq!(viewer_details.companions[0].plugin_id, "consumer");
        assert!(viewer_details.companions[0].optional && viewer_details.companions[0].installed);
        assert!(!viewer_details.companions[0].enabled);
        registry.set_enabled("viewer", true).unwrap();
        assert!(matches!(
            inspect(&registry, None, "consumer").unwrap().dependencies[0].state,
            DependencyState::Available
        ));
        assert!(inspect(&registry, None, "../consumer").is_err());
    }
    #[test]
    fn details_required_missing_and_multiple_subscribed_candidates_do_not_install_or_fetch() {
        let root = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(root.path().join("registry"));
        let store = CatalogStore::new(root.path().join("catalogs"));
        package(
            root.path(),
            &registry,
            "consumer",
            false,
            json!({"requires":[{"id":"launcher:viewer","version":"^1.0.0"}]}),
        );
        let viewer: serde_json::Value =
            serde_json::from_slice(&fs::read(root.path().join("consumer/plugin.json")).unwrap())
                .unwrap();
        for (id, url) in [
            ("source-a", "https://a.example/index.json"),
            ("source-b", "https://b.example/index.json"),
        ] {
            let source = store.add_source(id, url).unwrap();
            let mut candidate = viewer.clone();
            candidate["id"] = json!(id);
            candidate["api_version"] = json!("1.0.0");
            candidate["capabilities"] = json!(["viewer"]);
            candidate["file_extensions"] = json!(["schem"]);
            candidate["provider_origins"] = json!([]);
            candidate["interfaces"] = json!({});
            let index = json!({"schema_version":1,"name":id,"plugins":[{"manifest":candidate,"description":"Original fixture","component_url":"https://community.example/viewer.wasm","project_url":"https://github.com/example/viewer"}]});
            store
                .save_snapshot(&source.id, &serde_json::to_vec(&index).unwrap(), 1)
                .unwrap();
        }
        let details = inspect(&registry, Some(&store), "consumer").unwrap();
        let dependency = &details.dependencies[0];
        assert!(matches!(dependency.state, DependencyState::RequiredMissing));
        assert!(!dependency.optional);
        assert_eq!(dependency.candidates.len(), 2);
        assert!(
            dependency
                .candidates
                .iter()
                .all(|c| c.project_url.as_deref() == Some("https://github.com/example/viewer"))
        );
        assert_eq!(registry.list().unwrap().len(), 1);
    }
    #[test]
    fn viewer_reverse_combinations_use_declared_ranges_and_keep_multiple_source_hints() {
        let root = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(root.path().join("registry"));
        let catalogs = CatalogStore::new(root.path().join("catalogs"));
        package(root.path(), &registry, "viewer", true, json!({}));
        let candidate = json!({"id":"community-provider","name":"Independent provider","version":"1.0.0","publisher":"Fixture","api_version":"0.2.0","capabilities":["blueprint-provider"],"file_extensions":[],"provider_origins":["https://community.example"],"interfaces":{"optional":[{"id":"launcher:viewer","version":"^1.0.0"}]},"component_sha256":"a".repeat(64)});
        for (name, url) in [
            ("First", "https://a.example/index.json"),
            ("Second", "https://b.example/index.json"),
        ] {
            let source = catalogs.add_source(name, url).unwrap();
            let mut wrong = candidate.clone();
            wrong["id"] = json!("wrong-viewer-range");
            wrong["interfaces"]["optional"][0]["version"] = json!("^2.0.0");
            let index = json!({"schema_version":1,"name":name,"plugins":[{"manifest":candidate,"description":"Original fixture","component_url":"https://community.example/provider.wasm","project_url":"https://github.com/example/community-provider"},{"manifest":wrong,"description":"Other version","component_url":"https://community.example/wrong.wasm"}]});
            catalogs
                .save_snapshot(&source.id, &serde_json::to_vec(&index).unwrap(), 1)
                .unwrap();
        }
        let details = inspect(&registry, Some(&catalogs), "viewer").unwrap();
        assert!(details.dependencies.is_empty());
        assert_eq!(details.companions.len(), 2);
        assert!(
            details
                .companions
                .iter()
                .all(|p| p.plugin_id == "community-provider"
                    && p.optional
                    && !p.installed
                    && p.project_url.as_deref()
                        == Some("https://github.com/example/community-provider"))
        );
        assert_ne!(
            details.companions[0].source_id,
            details.companions[1].source_id
        );
        assert_eq!(
            registry.list().unwrap().len(),
            1,
            "metadata discovery never installs a candidate"
        );
    }
}
