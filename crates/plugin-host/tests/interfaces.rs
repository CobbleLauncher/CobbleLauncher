// SPDX-License-Identifier: MIT
//! Metadata/lifecycle gate tests; no component execution or network is simulated.
use cobble_plugin_api::{InterfaceRequirement, PluginManifest};
use cobble_plugin_host::{PluginRegistry, catalog::CatalogStore, digest};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
fn manifest(id: &str, provider: bool) -> Value {
    json!({"id":id,"name":id,"version":"1.0.0","publisher":"Original fixture","api_version":if provider {"0.2.0"}else{"1.0.0"},"capabilities":[if provider{"blueprint-provider"}else{"viewer"}],"file_extensions":if provider{vec![]}else{vec!["schem"]},"provider_origins":if provider{vec!["https://community.example"]}else{vec![]},"component_sha256":digest(b"\0asm\x0d\0\x01\0")})
}
fn install(root: &Path, registry: &PluginRegistry, value: &Value) {
    let source = root.join(value["id"].as_str().unwrap());
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("plugin.json"),
        serde_json::to_vec(value).unwrap(),
    )
    .unwrap();
    fs::write(source.join("component.wasm"), b"\0asm\x0d\0\x01\0").unwrap();
    registry.install_directory(&source).unwrap();
}
#[test]
fn required_interfaces_gate_direct_execution_optional_peers_do_not() {
    let temp = tempfile::tempdir().unwrap();
    let registry = PluginRegistry::new(temp.path().join("registry"));
    let requirement = InterfaceRequirement::blueprint_provider();
    let mut required = manifest("requires-provider", false);
    required["interfaces"] = json!({"requires":[requirement]});
    let mut optional = manifest("optional-provider", false);
    optional["interfaces"] = json!({"optional":[requirement]});
    install(temp.path(), &registry, &required);
    install(temp.path(), &registry, &optional);
    assert!(registry.resolve_enabled("optional-provider").is_ok());
    assert_eq!(
        registry
            .resolve_enabled("requires-provider")
            .unwrap_err()
            .code,
        "PLUGIN_MISSING_INTERFACE"
    );
    let error = registry
        .open_viewer(
            Path::new("must-not-spawn.exe"),
            "requires-provider",
            b"",
            Arc::new(AtomicBool::new(false)),
        )
        .err()
        .unwrap();
    assert_eq!(error.code, "PLUGIN_MISSING_INTERFACE");
    install(
        temp.path(),
        &registry,
        &manifest("independent-provider", true),
    );
    assert!(
        registry.resolve_enabled("requires-provider").is_ok(),
        "legacy0.2 implementation is synthesized without reinstall"
    );
    let result = registry.resolve_interface(&requirement, None).unwrap();
    assert_eq!(result.implementations[0].plugin_id, "independent-provider");
    registry.set_enabled("independent-provider", false).unwrap();
    assert!(registry.resolve_enabled("requires-provider").is_err());
    assert!(registry.resolve_enabled("optional-provider").is_ok());
    assert_eq!(
        registry.list().unwrap().len(),
        3,
        "metadata remains visible"
    );
    registry.set_enabled("independent-provider", true).unwrap();
    assert!(registry.resolve_enabled("requires-provider").is_ok());
    let mut broken = manifest("independent-provider", true);
    broken["interfaces"] =
        json!({"requires":[{"id":"launcher:blueprint-provider","version":"^0.3.0"}]});
    fs::write(
        temp.path()
            .join("registry/independent-provider/plugin.json"),
        serde_json::to_vec(&broken).unwrap(),
    )
    .unwrap();
    assert!(registry.resolve_enabled("independent-provider").is_err());
    assert!(
        registry.resolve_enabled("requires-provider").is_err(),
        "required readiness propagates without recursive installation"
    );
    assert!(registry.resolve_enabled("optional-provider").is_ok());
}
#[test]
fn subscription_candidates_are_local_enabled_and_exact_host_abi_supported() {
    let temp = tempfile::tempdir().unwrap();
    let registry = PluginRegistry::new(temp.path().join("registry"));
    let catalogs = CatalogStore::new(temp.path().join("catalogs"));
    let source = catalogs
        .add_source("Community", "https://community.example/index.json")
        .unwrap();
    let mut provider = manifest("community-provider", true);
    provider["interfaces"] =
        json!({"provides":[{"id":"launcher:blueprint-provider","version":"0.2.0"}]});
    let index = json!({"schema_version":1,"name":"Community","plugins":[{"manifest":provider,"description":"Original fixture","component_url":"https://community.example/provider.wasm"}]});
    catalogs
        .save_snapshot(&source.id, &serde_json::to_vec(&index).unwrap(), 1)
        .unwrap();
    let requirement = InterfaceRequirement::blueprint_provider();
    let result = registry
        .resolve_interface(&requirement, Some(&catalogs))
        .unwrap();
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].source_id, source.id);
    catalogs.set_enabled(&source.id, false).unwrap();
    assert!(
        registry
            .resolve_interface(&requirement, Some(&catalogs))
            .unwrap()
            .candidates
            .is_empty()
    );
    catalogs.set_enabled(&source.id, true).unwrap();
    install(temp.path(), &registry, &provider);
    let result = registry
        .resolve_interface(&requirement, Some(&catalogs))
        .unwrap();
    assert_eq!(result.implementations.len(), 1);
    assert!(result.candidates.is_empty());
    fs::write(
        temp.path().join("catalogs/sources.json"),
        b"invalid source config",
    )
    .unwrap();
    assert_eq!(
        registry
            .resolve_interface(&requirement, Some(&catalogs))
            .unwrap()
            .implementations
            .len(),
        1,
        "broken subscriptions cannot hide installed implementations"
    );
    provider["interfaces"]["provides"][0]["version"] = json!("0.2.1");
    let unsupported: PluginManifest = serde_json::from_value(provider).unwrap();
    assert!(requirement.matches(&unsupported.interfaces.provides[0]));
    assert!(
        !unsupported.supports_interface(&requirement),
        "SemVer compatibility never fabricates an implemented WIT ABI"
    );
}
