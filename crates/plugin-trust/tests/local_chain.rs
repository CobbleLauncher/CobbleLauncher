// SPDX-License-Identifier: MIT
use cobble_plugin_trust::*;
use std::{fs, num::NonZeroU64};
#[test]
fn delegated_chain_binds_actual_package_and_external_root() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
    let temp = tempfile::tempdir().unwrap();
    let keys = temp.path().join("keys");
    fs::create_dir(&keys).unwrap();
    for name in ["root", "publisher", "feed", "other"] {
        create_key(&keys.join(name)).unwrap();
    }
    assert!(create_key(&keys.join("root")).is_err());
    let expiry = "2036-01-01T00:00:00Z".parse().unwrap();
    let root = temp.path().join("root.json");
    create_root(
        &keys.join("root"),
        &keys.join("publisher"),
        &keys.join("feed"),
        &root,
        expiry,
    )
    .await
    .unwrap();
    let package = temp.path().join("package");
    fs::create_dir(&package).unwrap();
    let component = b"\0asm\x0d\0\x01\0";
    use sha2::{Digest, Sha256};
    let mut manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/pack-metadata/plugin.json")).unwrap();
    manifest["component_sha256"] = format!("{:x}", Sha256::digest(component)).into();
    let html = b"<!doctype html><title>Original fixture</title>";
    let index = serde_json::to_vec(&serde_json::json!({"schema_version":1,"files":[{"path":"ui/index.html","bytes":html.len(),"sha256":format!("{:x}",Sha256::digest(html))}]})).unwrap();
    manifest["frontend"] = serde_json::json!({"api_version":"0.1.0","entry":"ui/index.html","assets_sha256":format!("{:x}",Sha256::digest(&index))});
    manifest["interfaces"]["provides"].as_array_mut().unwrap().push(serde_json::json!({"id":"launcher:web-frontend","version":"0.1.0"}));
    manifest["permissions"].as_array_mut().unwrap().push(serde_json::json!({"permission":"launcher:ui/webview"}));
    fs::create_dir(package.join("ui")).unwrap();
    fs::write(package.join("ui/index.html"),html).unwrap();
    fs::write(package.join("frontend-assets.json"),index).unwrap();
    fs::write(
        package.join("plugin.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(package.join("component.wasm"), component).unwrap();
    let verified = sign_package(
        &root,
        &keys.join("publisher"),
        &keys.join("feed"),
        &package,
        NonZeroU64::MIN,
        expiry,
    )
    .await
    .unwrap();
    assert_eq!(verified.metadata_version, 1);
    assert_eq!(verified.bound_files.len(), 10);
    verify_bound_files(&package, &verified.bound_files).unwrap();
    let bytes = fs::read(&root).unwrap();
    let original_manifest = fs::read(package.join("plugin.json")).unwrap();
    manifest["name"] = "Changed without a signature".into();
    fs::write(package.join("plugin.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(verify_package(&package, &bytes).await.is_err());
    fs::write(package.join("plugin.json"), original_manifest).unwrap();
    fs::write(package.join("ui/index.html"),b"tampered").unwrap();
    assert!(verify_package(&package,&bytes).await.is_err());
    fs::write(package.join("ui/index.html"),html).unwrap();
    let package_clone = package.clone();
    let root_clone = bytes.clone();
    tokio::task::spawn_blocking(move || verify_package_blocking(&package_clone, &root_clone))
        .await
        .unwrap()
        .unwrap();
    let other = temp.path().join("other-root.json");
    create_root(
        &keys.join("other"),
        &keys.join("other"),
        &keys.join("other"),
        &other,
        expiry,
    )
    .await
    .unwrap();
    assert!(
        verify_package(&package, &fs::read(other).unwrap())
            .await
            .is_err()
    );
    fs::write(package.join("component.wasm"), b"changed").unwrap();
    assert!(verify_package(&package, &bytes).await.is_err());
    assert!(verify_bound_files(&package, &verified.bound_files).is_err());
    fs::write(package.join("component.wasm"), component).unwrap();
    fs::write(package.join(".cobble-signatures/metadata/feed.json"), b"{}").unwrap();
    assert!(verify_package(&package, &bytes).await.is_err());
});
}
