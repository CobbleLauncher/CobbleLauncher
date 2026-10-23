// SPDX-License-Identifier: MIT
//! Immutable selected-folder snapshots, never a WASI directory or live filesystem grant.
use crate::{PluginError, PluginManifest, REPORT_CONTEXT_API_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub const CONFIG_FOLDER_PROTOCOL: &str = "cobble.config-folder@1";
pub const CONFIG_FOLDER_MEDIA_TYPE: &str = "application/vnd.cobble.config-folder+json";
pub const MAX_CONFIG_FOLDER_FILES: usize = 64;
pub const MAX_CONFIG_FOLDER_FILE_BYTES: usize = 256 * 1024;
pub const MAX_CONFIG_FOLDER_TEXT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_CONFIG_FOLDER_ENVELOPE_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFolderFile {
    pub relative_name: String,
    pub media_type: String,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFolderSnapshot {
    pub schema_version: u32,
    pub files: Vec<ConfigFolderFile>,
    pub skipped: u32,
    pub truncated: bool,
}
pub fn config_folder_media_type(extension: &str) -> Option<&'static str> {
    match extension {
        "json" => Some("application/json"),
        "toml" => Some("application/toml"),
        "properties" => Some("text/x-java-properties"),
        "yaml" | "yml" => Some("application/yaml"),
        "txt" => Some("text/plain"),
        _ => None,
    }
}
fn invalid() -> PluginError {
    PluginError::new(
        "PLUGIN_INVALID_FOLDER_SNAPSHOT",
        "Invalid or unauthorized bounded configuration snapshot",
    )
}
impl ConfigFolderSnapshot {
    pub fn validate(&self, manifest: &PluginManifest) -> Result<(), PluginError> {
        manifest.validate()?;
        if manifest.capabilities != ["report"]
            || manifest.api_version != REPORT_CONTEXT_API_VERSION
            || manifest.report_protocols != [CONFIG_FOLDER_PROTOCOL]
        {
            return Err(invalid());
        }
        self.validate_bounds()?;
        if self.files.iter().any(|file| {
            !manifest.file_extensions.iter().any(|ext| {
                file.relative_name
                    .rsplit_once('.')
                    .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case(ext))
            })
        }) {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn validate_bounds(&self) -> Result<(), PluginError> {
        let mut names = BTreeSet::new();
        let mut total = 0usize;
        if self.schema_version != 1 || self.files.len() > MAX_CONFIG_FOLDER_FILES {
            return Err(invalid());
        }
        for file in &self.files {
            let name = &file.relative_name;
            if name.is_empty()
                || name.len() > 240
                || name
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '\\' | ':'))
                || name
                    .split('/')
                    .any(|part| part.is_empty() || matches!(part, "." | ".."))
                || !names.insert(name.to_lowercase())
                || file.text.len() > MAX_CONFIG_FOLDER_FILE_BYTES
            {
                return Err(invalid());
            }
            let extension = name
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
                .ok_or_else(invalid)?;
            if config_folder_media_type(&extension) != Some(file.media_type.as_str()) {
                return Err(invalid());
            }
            total = total.checked_add(file.text.len()).ok_or_else(invalid)?;
        }
        if total > MAX_CONFIG_FOLDER_TEXT_BYTES
            || serde_json::to_vec(self).map_err(|_| invalid())?.len()
                > MAX_CONFIG_FOLDER_ENVELOPE_BYTES
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> ConfigFolderSnapshot {
        ConfigFolderSnapshot {
            schema_version: 1,
            files: vec![ConfigFolderFile {
                relative_name: "settings.toml".into(),
                media_type: "application/toml".into(),
                text: "enabled=true\n".into(),
            }],
            skipped: 0,
            truncated: false,
        }
    }
    #[test]
    fn config_folder_bounds_reject_paths_duplicates_and_oversized_or_mismatched_inputs() {
        sample().validate_bounds().unwrap();
        for name in [
            "/secret.toml",
            "../secret.toml",
            "C:/secret.toml",
            "dir\\secret.toml",
            "dir//secret.toml",
        ] {
            let mut s = sample();
            s.files[0].relative_name = name.into();
            assert!(s.validate_bounds().is_err());
        }
        let mut s = sample();
        s.files.push(s.files[0].clone());
        assert!(s.validate_bounds().is_err());
        let mut s = sample();
        s.files[0].text = "x".repeat(MAX_CONFIG_FOLDER_FILE_BYTES + 1);
        assert!(s.validate_bounds().is_err());
        let mut s = sample();
        s.files[0].media_type = "text/plain".into();
        assert!(s.validate_bounds().is_err());
    }
    #[test]
    fn config_folder_requires_explicit_protocol_and_permission_not_just_file_read() {
        let mut manifest:PluginManifest=serde_json::from_value(serde_json::json!({"id":"doctor","name":"Doctor","version":"0.1.0","publisher":"Original","api_version":"0.2.0","capabilities":["report"],"file_extensions":["toml"],"component_sha256":"0".repeat(64)})).unwrap();
        let old = manifest.clone();
        assert!(sample().validate(&manifest).is_err());
        manifest.report_protocols = vec![CONFIG_FOLDER_PROTOCOL.into()];
        assert!(manifest.validate().is_err());
        manifest.permissions = Some(manifest.effective_permissions());
        sample().validate(&manifest).unwrap();
        assert!(manifest.requires_permission_approval(&old));
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, crate::PermissionNode::SelectedFolderRead { .. }));
        assert!(manifest.validate().is_err());
    }
}
