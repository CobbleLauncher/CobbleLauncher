// SPDX-License-Identifier: MIT
use cobble_plugin_host::{PluginRegistry, digest};
use std::{fs, path::Path};

fn package(path: &Path, version: &str) {
    fs::create_dir_all(path).unwrap();
    let bytes = b"\0asm\x0d\0\x01\0";
    let manifest = serde_json::json!({
        "id":"update-fixture", "name":"Update fixture", "publisher":"Fixture", "version":version,
        "api_version":"1.0.0", "capabilities":["viewer"], "file_extensions":["schem"],
        "component_sha256":digest(bytes)
    });
    fs::write(
        path.join("plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(path.join("component.wasm"), bytes).unwrap();
}

#[test]
fn approved_update_keeps_disabled_state_and_rejects_invalid_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let registry = PluginRegistry::new(temp.path().join("registry"));
    package(&source, "1.0.0");
    registry.install_directory(&source).unwrap();
    registry.set_enabled("update-fixture", false).unwrap();
    package(&source, "2.0.0");
    let updated = registry.update_directory(&source).unwrap();
    assert_eq!(updated.manifest.version, "2.0.0");
    assert!(!updated.enabled);
    fs::write(source.join("component.wasm"), b"invalid").unwrap();
    assert!(registry.update_directory(&source).is_err());
    assert_eq!(registry.list().unwrap()[0].manifest.version, "2.0.0");
}

#[test]
fn interrupted_directory_swap_restores_previous_package() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("registry");
    let registry = PluginRegistry::new(root.clone());
    package(&source, "1.0.0");
    registry.install_directory(&source).unwrap();
    fs::rename(
        root.join("update-fixture"),
        root.join(".previous-update-fixture"),
    )
    .unwrap();
    assert_eq!(registry.list().unwrap()[0].manifest.version, "1.0.0");
    assert!(!root.join(".previous-update-fixture").exists());
}

#[test]
fn unsupported_provider_remains_visible_updatable_and_removable() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("registry");
    let installed = root.join("update-fixture");
    package(&installed, "1.0.0");
    let path = installed.join("plugin.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["api_version"] = serde_json::json!("0.1.0");
    manifest["capabilities"] = serde_json::json!(["blueprint-provider"]);
    manifest["file_extensions"] = serde_json::json!([]);
    manifest["provider_origins"] = serde_json::json!(["https://community.example"]);
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(installed.join("disabled"), b"disabled").unwrap();
    let registry = PluginRegistry::new(root.clone());
    let listed = registry.list().unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].problem.is_some());
    assert!(!listed[0].enabled);
    assert!(cobble_plugin_host::read_package(&installed).is_err());
    fs::rename(&installed, root.join(".previous-update-fixture")).unwrap();
    assert_eq!(
        registry.list().unwrap().len(),
        1,
        "unsupported backup must recover"
    );
    let source = temp.path().join("source");
    package(&source, "2.0.0");
    manifest["api_version"] = serde_json::json!(cobble_plugin_api::PROVIDER_API_VERSION);
    manifest["version"] = serde_json::json!("2.0.0");
    fs::write(
        source.join("plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let updated = registry.update_directory(&source).unwrap();
    assert_eq!(
        updated.manifest.api_version,
        cobble_plugin_api::PROVIDER_API_VERSION
    );
    assert!(!updated.enabled);
    let backup = root.join(".previous-update-fixture");
    package(&backup, "1.0.0");
    let mut previous = manifest.clone();
    previous["api_version"] = serde_json::json!("0.1.0");
    fs::write(
        backup.join("plugin.json"),
        serde_json::to_vec(&previous).unwrap(),
    )
    .unwrap();
    assert_eq!(
        registry.list().unwrap()[0].manifest.api_version,
        cobble_plugin_api::PROVIDER_API_VERSION
    );
    assert!(
        !backup.exists(),
        "obsolete unsupported backup must be cleaned after replacement"
    );
    fs::write(
        installed.join("plugin.json"),
        serde_json::to_vec(&{
            manifest["api_version"] = serde_json::json!("0.1.0");
            manifest
        })
        .unwrap(),
    )
    .unwrap();
    registry.uninstall("update-fixture").unwrap();
    assert!(registry.list().unwrap().is_empty());
}

#[test]
fn permission_upgrade_requires_exact_review_and_denial_preserves_previous() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let registry = PluginRegistry::new(temp.path().join("registry"));
    package(&source, "1.0.0");
    registry.install_directory(&source).unwrap();
    package(&source, "2.0.0");
    registry.update_directory(&source).unwrap();
    let manifest_path = source.join("plugin.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    value["version"] = serde_json::json!("3.0.0");
    value["file_extensions"] = serde_json::json!(["schem", "litematic"]);
    fs::write(&manifest_path, serde_json::to_vec(&value).unwrap()).unwrap();
    let reviewed: cobble_plugin_api::PluginManifest =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        registry.update_directory(&source).unwrap_err().code,
        "PLUGIN_PERMISSION_APPROVAL_REQUIRED"
    );
    assert_eq!(registry.list().unwrap()[0].manifest.version, "2.0.0");
    let mut mismatched = reviewed.clone();
    mismatched.name = "Different reviewed package".into();
    assert_eq!(
        registry
            .update_directory_approved(&source, &mismatched)
            .unwrap_err()
            .code,
        "PLUGIN_REVIEW_MISMATCH"
    );
    assert_eq!(registry.list().unwrap()[0].manifest.version, "2.0.0");
    registry
        .update_directory_approved(&source, &reviewed)
        .unwrap();
    value["version"] = serde_json::json!("4.0.0");
    value["file_extensions"] = serde_json::json!(["schem"]);
    fs::write(&manifest_path, serde_json::to_vec(&value).unwrap()).unwrap();
    registry.update_directory(&source).unwrap();
    assert_eq!(registry.list().unwrap()[0].manifest.version, "4.0.0");
}
