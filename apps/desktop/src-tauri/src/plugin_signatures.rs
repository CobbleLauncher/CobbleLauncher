// SPDX-License-Identifier: MIT
//! The official publisher root ships with the application; other roots require explicit approval.
use cobble_plugin_api::{PluginError, PluginManifest};
use std::io::Write;
use std::path::{Path, PathBuf};
const OFFICIAL_ROOT: &[u8] = include_bytes!("../resources/plugin-trust/1.root.json");
const OFFICIAL_ROOT_ID: &str = "7709b03a49742b0933d4703e32c380a5eb5bf33d449ed1d268a1a5c8b6c198fe";

fn seed_official_root(roots: &Path) -> Result<(), String> {
    if cobble_plugin_host::digest(OFFICIAL_ROOT) != OFFICIAL_ROOT_ID {
        return Err("PLUGIN_SIGNATURE_OFFICIAL_ROOT".into());
    }
    std::fs::create_dir_all(roots).map_err(|_| "PLUGIN_SIGNATURE_ROOT_STORE")?;
    let destination = roots.join(format!("{OFFICIAL_ROOT_ID}.json"));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
    {
        Ok(mut file) => file
            .write_all(OFFICIAL_ROOT)
            .map_err(|_| "PLUGIN_SIGNATURE_ROOT_STORE".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(&destination)
                .map_err(|_| "PLUGIN_SIGNATURE_ROOT_STORE")?;
            let linked = metadata.file_type().is_symlink();
            #[cfg(windows)]
            let linked = {
                use std::os::windows::fs::MetadataExt;
                linked || metadata.file_attributes() & 0x400 != 0
            };
            if linked
                || !metadata.is_file()
                || metadata.len() != OFFICIAL_ROOT.len() as u64
                || std::fs::read(&destination).map_err(|_| "PLUGIN_SIGNATURE_ROOT_STORE")?
                    != OFFICIAL_ROOT
            {
                return Err("PLUGIN_SIGNATURE_OFFICIAL_ROOT".into());
            }
            Ok(())
        }
        Err(_) => Err("PLUGIN_SIGNATURE_ROOT_STORE".into()),
    }
}
pub(crate) fn trusted_roots() -> Result<PathBuf, String> {
    let roots = dirs::config_dir()
        .ok_or("PLUGIN_SIGNATURE_ROOT_STORE")?
        .join("CobbleLauncher/plugin-trust/roots");
    seed_official_root(&roots)?;
    Ok(roots)
}
pub(super) fn read_package(path: &Path) -> Result<(PluginManifest, Vec<u8>), PluginError> {
    let roots = trusted_roots().map_err(|_| {
        PluginError::new(
            "PLUGIN_SIGNATURE_ROOT_STORE",
            "Trusted plugin roots are unavailable",
        )
    })?;
    cobble_plugin_host::read_package_with_trusted_roots(path, Some(&roots))
}
#[tauri::command]
pub fn get_pending_plugin_signature() -> Result<cobble_plugin_host::SignatureStatus, String> {
    let package = super::PACKAGE
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "PLUGIN_PREVIEW_UNAVAILABLE")?
        .pending
        .clone()
        .ok_or("PLUGIN_PREVIEW_REQUIRED")?;
    let roots = trusted_roots()?;
    let status = cobble_plugin_host::package_signature_status(&package.path, Some(&roots))
        .map_err(|error| error.code)?;
    if status != package.signature {
        return Err("PLUGIN_REVIEW_MISMATCH".into());
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_root_is_seeded_on_fresh_profile_and_preserves_other_roots() {
        let directory = tempfile::tempdir().unwrap();
        let roots = directory.path().join("roots");
        seed_official_root(&roots).unwrap();
        std::fs::write(roots.join("user-approved.json"), b"user root").unwrap();
        seed_official_root(&roots).unwrap();
        assert_eq!(
            std::fs::read(roots.join(format!("{OFFICIAL_ROOT_ID}.json"))).unwrap(),
            OFFICIAL_ROOT
        );
        assert_eq!(
            std::fs::read(roots.join("user-approved.json")).unwrap(),
            b"user root"
        );
    }
    #[test]
    fn changed_official_root_is_rejected_without_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join(format!("{OFFICIAL_ROOT_ID}.json"));
        std::fs::write(&root, b"changed").unwrap();
        assert!(seed_official_root(directory.path()).is_err());
        assert_eq!(std::fs::read(root).unwrap(), b"changed");
    }

    #[test]
    #[ignore = "requires explicitly selected signed public release distribution"]
    fn public_release_installs_with_fresh_official_root_and_preserves_permissions() {
        let distribution = PathBuf::from(
            std::env::var_os("COBBLE_TEST_PLUGIN_DISTRIBUTION").expect("release distribution"),
        );
        let index: cobble_plugin_host::catalog::CatalogIndex =
            serde_json::from_slice(&std::fs::read(distribution.join("index.json")).unwrap())
                .unwrap();
        assert_eq!(index.plugins.len(), 30);
        let profile = tempfile::tempdir().unwrap();
        let roots = profile.path().join("roots");
        seed_official_root(&roots).unwrap();
        let registry = cobble_plugin_host::PluginRegistry::new(profile.path().join("plugins"))
            .with_trusted_roots(roots.clone());
        for plugin in index.plugins {
            let artifact = plugin.package.unwrap();
            let filename = artifact.url.rsplit('/').next().unwrap();
            let bytes = std::fs::read(distribution.join("assets").join(filename)).unwrap();
            let (staging, signature) =
                crate::catalog_install::extract_bundle(&plugin.manifest, &artifact, &bytes, &roots)
                    .unwrap();
            assert!(matches!(
                signature,
                cobble_plugin_host::SignatureStatus::Verified { .. }
            ));
            let installed = registry.install_directory(staging.path()).unwrap();
            assert_eq!(installed.manifest, plugin.manifest);
            assert_eq!(installed.manifest.permissions, plugin.manifest.permissions);
            registry
                .read_installed_package(&plugin.manifest.id)
                .unwrap();
        }
    }
}
