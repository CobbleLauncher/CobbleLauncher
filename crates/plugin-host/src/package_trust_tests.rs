// SPDX-License-Identifier: MIT
use super::*;
fn manifest(bytes: &[u8]) -> PluginManifest {
    serde_json::from_value(serde_json::json!({"id":"proof-fixture","name":"Fixture","version":"1","api_version":"0.2.0","publisher":"Original fixture","capabilities":["report"],"file_extensions":["json"],"component_sha256":digest(bytes)})).unwrap()
}
fn bound(package: &Path, path: &str) -> BoundFile {
    let bytes = fs::read(package.join(path)).unwrap();
    BoundFile {
        path: path.into(),
        length: bytes.len() as u64,
        sha256: digest(&bytes),
    }
}
fn receipt(package: &Path, manifest: &PluginManifest, root: &str, version: u64) -> Receipt {
    Receipt {
        schema_version: 1,
        plugin_id: manifest.id.clone(),
        manifest_sha256: digest(&fs::read(package.join("plugin.json")).unwrap()),
        root_sha256: Some(root.into()),
        metadata_version: version,
        bound_files: vec![
            bound(package, "plugin.json"),
            bound(package, "component.wasm"),
            bound(package, ".cobble-signatures/metadata/timestamp.json"),
        ],
    }
}
fn verified(receipt: &Receipt) -> VerifiedPackage {
    VerifiedPackage {
        plugin_id: receipt.plugin_id.clone(),
        version: "1".into(),
        root_sha256: receipt.root_sha256.clone().unwrap(),
        manifest_sha256: receipt.manifest_sha256.clone(),
        descriptor_target: "plugins/proof-fixture/1/package.json".into(),
        metadata_version: receipt.metadata_version,
        files_verified: receipt.bound_files.len(),
        bound_files: receipt.bound_files.clone(),
    }
}
#[test]
fn complete_approval_identity_keeps_old_proof_usable_and_removal_cannot_downgrade() {
    let dir = tempfile::tempdir().unwrap();
    let registry = dir.path().join("registry");
    let package = registry.join("proof-fixture");
    let roots = dir.path().join("roots");
    fs::create_dir_all(package.join(".cobble-signatures/metadata")).unwrap();
    fs::create_dir(&roots).unwrap();
    let root = b"Previously approved public-root fixture";
    let root_id = digest(root);
    fs::write(roots.join(format!("{root_id}.json")), root).unwrap();
    let component = b"\0asm\x0d\0\x01\0";
    let manifest = manifest(component);
    fs::write(
        package.join("plugin.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(package.join("component.wasm"), component).unwrap();
    // Runtime receipt checks deliberately do not parse freshness; authentic TUF verification is tested separately.
    fs::write(
        package.join(".cobble-signatures/metadata/timestamp.json"),
        b"approved earlier; now expired",
    )
    .unwrap();
    let old = receipt(&package, &manifest, &root_id, 1);
    persist(&registry, &old).unwrap();
    let mut new = receipt(&package, &manifest, &root_id, 2);
    new.bound_files[2].sha256 = digest(b"new proof");
    new.bound_files[2].length = 9;
    persist(&registry, &new).unwrap();
    assert_eq!(receipts(&registry, &manifest.id).unwrap().len(), 2);
    verify_installed(&registry, &package, &manifest, Some(&roots)).unwrap();
    assert!(
        matches!(installed_signature_status(&registry, &package, &manifest, Some(&roots)).unwrap(), SignatureStatus::Verified { root_id, .. } if root_id == digest(root))
    );
    assert!(verify_installed(&registry, &package, &manifest, None).is_err());
    fs::write(
        package.join(".cobble-signatures/metadata/timestamp.json"),
        b"new proof",
    )
    .unwrap();
    verify_installed(&registry, &package, &manifest, Some(&roots)).unwrap();
    persist(
        &registry,
        &Receipt {
            schema_version: 1,
            plugin_id: manifest.id.clone(),
            manifest_sha256: old.manifest_sha256,
            root_sha256: None,
            metadata_version: 0,
            bound_files: vec![],
        },
    )
    .unwrap();
    mark_verified(&registry, &manifest.id).unwrap();
    fs::remove_dir_all(package.join(".cobble-signatures")).unwrap();
    assert!(verify_installed(&registry, &package, &manifest, Some(&roots)).is_err());
}
#[test]
fn same_root_sequence_rejects_rollback_or_equivocation() {
    let prior = Receipt {
        schema_version: 1,
        plugin_id: "proof-fixture".into(),
        manifest_sha256: "a".repeat(64),
        root_sha256: Some("b".repeat(64)),
        metadata_version: 5,
        bound_files: vec![],
    };
    let mut next = verified(&prior);
    next.metadata_version = 4;
    assert!(check_revision(&prior, &next).is_err());
    next.metadata_version = 5;
    next.manifest_sha256 = "c".repeat(64);
    assert!(check_revision(&prior, &next).is_err());
    next.manifest_sha256 = prior.manifest_sha256.clone();
    next.bound_files.push(BoundFile {
        path: "plugin.json".into(),
        length: 1,
        sha256: "d".repeat(64),
    });
    assert!(check_revision(&prior, &next).is_err());
    next.metadata_version = 6;
    assert!(check_revision(&prior, &next).is_ok());
}
#[test]
#[ignore = "requires explicitly selected signed package and approved public root"]
fn actual_signed_install_preserves_bytes_and_proof_then_rejects_deletion() {
    let package =
        PathBuf::from(std::env::var_os("COBBLE_TEST_SIGNED_PACKAGE").expect("signed package"));
    let root =
        fs::read(std::env::var_os("COBBLE_TEST_TRUSTED_ROOT").expect("public root")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let roots = dir.path().join("roots");
    fs::create_dir(&roots).unwrap();
    fs::write(roots.join(format!("{}.json", digest(&root))), &root).unwrap();
    assert!(matches!(
        package_signature_status(&package, Some(&roots)).unwrap(),
        SignatureStatus::Verified { .. }
    ));
    assert!(package_signature_status(&package, None).is_err());
    let registry =
        crate::PluginRegistry::new(dir.path().join("plugins")).with_trusted_roots(roots.clone());
    let installed = registry.install_directory(&package).unwrap();
    let destination = registry.root.join(&installed.manifest.id);
    assert_eq!(
        fs::read(package.join("plugin.json")).unwrap(),
        fs::read(destination.join("plugin.json")).unwrap()
    );
    registry
        .read_installed_package(&installed.manifest.id)
        .unwrap();
    fs::remove_dir_all(destination.join(cobble_plugin_trust::PROOF_DIR)).unwrap();
    assert!(
        registry
            .read_installed_package(&installed.manifest.id)
            .is_err()
    );
    assert!(registry.set_enabled(&installed.manifest.id, true).is_err());
}
