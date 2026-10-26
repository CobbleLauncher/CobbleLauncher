// SPDX-License-Identifier: MIT
//! Versioned Viewer/report data and declarative plugin package validation.
use serde::{Deserialize, Serialize};

pub mod config_folder;
pub mod contributions;
pub mod frontend;
pub use config_folder::*;
pub use frontend::*;
pub mod image_layout;
pub use image_layout::*;
pub mod inspection;
pub use inspection::*;
pub mod interfaces;
pub mod mesh_events;
pub mod message_composer;
pub mod permissions;
pub use interfaces::*;
pub use mesh_events::*;
pub use message_composer::*;
pub use permissions::*;
pub mod report;
pub mod settings_form;
pub mod tool_form;
pub use contributions::*;
pub use report::Report;
pub use settings_form::*;
pub use tool_form::*;

pub const API_VERSION: &str = "1.0.0";
pub const REPORT_API_VERSION: &str = "0.1.0";
pub const REPORT_CONTEXT_API_VERSION: &str = "0.2.0";
pub const PROVIDER_API_VERSION: &str = "0.2.0";
pub const MRPACK_PAGE_PROTOCOL: &str = "cobble.mrpack-page@1";
pub const PACK_METADATA_PROTOCOL: &str = "cobble.pack-metadata@1";
pub const LANG_PACKAGE_PROTOCOL: &str = "cobble.lang-package@1";

#[cfg(test)]
#[test]
fn pack_metadata_zip_requires_its_explicit_protocol() {
    let mut manifest: PluginManifest = serde_json::from_value(serde_json::json!({
        "id":"metadata", "name":"Metadata", "version":"1.0.0", "api_version":"0.2.0",
        "publisher":"Fixture", "capabilities":["report"], "file_extensions":["zip", "mcmeta"],
        "component_sha256":"a".repeat(64)
    }))
    .unwrap();
    assert!(manifest.validate().is_err());
    manifest.report_protocols = vec![PACK_METADATA_PROTOCOL.into()];
    manifest.validate().unwrap();
    manifest.file_extensions.push("log".into());
    assert!(manifest.validate().is_err());
}
pub const MAX_COMPONENT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_REPLY_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_BLOCKS: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ManifestDescription {
    Plain(String),
    Localized(LocalizedText),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginIcon {
    pub file: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_form: Option<ToolForm>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontend: Option<FrontendManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<PermissionNode>>,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<ManifestDescription>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<PluginIcon>,
    pub version: String,
    pub api_version: String,
    pub publisher: String,
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub composer_permissions: Vec<ComposerPermission>,
    #[serde(default, skip_serializing_if = "Interfaces::is_empty")]
    pub interfaces: Interfaces,
    pub file_extensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub report_protocols: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_origins: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contributions: Option<Contributions>,
    pub component_sha256: String,
}
impl PluginManifest {
    /// A local frontend calculator needs no implicit file or storage grant.
    pub fn is_local_frontend_report(&self) -> bool {
        self.api_version == REPORT_CONTEXT_API_VERSION
            && self.capabilities == ["report"]
            && self.frontend.is_some()
            && self.file_extensions.is_empty()
            && self.report_protocols.is_empty()
            && self.permissions.as_ref().is_some_and(|nodes| {
                nodes.contains(&PermissionNode::UiWebview {})
                    && nodes.iter().all(|node| {
                        matches!(
                            node,
                            PermissionNode::UiWebview {}
                                | PermissionNode::ClipboardWrite {}
                                | PermissionNode::GeneratedBlueprintSave {}
                        )
                    })
            })
    }
    /// Closed host workflows do not imply a selected-file read grant.
    pub fn is_host_workflow_report(&self) -> bool {
        self.api_version == REPORT_CONTEXT_API_VERSION
            && self.capabilities == ["report"]
            && self.frontend.is_some()
            && self.file_extensions.is_empty()
            && self.report_protocols.is_empty()
            && self
                .permissions
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|node| {
                    matches!(
                        node,
                        PermissionNode::PlayManifestExport {}
                            | PermissionNode::FileInboxImportRequest {}
                    )
                })
    }
    /// Plugin-owned JSON data does not require a selected-file association.
    pub fn is_private_data_report(&self) -> bool {
        self.api_version == REPORT_CONTEXT_API_VERSION
            && self.capabilities == ["report"]
            && self.frontend.is_some()
            && self.file_extensions.is_empty()
            && self.report_protocols.is_empty()
            && self
                .permissions
                .as_deref()
                .unwrap_or_default()
                .contains(&PermissionNode::PrivateStorage {})
    }
    /// Host JSON payloads are independent of user-selected file associations.
    pub fn is_inspection_report(&self) -> bool {
        self.api_version == REPORT_CONTEXT_API_VERSION
            && self.capabilities == ["report"]
            && self.frontend.is_some()
            && (self.file_extensions.is_empty() || self.file_extensions == ["json"])
            && self.report_protocols.is_empty()
            && self.permissions.as_deref().unwrap_or_default().iter().any(|node| matches!(node, PermissionNode::GameInspectionRead { scopes } if !scopes.is_empty()))
    }
    pub fn validate_metadata(&self) -> Result<(), PluginError> {
        if let Some(frontend) = &self.frontend {
            frontend.validate_metadata()?;
        }
        self.interfaces.validate()?;
        if self.id.is_empty()
            || self.id.len() > 80
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PACKAGE",
                "Invalid plugin ID",
            ));
        }
        if self.name.is_empty()
            || self.name.len() > 120
            || self.publisher.is_empty()
            || self.publisher.len() > 120
            || self.version.is_empty()
            || self.version.len() > 40
            || self.component_sha256.len() != 64
            || !self.component_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PACKAGE",
                "Invalid plugin metadata or component digest",
            ));
        }
        let invalid = || {
            PluginError::new(
                "PLUGIN_INVALID_PACKAGE",
                "Invalid description, tags or icon metadata",
            )
        };
        if self.description.as_ref().is_some_and(|description| {
            let texts = match description {
                ManifestDescription::Plain(s) => vec![s.as_str()],
                ManifestDescription::Localized(s) => std::iter::once(s.en.as_str())
                    .chain(s.zh_cn.as_deref())
                    .collect(),
            };
            texts.iter().any(|s| {
                s.trim().is_empty()
                    || s.len() > 2048
                    || s.chars().count() > 512
                    || s.chars().any(char::is_control)
            })
        }) {
            return Err(invalid());
        }
        let mut tags = std::collections::HashSet::new();
        if self.tags.len() > 8
            || self.tags.iter().any(|s| {
                s.trim() != s
                    || s.is_empty()
                    || s.len() > 96
                    || s.chars().count() > 24
                    || s.chars().any(char::is_control)
                    || !tags.insert(s.to_lowercase())
            })
        {
            return Err(invalid());
        }
        if self.icon.as_ref().is_some_and(|icon| {
            icon.file != "icon.png"
                || icon.sha256.len() != 64
                || !icon.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        }) {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), PluginError> {
        if let Some(frontend) = &self.frontend {
            frontend.validate()?;
        }
        self.validate_interface_abi()?;
        self.validate_composer_permissions()?;
        self.validate_tool_form()?;
        self.validate_permissions()?;
        self.validate_metadata()?;
        let viewer = self.api_version == API_VERSION
            && self.capabilities == ["viewer"]
            && !self.file_extensions.is_empty()
            && self.file_extensions.len() <= 2
            && self
                .file_extensions
                .iter()
                .all(|extension| matches!(extension.as_str(), "schem" | "litematic"))
            && self
                .file_extensions
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.file_extensions.len();
        let report = matches!(
            self.api_version.as_str(),
            REPORT_API_VERSION | REPORT_CONTEXT_API_VERSION
        ) && self.capabilities == ["report"]
            && (!self.file_extensions.is_empty()
                || self.is_inspection_report()
                || self.is_private_data_report()
                || self.is_host_workflow_report()
                || self.is_mesh_events_frontend()
                || self.is_local_frontend_report())
            && self.file_extensions.len() <= 8
            && self.file_extensions.iter().all(|extension| {
                matches!(
                    extension.as_str(),
                    "log"
                        | "txt"
                        | "json"
                        | "mcmeta"
                        | "zip"
                        | "mrpack"
                        | "toml"
                        | "properties"
                        | "yaml"
                        | "yml"
                )
            })
            && self
                .file_extensions
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.file_extensions.len();
        let protocols_valid = (self.report_protocols.is_empty()
            || (report
                && self.api_version == REPORT_CONTEXT_API_VERSION
                && self.report_protocols == [CONFIG_FOLDER_PROTOCOL]
                && self
                    .file_extensions
                    .iter()
                    .all(|extension| config_folder_media_type(extension).is_some()))
            || (report && self.report_protocols == [MRPACK_PAGE_PROTOCOL])
            || (report
                && self.report_protocols == [PACK_METADATA_PROTOCOL]
                && self
                    .file_extensions
                    .iter()
                    .all(|extension| matches!(extension.as_str(), "zip" | "mcmeta" | "json")))
            || (report
                && self.api_version == REPORT_CONTEXT_API_VERSION
                && self.report_protocols == [LANG_PACKAGE_PROTOCOL]
                && self
                    .file_extensions
                    .iter()
                    .all(|extension| matches!(extension.as_str(), "json" | "zip"))))
            && (!self
                .file_extensions
                .iter()
                .any(|extension| extension == "zip")
                || self.report_protocols == [PACK_METADATA_PROTOCOL]
                || self.report_protocols == [LANG_PACKAGE_PROTOCOL]);
        let provider = self.api_version == PROVIDER_API_VERSION
            && self.capabilities == ["blueprint-provider"]
            && self.file_extensions.is_empty()
            && self.report_protocols.is_empty()
            && !self.provider_origins.is_empty()
            && self.provider_origins.len() <= 4
            && self
                .provider_origins
                .iter()
                .all(|origin| valid_provider_origin(origin));
        let composer = self.api_version == MESSAGE_COMPOSER_API_VERSION
            && self.capabilities == ["message-composer"]
            && self.file_extensions.is_empty()
            && self.report_protocols.is_empty();
        let image_layout = self.api_version == IMAGE_LAYOUT_API_VERSION
            && self.capabilities == ["image-layout"]
            && self.file_extensions.is_empty()
            && self.report_protocols.is_empty();
        let mut origins = std::collections::HashSet::new();
        let origins_valid = if provider {
            self.provider_origins
                .iter()
                .all(|origin| origins.insert(origin.strip_suffix(":443").unwrap_or(origin)))
        } else {
            self.provider_origins.is_empty()
        };
        let tool_form = self.api_version == TOOL_FORM_API_VERSION
            && self.capabilities == ["tool-form"]
            && self.file_extensions.is_empty()
            && self.report_protocols.is_empty();
        if (!viewer && !report && !provider && !composer && !image_layout && !tool_form)
            || !protocols_valid
            || !origins_valid
        {
            return Err(PluginError::new(
                "PLUGIN_UNSUPPORTED_API",
                "Unsupported plugin API, capabilities or file extensions",
            ));
        }
        if let Some(contributions) = &self.contributions {
            contributions.validate(self)?;
        }
        Ok(())
    }
}
/// Structural validation only. Native networking must also enforce grants, DNS,
/// public address restrictions and origin-bound redirect credentials.
pub fn valid_provider_origin(origin: &str) -> bool {
    let Some(authority) = origin.strip_prefix("https://") else {
        return false;
    };
    let host = authority;
    host.len() <= 253
        && host.contains('.')
        && !host.ends_with(".localhost")
        && !host
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub manifest: PluginManifest,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialCount {
    pub state: String,
    pub count: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneBlock {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub palette_index: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneChunk {
    pub blocks: Vec<SceneBlock>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewerScene {
    pub format_version: u32,
    pub width: u32,
    pub height: u32,
    pub length: u32,
    pub palette: Vec<String>,
    pub materials: Vec<MaterialCount>,
    pub chunk_count: u32,
}
impl ViewerScene {
    pub fn validate(&self) -> Result<(), PluginError> {
        let volume = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|n| n.checked_mul(self.length as usize));
        if !matches!(self.format_version, 1..=7)
            || !volume.is_some_and(|n| n > 0 && n <= MAX_BLOCKS)
            || self.palette.is_empty()
            || self.palette.len() > 4096
            || self.palette.iter().any(|s| s.len() > 512)
            || self.materials.len() > self.palette.len()
            || self.chunk_count as usize > volume.unwrap_or(0).div_ceil(1024)
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_REPLY",
                "Viewer output exceeds supported scene limits",
            ));
        }
        let palette: std::collections::HashSet<_> =
            self.palette.iter().map(String::as_str).collect();
        let mut states = std::collections::HashSet::new();
        let mut total = 0u64;
        for material in &self.materials {
            total += u64::from(material.count);
            if material.count == 0
                || !palette.contains(material.state.as_str())
                || !states.insert(material.state.as_str())
                || total > volume.unwrap_or(0) as u64
            {
                return Err(PluginError::new(
                    "PLUGIN_INVALID_REPLY",
                    "Viewer materials do not match the scene palette or volume",
                ));
            }
        }
        Ok(())
    }
    pub fn validate_chunk(&self, chunk: &SceneChunk) -> Result<(), PluginError> {
        let mut positions = std::collections::HashSet::new();
        if chunk.blocks.len() > 1024 {
            return Err(PluginError::new(
                "PLUGIN_INVALID_REPLY",
                "Viewer chunk is too large",
            ));
        }
        for b in &chunk.blocks {
            if b.x >= self.width
                || b.y >= self.height
                || b.z >= self.length
                || b.palette_index as usize >= self.palette.len()
                || !positions.insert((b.x, b.y, b.z))
            {
                return Err(PluginError::new(
                    "PLUGIN_INVALID_REPLY",
                    "Viewer contains invalid block coordinates or palette indexes",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginError {
    pub code: String,
    pub message: String,
}
impl PluginError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for PluginError {}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scene_summary_binds_chunks_and_materials_to_actual_volume() {
        let scene = ViewerScene {
            format_version: 2,
            width: 2,
            height: 1,
            length: 1,
            palette: vec!["minecraft:stone".into(), "minecraft:air".into()],
            materials: vec![MaterialCount {
                state: "minecraft:stone".into(),
                count: 2,
            }],
            chunk_count: 1,
        };
        assert!(scene.validate().is_ok());
        let mut litematic = scene.clone();
        for version in 1..=7 {
            litematic.format_version = version;
            assert!(litematic.validate().is_ok());
        }
        litematic.format_version = 8;
        assert!(litematic.validate().is_err());
        let mut invalid = scene.clone();
        invalid.chunk_count = 2;
        assert!(invalid.validate().is_err());
        let mut invalid = scene.clone();
        invalid.materials[0].state = "minecraft:dirt".into();
        assert!(invalid.validate().is_err());
        let mut invalid = scene.clone();
        invalid.materials[0].count = 3;
        assert!(invalid.validate().is_err());
        let mut invalid = scene.clone();
        invalid.materials[0].count = 0;
        assert!(invalid.validate().is_err());
        let mut invalid = scene;
        invalid.materials[0].count = 1;
        invalid.materials.push(invalid.materials[0].clone());
        assert!(invalid.validate().is_err());
    }
    #[test]
    fn manifest_refuses_paths_and_unimplemented_permissions() {
        let mut manifest = PluginManifest {
            frontend: None,
            permissions: None,
            description: None,
            tags: Vec::new(),
            icon: None,
            id: "schematic-viewer".into(),
            name: "Schematic Viewer".into(),
            version: "0.1.0".into(),
            api_version: API_VERSION.into(),
            publisher: "CobbleLauncher".into(),
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
        assert!(manifest.validate().is_ok());
        manifest.file_extensions.push("litematic".into());
        assert!(manifest.validate().is_ok());
        manifest.file_extensions.push("litematic".into());
        assert!(manifest.validate().is_err());
        manifest.file_extensions.pop();
        manifest.id = "../outside".into();
        assert!(manifest.validate().is_err());
        manifest.id = "safe".into();
        manifest.capabilities.push("filesystem".into());
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn report_manifest_dispatch_preserves_viewer_scope() {
        let manifest = PluginManifest {
            frontend: None,
            permissions: None,
            description: None,
            tags: Vec::new(),
            icon: None,
            id: "crash-doctor".into(),
            name: "Crash Doctor".into(),
            version: "0.1.0".into(),
            api_version: REPORT_API_VERSION.into(),
            publisher: "CobbleLauncher".into(),
            capabilities: vec!["report".into()],
            tool_form: None,
            composer_permissions: Default::default(),
            interfaces: Default::default(),
            file_extensions: vec!["log".into(), "txt".into(), "json".into()],
            report_protocols: vec![],
            contributions: None,
            provider_origins: vec![],
            component_sha256: "a".repeat(64),
        };
        assert!(manifest.validate().is_ok());
        for extensions in [vec![], vec!["schem"], vec!["../log"], vec!["log", "log"]] {
            let mut invalid = manifest.clone();
            invalid.file_extensions = extensions.into_iter().map(str::to_owned).collect();
            assert!(invalid.validate().is_err());
        }
        for capabilities in [
            vec!["viewer"],
            vec!["report", "network"],
            vec!["report", "filesystem"],
        ] {
            let mut invalid = manifest.clone();
            invalid.capabilities = capabilities.into_iter().map(str::to_owned).collect();
            assert!(invalid.validate().is_err());
        }
        let mut invalid = manifest;
        invalid.api_version = API_VERSION.into();
        assert!(invalid.validate().is_err());
    }
    #[test]
    fn report_protocol_optin_defaults_and_rejects_other_capabilities() {
        let legacy = serde_json::json!({"id":"report","name":"Report","version":"0.1.0",
            "api_version":REPORT_API_VERSION,"publisher":"Fixture","capabilities":["report"],
            "file_extensions":["mrpack"],"component_sha256":"a".repeat(64)});
        let mut manifest: PluginManifest = serde_json::from_value(legacy).unwrap();
        assert!(manifest.report_protocols.is_empty());
        assert!(
            serde_json::to_value(&manifest)
                .unwrap()
                .get("report_protocols")
                .is_none()
        );
        manifest.report_protocols = vec![MRPACK_PAGE_PROTOCOL.into()];
        assert!(manifest.validate().is_ok());
        manifest.report_protocols.push(MRPACK_PAGE_PROTOCOL.into());
        assert!(manifest.validate().is_err());
        manifest.report_protocols = vec!["unknown@1".into()];
        assert!(manifest.validate().is_err());
        manifest.report_protocols = vec![MRPACK_PAGE_PROTOCOL.into()];
        manifest.capabilities = vec!["viewer".into()];
        manifest.api_version = API_VERSION.into();
        manifest.file_extensions = vec!["schem".into()];
        assert!(manifest.validate().is_err());
    }
}
#[test]
fn provider_manifests_separate_network_origins_from_file_workflows() {
    let mut manifest:PluginManifest = serde_json::from_value(serde_json::json!({"id":"mcs-provider","name":"MCS","version":"0.1.0","api_version":PROVIDER_API_VERSION,"publisher":"Fixture","capabilities":["blueprint-provider"],"file_extensions":[],"provider_origins":["https://www.mcschematic.top"],"component_sha256":"a".repeat(64)})).unwrap();
    manifest.validate().unwrap();
    manifest.api_version = "0.1.0".into();
    assert!(
        manifest.validate().is_err(),
        "legacy Provider ABI must be unsupported"
    );
    manifest.api_version = PROVIDER_API_VERSION.into();
    for origin in [
        "http://www.mcschematic.top",
        "https://user@www.mcschematic.top",
        "https://www.mcschematic.top/path",
        "https://www.mcschematic.top?token=x",
        "https://127.0.0.1",
        "https://www.mcschematic.top:444",
        "https://app.localhost",
    ] {
        manifest.provider_origins = vec![origin.into()];
        assert!(manifest.validate().is_err(), "{origin}");
    }
    manifest.provider_origins = vec![
        "https://www.mcschematic.top".into(),
        "https://www.mcschematic.top:443".into(),
    ];
    assert!(manifest.validate().is_err());
    manifest.provider_origins = vec!["https://www.mcschematic.top".into()];
    manifest.file_extensions = vec!["json".into()];
    assert!(manifest.validate().is_err());
    manifest.capabilities = vec!["report".into()];
    assert!(manifest.validate().is_err());
    manifest.provider_origins.clear();
    manifest.validate().unwrap();
}
#[cfg(test)]
mod metadata_tests {
    use super::*;
    #[test]
    fn legacy_metadata_defaults_and_localized_descriptions_are_bounded() {
        let mut manifest:PluginManifest=serde_json::from_value(serde_json::json!({"id":"metadata-fixture","name":"Metadata","version":"1.0.0","api_version":"0.1.0","publisher":"Original test","capabilities":["report"],"file_extensions":["txt"],"component_sha256":"a".repeat(64)})).unwrap();
        assert!(manifest.description.is_none());
        assert!(manifest.tags.is_empty());
        assert!(manifest.icon.is_none());
        manifest.validate().unwrap();
        manifest.description = Some(ManifestDescription::Localized(LocalizedText {
            en: "Description".into(),
            zh_cn: Some("说明".into()),
        }));
        manifest.tags = vec!["worlds".into()];
        manifest.validate().unwrap();
        manifest.tags.push("WORLDS".into());
        assert!(manifest.validate().is_err());
        manifest.tags.clear();
        manifest.description = Some(ManifestDescription::Plain("x".repeat(513)));
        assert!(manifest.validate().is_err());
        manifest.description = None;
        manifest.icon = Some(PluginIcon {
            file: "https://example.invalid/icon.png".into(),
            sha256: "a".repeat(64),
        });
        assert!(manifest.validate().is_err());
    }
}
