// SPDX-License-Identifier: MIT
//! Catalog downloads prepare the existing native approval flow; they never install.
use cobble_net::{Broker, BrokerConfig};
use cobble_plugin_api::{MAX_COMPONENT_BYTES, PluginManifest};
use cobble_plugin_host::catalog::{CatalogPlugin, CatalogSnapshot, CatalogSource, CatalogStore};

fn store() -> Result<CatalogStore, String> {
    Ok(CatalogStore::new(
        dirs::data_local_dir()
            .ok_or("无法找到数据目录")?
            .join("CobbleLauncher/plugin-catalogs"),
    ))
}
fn selection(
    store: &CatalogStore,
    source_id: &str,
    plugin_id: &str,
) -> Result<(CatalogSource, CatalogSnapshot, CatalogPlugin), String> {
    let source = store
        .list_sources()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|source| source.id == source_id)
        .ok_or("找不到这个插件目录")?;
    if !source.enabled {
        return Err("请先启用这个插件目录".into());
    }
    let snapshot = store
        .read_snapshot(source_id)
        .map_err(|error| error.to_string())?
        .ok_or("请先刷新这个插件目录")?;
    let plugin = snapshot
        .index
        .plugins
        .iter()
        .find(|plugin| plugin.manifest.id == plugin_id)
        .cloned()
        .ok_or("插件已不在这个目录中，请刷新后重试")?;
    if !plugin.supported() {
        return Err("当前启动器不支持这个插件的 API 或权限".into());
    }
    Ok((source, snapshot, plugin))
}

#[tauri::command]
pub async fn preview_catalog_plugin(
    source_id: String,
    plugin_id: String,
) -> Result<PluginManifest, String> {
    let revision = super::plugins::begin_plugin_preview()?;
    let store = store()?;
    let selected = selection(&store, &source_id, &plugin_id)?;
    let artifact = selected.2.package.as_ref();
    let url = artifact.map_or(selected.2.component_url.as_str(), |package| {
        package.url.as_str()
    });
    let limit = artifact.map_or(MAX_COMPONENT_BYTES as u64, |package| package.size);
    let response = Broker::new(BrokerConfig::default())
        .map_err(|error| error.to_string())?
        .get(url, None, limit)
        .await
        .map_err(|error| error.to_string())?;
    if response.metadata.status != 200 {
        return Err(format!("插件下载失败（HTTP {}）", response.metadata.status));
    }
    if selection(&store, &source_id, &plugin_id)? != selected {
        return Err("插件目录已变化，请重新选择插件".into());
    }
    if let Some(artifact) = artifact {
        return super::plugins::prepare_catalog_bundle(
            selected.2.manifest.clone(),
            artifact,
            &response.body,
            revision,
        );
    }
    let icon = selected.2.icon_bytes().map_err(|error| error.to_string())?;
    super::plugins::prepare_catalog_package(
        selected.2.manifest,
        &response.body,
        icon.as_deref(),
        revision,
    )
}

pub(super) fn extract_bundle(
    manifest: &PluginManifest,
    artifact: &cobble_plugin_host::catalog::CatalogPackage,
    bytes: &[u8],
    roots: &std::path::Path,
) -> Result<
    (
        std::sync::Arc<tempfile::TempDir>,
        cobble_plugin_host::SignatureStatus,
    ),
    String,
> {
    if bytes.len() as u64 != artifact.size
        || artifact.size > cobble_plugin_host::catalog::MAX_CATALOG_PACKAGE_BYTES
        || cobble_plugin_host::digest(bytes) != artifact.sha256.to_ascii_lowercase()
    {
        return Err("PLUGIN_CATALOG_PACKAGE_DIGEST".into());
    }
    let staging = std::sync::Arc::new(
        tempfile::Builder::new()
            .prefix("cobble-plugin-preview-")
            .tempdir()
            .map_err(|error| error.to_string())?,
    );
    let directory =
        cap_std::fs::Dir::open_ambient_dir(staging.path(), cap_std::ambient_authority())
            .map_err(|error| error.to_string())?;
    cobble_store::extract_zip(
        std::io::Cursor::new(bytes),
        &directory,
        cobble_store::ArchiveLimits {
            max_archive_bytes: cobble_plugin_host::catalog::MAX_CATALOG_PACKAGE_BYTES,
            max_entries: 1024,
            max_entry_bytes: MAX_COMPONENT_BYTES as u64,
            max_total_bytes: 128 * 1024 * 1024,
            max_ratio: 200,
        },
    )
    .map_err(|error| error.to_string())?;
    let (actual, _) =
        cobble_plugin_host::read_package_with_trusted_roots(staging.path(), Some(roots))
            .map_err(|error| error.to_string())?;
    if &actual != manifest {
        return Err("PLUGIN_REVIEW_MISMATCH".into());
    }
    let signature = cobble_plugin_host::package_signature_status(staging.path(), Some(roots))
        .map_err(|error| error.to_string())?;
    if signature == cobble_plugin_host::SignatureStatus::Unsigned {
        return Err("PLUGIN_SIGNATURE_REQUIRED".into());
    }
    Ok((staging, signature))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn zip_files(files: &[(String, Vec<u8>)]) -> Vec<u8> {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, bytes) in files {
            writer
                .start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    fn artifact(bytes: &[u8]) -> cobble_plugin_host::catalog::CatalogPackage {
        cobble_plugin_host::catalog::CatalogPackage {
            url: "https://example.invalid/package.zip".into(),
            sha256: cobble_plugin_host::digest(bytes),
            size: bytes.len() as u64,
        }
    }
    #[test]
    fn bundle_rejects_transport_changes_unsigned_and_traversal() {
        let component = b"\0asm\x0d\0\x01\0".to_vec();
        let manifest: PluginManifest = serde_json::from_value(serde_json::json!({"id":"bundle-fixture","name":"Fixture","version":"1","api_version":"0.2.0","publisher":"Fixture","capabilities":["report"],"file_extensions":["json"],"component_sha256":cobble_plugin_host::digest(&component)})).unwrap();
        let files = vec![
            ("plugin.json".into(), serde_json::to_vec(&manifest).unwrap()),
            ("component.wasm".into(), component),
        ];
        let bytes = zip_files(&files);
        let roots = tempfile::tempdir().unwrap();
        assert!(
            extract_bundle(&manifest, &artifact(&bytes), &bytes, roots.path())
                .err()
                .unwrap()
                .contains("PLUGIN_SIGNATURE_REQUIRED")
        );
        let mut changed = artifact(&bytes);
        changed.sha256 = "0".repeat(64);
        assert!(
            extract_bundle(&manifest, &changed, &bytes, roots.path())
                .err()
                .unwrap()
                .contains("DIGEST")
        );
        let bytes = zip_files(&[("../escape".into(), vec![1])]);
        assert!(extract_bundle(&manifest, &artifact(&bytes), &bytes, roots.path()).is_err());
    }
    #[test]
    #[ignore = "requires the explicitly generated local public package fixture"]
    fn local_signed_bundle_preserves_bytes_and_rejects_missing_root() {
        fn collect(
            root: &std::path::Path,
            current: &std::path::Path,
            files: &mut Vec<(String, Vec<u8>)>,
        ) {
            for entry in std::fs::read_dir(current).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    collect(root, &path, files);
                } else {
                    files.push((
                        path.strip_prefix(root)
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .replace('\\', "/"),
                        std::fs::read(path).unwrap(),
                    ));
                }
            }
        }
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target/plugin-trust-local");
        let package = fixture.join("config-doctor");
        let mut files = Vec::new();
        collect(&package, &package, &mut files);
        let manifest: PluginManifest =
            serde_json::from_slice(&std::fs::read(package.join("plugin.json")).unwrap()).unwrap();
        let bytes = zip_files(&files);
        let roots = tempfile::tempdir().unwrap();
        assert!(extract_bundle(&manifest, &artifact(&bytes), &bytes, roots.path()).is_err());
        let root = std::fs::read(fixture.join("1.root.json")).unwrap();
        std::fs::write(
            roots
                .path()
                .join(format!("{}.json", cobble_plugin_host::digest(&root))),
            root,
        )
        .unwrap();
        let (staged, status) =
            extract_bundle(&manifest, &artifact(&bytes), &bytes, roots.path()).unwrap();
        assert!(matches!(
            status,
            cobble_plugin_host::SignatureStatus::Verified { .. }
        ));
        for (path, original) in files {
            assert_eq!(std::fs::read(staged.path().join(path)).unwrap(), original);
        }
    }
    #[test]
    fn selection_requires_current_enabled_source_and_supported_snapshot_entry() {
        let temporary = tempfile::tempdir().unwrap();
        let store = CatalogStore::new(temporary.path().to_owned());
        let source = store
            .add_source("Synthetic source", "https://example.invalid/index.json")
            .unwrap();
        assert!(selection(&store, &source.id, "synthetic-viewer").is_err());
        let manifest = PluginManifest {
            frontend: None,
            permissions: None,
            description: None,
            tags: Vec::new(),
            icon: None,
            id: "synthetic-viewer".into(),
            name: "Synthetic".into(),
            version: "0.1.0".into(),
            api_version: cobble_plugin_api::API_VERSION.into(),
            publisher: "Original test fixture".into(),
            capabilities: vec!["viewer".into()],
            tool_form: None,
            composer_permissions: Default::default(),
            interfaces: Default::default(),
            file_extensions: vec!["schem".into()],
            report_protocols: vec![],
            contributions: None,
            provider_origins: vec![],
            component_sha256: "a".repeat(64),
        };
        let mut index = cobble_plugin_host::catalog::CatalogIndex {
            schema_version: 1,
            name: "Synthetic".into(),
            plugins: vec![CatalogPlugin {
                package: None,
                icon_png_base64: None,
                manifest,
                description: String::new(),
                component_url: "https://example.invalid/viewer.wasm".into(),
                project_url: None,
            }],
        };
        store
            .save_snapshot(&source.id, &serde_json::to_vec(&index).unwrap(), 1)
            .unwrap();
        let pinned = selection(&store, &source.id, "synthetic-viewer").unwrap();
        index.plugins[0].manifest.version = "0.2.0".into();
        store
            .save_snapshot(&source.id, &serde_json::to_vec(&index).unwrap(), 2)
            .unwrap();
        assert_ne!(
            selection(&store, &source.id, "synthetic-viewer").unwrap(),
            pinned
        );
        store.set_enabled(&source.id, false).unwrap();
        assert!(selection(&store, &source.id, "synthetic-viewer").is_err());
        store.set_enabled(&source.id, true).unwrap();
        index.plugins[0].manifest.api_version = "2.0.0".into();
        store
            .save_snapshot(&source.id, &serde_json::to_vec(&index).unwrap(), 3)
            .unwrap();
        assert!(selection(&store, &source.id, "synthetic-viewer").is_err());
        store.remove_source(&source.id).unwrap();
        assert!(selection(&store, &source.id, "synthetic-viewer").is_err());
    }
}
