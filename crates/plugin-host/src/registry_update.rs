// SPDX-License-Identifier: MIT
use crate::{InstalledPlugin, PluginError, PluginRegistry, fs, io_error};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

static REGISTRY_MUTATION: Mutex<()> = Mutex::new(());
pub(crate) fn lock() -> Result<MutexGuard<'static, ()>, PluginError> {
    REGISTRY_MUTATION.lock().map_err(io_error)
}

impl PluginRegistry {
    /// Updates with unchanged or reduced authority only. Callers must stop sessions.
    /// Expanded authority requires update_directory_approved with the exact review.
    pub fn update_directory(&self, source: &Path) -> Result<InstalledPlugin, PluginError> {
        self.update_directory_reviewed(source, None)
    }
    /// Explicit approval binds the entire replacement manifest, including scopes.
    pub fn update_directory_approved(
        &self,
        source: &Path,
        reviewed: &cobble_plugin_api::PluginManifest,
    ) -> Result<InstalledPlugin, PluginError> {
        self.update_directory_reviewed(source, Some(reviewed))
    }
    fn update_directory_reviewed(
        &self,
        source: &Path,
        reviewed: Option<&cobble_plugin_api::PluginManifest>,
    ) -> Result<InstalledPlugin, PluginError> {
        let _guard = lock()?;
        let (manifest, bytes, verified) = self.read_source_package(source)?;
        self.recover_updates()?;
        let destination = self.directory(&manifest.id)?;
        let (previous, _) = self.read_installed_integrity(&destination)?;
        if previous.id != manifest.id {
            return Err(PluginError::new(
                "PLUGIN_INVALID_ID",
                "Replacement plugin ID differs",
            ));
        }
        if reviewed.is_some_and(|reviewed| reviewed != &manifest) {
            return Err(PluginError::new(
                "PLUGIN_REVIEW_MISMATCH",
                "Reviewed plugin manifest differs from replacement",
            ));
        }
        if reviewed.is_none()
            && (manifest.publisher != previous.publisher
                || manifest.requires_permission_approval(&previous))
        {
            return Err(PluginError::new(
                "PLUGIN_PERMISSION_APPROVAL_REQUIRED",
                "Review expanded plugin permissions before updating",
            ));
        }
        let enabled = !destination.join("disabled").exists();
        let staging = tempfile::Builder::new()
            .prefix(".update-")
            .tempdir_in(&self.root)
            .map_err(io_error)?;
        fs::write(
            staging.path().join("plugin.json"),
            crate::read_regular(source, "plugin.json", 16384)?,
        )
        .map_err(io_error)?;
        fs::write(staging.path().join("component.wasm"), bytes).map_err(io_error)?;
        crate::package_assets::copy_package_assets(source, staging.path(), &manifest)?;
        let signed = crate::package_trust::approve(
            &self.root,
            source,
            staging.path(),
            &manifest,
            verified,
            Some(&destination),
        )?;
        if !enabled {
            fs::write(staging.path().join("disabled"), b"disabled").map_err(io_error)?;
        }
        let backup = self.root.join(format!(".previous-{}", manifest.id));
        fs::rename(&destination, &backup).map_err(io_error)?;
        if let Err(error) = fs::rename(staging.path(), &destination) {
            fs::rename(&backup, &destination).map_err(io_error)?;
            return Err(io_error(error));
        }
        if signed && let Err(error) = crate::package_trust::mark_verified(&self.root, &manifest.id)
        {
            fs::rename(&destination, staging.path()).map_err(io_error)?;
            fs::rename(&backup, &destination).map_err(io_error)?;
            return Err(error);
        }
        // A failed cleanup is retried on the next registry listing, without losing the new package.
        let _ = fs::remove_dir_all(&backup);
        Ok(InstalledPlugin {
            manifest,
            enabled,
            problem: None,
        })
    }

    pub(crate) fn recover_updates(&self) -> Result<(), PluginError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(error)),
        };
        for entry in entries {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_prefix(".previous-"))
            else {
                continue;
            };
            let destination = self.directory(id)?;
            let backup = entry.path();
            let (manifest, _) = self.read_installed_integrity(&backup)?;
            if manifest.id != id {
                return Err(PluginError::new(
                    "PLUGIN_INVALID_PACKAGE",
                    "Update backup ID differs",
                ));
            }
            if destination.exists() {
                let (current, _) = self.read_installed_integrity(&destination)?;
                if current.id != id {
                    return Err(PluginError::new(
                        "PLUGIN_INVALID_PACKAGE",
                        "Update destination ID differs",
                    ));
                }
                fs::remove_dir_all(backup).map_err(io_error)?;
            } else {
                fs::rename(backup, destination).map_err(io_error)?;
            }
        }
        Ok(())
    }
}
