// SPDX-License-Identifier: MIT
use cobble_plugin_api::PluginManifest;
use cobble_plugin_host::{PluginRegistry, digest, read_frontend_asset, read_package};
use std::{fs, path::Path};
fn package(root: &Path, version: &str, text: &[u8]) -> PluginManifest {
    fs::create_dir_all(root.join("ui/assets")).unwrap();
    fs::write(root.join("ui/index.html"), text).unwrap();
    fs::write(
        root.join("ui/assets/app.js"),
        b"export const original = true;",
    )
    .unwrap();
    let index=serde_json::to_vec(&serde_json::json!({"schema_version":1,"files":[{"path":"ui/index.html","sha256":digest(text),"bytes":text.len()},{"path":"ui/assets/app.js","sha256":digest(b"export const original = true;"),"bytes":29}]})).unwrap();
    fs::write(root.join("frontend-assets.json"), &index).unwrap();
    let component = b"\0asm\x0d\0\x01\0";
    fs::write(root.join("component.wasm"), component).unwrap();
    let manifest:PluginManifest=serde_json::from_value(serde_json::json!({"id":"frontend-fixture","name":"Original frontend fixture","version":version,"api_version":"0.2.0","publisher":"Original fixture","capabilities":["report"],"file_extensions":["txt"],"permissions":[{"permission":"launcher:resource/selected/read","extensions":["txt"]},{"permission":"launcher:ui/webview"}],"frontend":{"api_version":"0.1.0","entry":"ui/index.html","assets_sha256":digest(&index)},"component_sha256":digest(component)})).unwrap();
    fs::write(
        root.join("plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    manifest
}
#[test]
fn indexed_frontend_survives_install_update_and_rejects_tampering() {
    let source = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let registry = PluginRegistry::new(root.path().into());
    let first = package(source.path(), "1.0.0", b"<!doctype html><h1>Original</h1>");
    read_package(source.path()).unwrap();
    registry.install_directory(source.path()).unwrap();
    let installed = root.path().join(&first.id);
    assert_eq!(
        read_frontend_asset(&installed, &first, "ui/index.html")
            .unwrap()
            .media_type,
        "text/html; charset=utf-8"
    );
    let second = package(source.path(), "2.0.0", b"<!doctype html><h1>Changed</h1>");
    registry
        .update_directory_approved(source.path(), &second)
        .unwrap();
    assert!(read_frontend_asset(&installed, &first, "ui/index.html").is_err());
    assert!(read_frontend_asset(&installed, &second, "ui/assets/app.js").is_ok());
    fs::write(installed.join("ui/assets/app.js"), b"changed").unwrap();
    assert!(read_frontend_asset(&installed, &second, "ui/assets/app.js").is_err());
    assert!(read_package(&installed).is_err());
}
#[test]
fn only_indexed_safe_paths_are_served_and_wasm_remains_required() {
    let source = tempfile::tempdir().unwrap();
    let manifest = package(source.path(), "1.0.0", b"<!doctype html>");
    fs::write(source.path().join("ui/private.json"), b"unlisted").unwrap();
    for path in [
        "ui/private.json",
        "../plugin.json",
        "ui/../plugin.json",
        "ui/assets/app.js?query",
        "/ui/index.html",
    ] {
        assert!(read_frontend_asset(source.path(), &manifest, path).is_err());
    }
    fs::remove_file(source.path().join("component.wasm")).unwrap();
    assert!(read_package(source.path()).is_err());
}
