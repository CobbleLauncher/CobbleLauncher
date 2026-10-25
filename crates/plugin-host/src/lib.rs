// SPDX-License-Identifier: MIT
//! Local Viewer registry and bounded, killable, isolated component execution.
use cobble_plugin_api::{
    InstalledPlugin, MAX_COMPONENT_BYTES, MAX_FILE_BYTES, PluginError, PluginManifest, ViewerScene,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
mod package_trust;
pub use package_trust::{SignatureStatus, package_signature_status};
pub mod catalog;
mod composer;
mod tool_form;
pub use tool_form::run_tool_form;
mod image_layout;
pub use image_layout::run_image_layout;
pub mod interfaces;
mod process;
pub mod provider;
mod registry_update;
mod report;
pub mod resource_grants;
pub use composer::run_message_composer;
pub use report::check_input as validate_report_input;
pub use report::{run_report, run_report_with_context};
mod report_context;
pub use report_context::ReportHostContext;
mod session;
mod worker_session;
pub use session::PluginSession;

mod frontend_assets;
pub use frontend_assets::{FrontendAssetBytes, read_frontend_asset};
mod package_assets;
pub use package_assets::{MAX_ICON_BYTES, copy_package_assets, validate_icon_png};

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn io_error(error: impl std::fmt::Display) -> PluginError {
    PluginError::new("PLUGIN_IO", error.to_string())
}
fn read_regular(directory: &Path, name: &str, maximum: usize) -> Result<Vec<u8>, PluginError> {
    let path = directory.join(name);
    let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(PluginError::new(
            "PLUGIN_INVALID_PACKAGE",
            "Package files must be regular files",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PACKAGE",
                "Package reparse points are unsupported",
            ));
        }
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(io_error)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > maximum {
        return Err(PluginError::new(
            "PLUGIN_INVALID_PACKAGE",
            "Package file exceeds its size budget",
        ));
    }
    Ok(bytes)
}
fn read_manifest(directory: &Path) -> Result<PluginManifest, PluginError> {
    serde_json::from_slice(&read_regular(directory, "plugin.json", 16384)?).map_err(io_error)
}
/// Integrity-only access to an installed package, including unsupported old ABIs.
/// This never authorizes execution or a new installation.
fn read_package_integrity(directory: &Path) -> Result<(PluginManifest, Vec<u8>), PluginError> {
    let manifest = read_manifest(directory)?;
    let bytes = read_regular(directory, "component.wasm", MAX_COMPONENT_BYTES)?;
    if !bytes.starts_with(b"\0asm\x0d\0\x01\0")
        || digest(&bytes) != manifest.component_sha256.to_ascii_lowercase()
    {
        return Err(PluginError::new(
            "PLUGIN_INVALID_PACKAGE",
            "Expected a raw WebAssembly Component matching the declared SHA-256",
        ));
    }
    package_assets::validate_package_assets(directory, &manifest)?;
    Ok((manifest, bytes))
}
pub fn read_package_icon(
    directory: &Path,
) -> Result<(PluginManifest, Option<Vec<u8>>), PluginError> {
    let manifest = read_manifest(directory)?;
    let icon = package_assets::read_icon(directory, &manifest)?;
    Ok((manifest, icon))
}
pub fn read_package(directory: &Path) -> Result<(PluginManifest, Vec<u8>), PluginError> {
    read_package_with_trusted_roots(directory, None)
}
pub fn read_package_with_trusted_roots(
    directory: &Path,
    roots: Option<&Path>,
) -> Result<(PluginManifest, Vec<u8>), PluginError> {
    let (manifest, bytes) = read_package_integrity(directory)?;
    manifest.validate()?;
    package_trust::verify_source(directory, roots)?;
    Ok((manifest, bytes))
}
type VerifiedSource = (
    PluginManifest,
    Vec<u8>,
    Option<cobble_plugin_trust::VerifiedPackage>,
);
pub struct PluginRegistry {
    root: PathBuf,
    trusted_roots: Option<PathBuf>,
}
impl PluginRegistry {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            trusted_roots: None,
        }
    }
    pub fn with_trusted_roots(mut self, roots: PathBuf) -> Self {
        self.trusted_roots = Some(roots);
        self
    }
    pub fn read_installed_package(
        &self,
        id: &str,
    ) -> Result<(PluginManifest, Vec<u8>), PluginError> {
        let directory = self.directory(id)?;
        let (manifest, bytes) = self.read_installed_integrity(&directory)?;
        if manifest.id != id {
            return Err(PluginError::new(
                "PLUGIN_INVALID_ID",
                "Installed plugin ID differs",
            ));
        }
        manifest.validate()?;
        Ok((manifest, bytes))
    }
    fn read_installed_integrity(
        &self,
        directory: &Path,
    ) -> Result<(PluginManifest, Vec<u8>), PluginError> {
        let (manifest, bytes) = read_package_integrity(directory)?;
        package_trust::verify_installed(
            &self.root,
            directory,
            &manifest,
            self.trusted_roots.as_deref(),
        )?;
        Ok((manifest, bytes))
    }
    fn read_source_package(&self, directory: &Path) -> Result<VerifiedSource, PluginError> {
        let (manifest, bytes) = read_package_integrity(directory)?;
        manifest.validate()?;
        let verified = package_trust::verify_source(directory, self.trusted_roots.as_deref())?;
        Ok((manifest, bytes, verified))
    }
    fn directory(&self, id: &str) -> Result<PathBuf, PluginError> {
        if id.is_empty()
            || id.len() > 80
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(PluginError::new("PLUGIN_INVALID_ID", "Invalid plugin ID"));
        }
        let directory = self.root.join(id);
        if let Ok(metadata) = fs::symlink_metadata(&directory) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(PluginError::new(
                    "PLUGIN_INVALID_PACKAGE",
                    "Plugin directory must be a regular directory",
                ));
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(PluginError::new(
                        "PLUGIN_INVALID_PACKAGE",
                        "Plugin directory reparse points are unsupported",
                    ));
                }
            }
        }
        Ok(directory)
    }
    /// Uses local approval receipts and continued root trust, without online expiry checks.
    pub fn signature_status(&self, id: &str) -> Result<SignatureStatus, PluginError> {
        let _guard = registry_update::lock()?;
        self.recover_updates()?;
        let directory = self.directory(id)?;
        let (manifest, _) = self.read_installed_integrity(&directory)?;
        package_trust::installed_signature_status(
            &self.root,
            &directory,
            &manifest,
            self.trusted_roots.as_deref(),
        )
    }
    pub fn list(&self) -> Result<Vec<InstalledPlugin>, PluginError> {
        let _guard = registry_update::lock()?;
        self.recover_updates()?;
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_error(error)),
        };
        let mut plugins = Vec::new();
        for entry in entries.take(128) {
            let entry = entry.map_err(io_error)?;
            if self.directory(&entry.file_name().to_string_lossy()).is_ok()
                && let Ok(manifest) = read_manifest(&entry.path())
                && manifest.id == entry.file_name().to_string_lossy()
            {
                let enabled = !entry.path().join("disabled").exists();
                let problem = self
                    .read_installed_package(&manifest.id)
                    .err()
                    .map(|error| error.code);
                plugins.push(InstalledPlugin {
                    manifest,
                    enabled,
                    problem,
                });
            }
        }
        let ready: std::collections::HashSet<_> = interfaces::ready_plugins(&plugins)
            .into_iter()
            .map(|p| p.manifest.id.clone())
            .collect();
        for plugin in &mut plugins {
            if plugin.enabled && plugin.problem.is_none() && !ready.contains(&plugin.manifest.id) {
                plugin.problem = Some("PLUGIN_MISSING_INTERFACE".into());
            }
        }
        plugins.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        Ok(plugins)
    }
    /// Caller has displayed identity and the declared read-only permission.
    /// Installation records approval but never compiles or executes the guest.
    pub fn install_directory(&self, source: &Path) -> Result<InstalledPlugin, PluginError> {
        let _guard = registry_update::lock()?;
        let (manifest, bytes, verified) = self.read_source_package(source)?;
        let destination = self.directory(&manifest.id)?;
        fs::create_dir_all(&self.root).map_err(io_error)?;
        if destination.exists() {
            return Err(PluginError::new(
                "PLUGIN_ALREADY_INSTALLED",
                "Uninstall the existing plugin before installing another package",
            ));
        }
        let staging = tempfile::Builder::new()
            .prefix(".install-")
            .tempdir_in(&self.root)
            .map_err(io_error)?;
        fs::write(
            staging.path().join("plugin.json"),
            read_regular(source, "plugin.json", 16384)?,
        )
        .map_err(io_error)?;
        fs::write(staging.path().join("component.wasm"), bytes).map_err(io_error)?;
        copy_package_assets(source, staging.path(), &manifest)?;
        let signed = package_trust::approve(
            &self.root,
            source,
            staging.path(),
            &manifest,
            verified,
            None,
        )?;
        fs::rename(staging.path(), &destination).map_err(io_error)?;
        if signed && let Err(error) = package_trust::mark_verified(&self.root, &manifest.id) {
            fs::remove_dir_all(&destination).map_err(io_error)?;
            return Err(error);
        }
        Ok(InstalledPlugin {
            manifest,
            enabled: true,
            problem: None,
        })
    }
    pub fn icon(&self, id: &str) -> Result<Option<Vec<u8>>, PluginError> {
        let directory = self.directory(id)?;
        let manifest = read_manifest(&directory)?;
        package_assets::read_icon(&directory, &manifest)
    }
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), PluginError> {
        let _guard = registry_update::lock()?;
        let directory = self.directory(id)?;
        self.read_installed_package(id)?;
        if enabled {
            match fs::remove_file(directory.join("disabled")) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(io_error(e)),
            }
        } else {
            fs::write(directory.join("disabled"), b"disabled").map_err(io_error)?;
        }
        Ok(())
    }
    pub fn uninstall(&self, id: &str) -> Result<(), PluginError> {
        let _guard = registry_update::lock()?;
        let directory = self.directory(id)?;
        // Remove only the canonical, exact child of this registry.
        let canonical_root = fs::canonicalize(&self.root).map_err(io_error)?;
        let canonical = fs::canonicalize(&directory).map_err(io_error)?;
        if canonical.parent() != Some(canonical_root.as_path()) {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PACKAGE",
                "Plugin directory escaped registry root",
            ));
        }
        fs::remove_dir_all(canonical).map_err(io_error)?;
        package_trust::remove_receipts(&self.root, id)
    }
    /// Revalidate the identity of an already loaded immutable Viewer session.
    /// Its component bytes were verified at open; chunk reads need not hash every package again.
    pub fn validate_viewer_session(&self, expected: &PluginManifest) -> Result<(), PluginError> {
        if !expected.supports_interface(&cobble_plugin_api::InterfaceRequirement::viewer()) {
            return Err(PluginError::new(
                "PLUGIN_UNSUPPORTED_API",
                "Session does not provide a compatible Viewer",
            ));
        }
        let guard = registry_update::lock()?;
        let directory = self.directory(&expected.id)?;
        if directory.join("disabled").exists() || read_manifest(&directory)? != *expected {
            return Err(PluginError::new(
                "PLUGIN_DISABLED",
                "Viewer is disabled or changed",
            ));
        }
        drop(guard);
        if !expected.interfaces.requires.is_empty() {
            self.resolve_enabled(&expected.id)?;
        }
        Ok(())
    }
    pub fn open_viewer(
        &self,
        host_executable: &Path,
        id: &str,
        data: &[u8],
        cancel: Arc<AtomicBool>,
    ) -> Result<PluginSession, PluginError> {
        if data.len() > MAX_FILE_BYTES {
            return Err(PluginError::new(
                "VIEWER_FILE_TOO_LARGE",
                "Viewer input exceeds 8 MiB",
            ));
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(PluginError::new("CANCEL_REQUESTED", "Viewer cancelled"));
        }
        self.resolve_enabled(id)?;
        let directory = self.directory(id)?;
        if directory.join("disabled").exists() {
            return Err(PluginError::new(
                "PLUGIN_DISABLED",
                "Enable this plugin before opening a file",
            ));
        }
        let (manifest, component) = self.read_installed_package(id)?;
        if manifest.api_version != cobble_plugin_api::API_VERSION
            || manifest.capabilities != ["viewer"]
        {
            return Err(PluginError::new(
                "PLUGIN_UNSUPPORTED",
                "This plugin does not provide a Viewer",
            ));
        }
        let result = PluginSession::open(host_executable, &component, data, cancel);
        if result.as_ref().is_err_and(|error| {
            matches!(
                error.code.as_str(),
                "PLUGIN_CRASH" | "PLUGIN_TIMEOUT" | "PLUGIN_TRAP"
            )
        }) {
            let counter = directory.join("failures");
            let failures = fs::read_to_string(&counter)
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0)
                .saturating_add(1);
            let _ = fs::write(counter, failures.to_string());
            if failures >= 3 {
                let _ = self.set_enabled(id, false);
            }
        } else if result.is_ok() {
            let _ = fs::remove_file(directory.join("failures"));
        }
        result
    }
    pub fn inspect(
        &self,
        host_executable: &Path,
        id: &str,
        data: &[u8],
        cancel: Arc<AtomicBool>,
    ) -> Result<ViewerScene, PluginError> {
        Ok(self
            .open_viewer(host_executable, id, data, cancel)?
            .summary()
            .clone())
    }
}

/// Component and input are passed through anonymous pipes, never guest paths.
/// Watchdog includes native compilation and terminates/reaps this owned child.
pub fn run_process(
    host_executable: &Path,
    component: &[u8],
    data: &[u8],
    cancel: Arc<AtomicBool>,
) -> Result<ViewerScene, PluginError> {
    Ok(
        PluginSession::open(host_executable, component, data, cancel)?
            .summary()
            .clone(),
    )
}
