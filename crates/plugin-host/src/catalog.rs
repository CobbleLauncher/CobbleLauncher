// SPDX-License-Identifier: MIT
//! User-selected static catalogs. Parsing is local; callers own network policy.
mod store;
use cobble_plugin_api::{PluginError, PluginManifest};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
pub use store::{CatalogSnapshot, CatalogSource, CatalogStore};

pub const MAX_CATALOG_BYTES: usize = 1024 * 1024;
pub const CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogIndex {
    pub schema_version: u32,
    pub name: String,
    pub plugins: Vec<CatalogPlugin>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogPlugin {
    pub manifest: PluginManifest,
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub component_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<CatalogPackage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_png_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_url: Option<String>,
}
/// Complete signed ZIP package. The digest binds transport bytes; TUF binds content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogPackage {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}
pub const MAX_CATALOG_PACKAGE_BYTES: u64 = 64 * 1024 * 1024;
impl CatalogPlugin {
    /// Listing compatibility never grants install permission or skips package checks.
    pub fn icon_bytes(&self) -> Result<Option<Vec<u8>>, PluginError> {
        use base64::Engine;
        let Some(encoded) = &self.icon_png_base64 else {
            return Ok(None);
        };
        if encoded.len() > 43692 {
            return Err(invalid("Catalog icon exceeds 32 KiB"));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| invalid("Invalid catalog icon base64"))?;
        if bytes.len() > 32768 {
            return Err(invalid("Catalog icon exceeds 32 KiB"));
        }
        let icon = self
            .manifest
            .icon
            .as_ref()
            .ok_or_else(|| invalid("Catalog icon requires manifest icon"))?;
        crate::validate_icon_png(&bytes, &icon.sha256)?;
        Ok(Some(bytes))
    }
    pub fn supported(&self) -> bool {
        self.manifest.validate().is_ok()
            && (self.package.is_some()
                || self.manifest.icon.is_none()
                || self.icon_png_base64.is_some())
    }
}

fn invalid(message: impl Into<String>) -> PluginError {
    PluginError::new("PLUGIN_INVALID_CATALOG", message)
}
pub(super) fn validate_url(value: &str) -> Result<(), PluginError> {
    if value.len() > 2048
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
        || !value.starts_with("https://")
    {
        return Err(invalid(
            "Catalog URLs must be HTTPS URLs of at most 2048 bytes",
        ));
    }
    let url = url::Url::parse(value).map_err(|_| invalid("Invalid catalog HTTPS URL"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(
            "Catalog URLs must use HTTPS without credentials or fragments",
        ));
    }
    Ok(())
}
pub fn parse_index(raw: &[u8]) -> Result<CatalogIndex, PluginError> {
    if raw.len() > MAX_CATALOG_BYTES {
        return Err(invalid("Catalog exceeds 1 MiB"));
    }
    let index: CatalogIndex = serde_json::from_slice(raw)
        .map_err(|error| invalid(format!("Invalid catalog JSON: {error}")))?;
    if index.schema_version != CATALOG_SCHEMA_VERSION {
        return Err(PluginError::new(
            "PLUGIN_UNSUPPORTED_CATALOG",
            "Only catalog schema version 1 is supported",
        ));
    }
    if index.name.trim().is_empty() || index.name.len() > 120 || index.plugins.len() > 512 {
        return Err(invalid(
            "Catalog requires a name up to 120 bytes and at most 512 plugins",
        ));
    }
    let mut ids = HashSet::new();
    for plugin in &index.plugins {
        plugin.icon_bytes()?;
        // Catalogs may announce future capabilities. Check identity/metadata/hash
        // independently of whether this launcher can install that API yet.
        plugin.manifest.validate_metadata()?;
        if plugin.manifest.api_version.is_empty()
            || plugin.manifest.api_version.len() > 40
            || plugin.manifest.capabilities.is_empty()
            || plugin.manifest.capabilities.len() > 32
            || plugin.manifest.file_extensions.len() > 32
            || plugin
                .manifest
                .capabilities
                .iter()
                .chain(&plugin.manifest.file_extensions)
                .any(|value| value.is_empty() || value.len() > 80)
        {
            return Err(invalid(
                "Invalid catalog API, capability or file extension metadata",
            ));
        }
        if !ids.insert(&plugin.manifest.id) {
            return Err(invalid(format!(
                "Duplicate plugin ID: {}",
                plugin.manifest.id
            )));
        }
        if plugin.description.len() > 2000 {
            return Err(invalid("Plugin description exceeds 2000 bytes"));
        }
        if let Some(package) = &plugin.package {
            validate_url(&package.url)?;
            if !plugin.component_url.is_empty()
                || package.size == 0
                || package.size > MAX_CATALOG_PACKAGE_BYTES
                || package.sha256.len() != 64
                || !package.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(invalid("Invalid or ambiguous signed package descriptor"));
            }
        } else {
            validate_url(&plugin.component_url)?;
        }
        if let Some(url) = &plugin.project_url {
            validate_url(url)?;
        }
    }
    Ok(index)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    pub fn fixture() -> CatalogIndex {
        CatalogIndex {
            schema_version: 1,
            name: "Synthetic catalog".into(),
            plugins: vec![CatalogPlugin {
                package: None,
                icon_png_base64: None,
                manifest: PluginManifest {
                    frontend: None,
                    permissions: None,
                    description: None,
                    tags: Vec::new(),
                    icon: None,
                    id: "synthetic-viewer".into(),
                    name: "Synthetic Viewer".into(),
                    version: "0.1.0".into(),
                    api_version: cobble_plugin_api::API_VERSION.into(),
                    publisher: "Original test fixture".into(),
                    capabilities: vec!["viewer".into()],
                    composer_permissions: Default::default(),
                    interfaces: Default::default(),
                    file_extensions: vec!["schem".into()],
                    report_protocols: vec![],
                    contributions: None,
                    provider_origins: vec![],
                    component_sha256: "a".repeat(64),
                },
                description: "Read-only synthetic fixture".into(),
                component_url: "https://example.invalid/viewer.wasm".into(),
                project_url: Some("https://example.invalid/project".into()),
            }],
        }
    }
    #[test]
    fn signed_bundle_descriptor_is_bounded_unambiguous_and_legacy_optional() {
        let mut index = fixture();
        let plugin = &mut index.plugins[0];
        plugin.component_url.clear();
        plugin.package = Some(CatalogPackage {
            url: "https://example.invalid/package.zip".into(),
            sha256: "a".repeat(64),
            size: 128,
        });
        assert!(parse_index(&serde_json::to_vec(&index).unwrap()).is_ok());
        index.plugins[0].component_url = "https://example.invalid/component.wasm".into();
        assert!(parse_index(&serde_json::to_vec(&index).unwrap()).is_err());
        index.plugins[0].component_url.clear();
        for size in [0, MAX_CATALOG_PACKAGE_BYTES + 1] {
            index.plugins[0].package.as_mut().unwrap().size = size;
            assert!(parse_index(&serde_json::to_vec(&index).unwrap()).is_err());
        }
    }
    #[test]
    fn parses_current_and_future_entries_without_granting_runtime_support() {
        let mut index = fixture();
        assert_eq!(
            parse_index(&serde_json::to_vec(&index).unwrap()).unwrap(),
            index
        );
        let mut future = index.plugins[0].clone();
        future.manifest.id = "future-tool".into();
        future.manifest.api_version = "2.0.0".into();
        future.manifest.capabilities = vec!["tool".into()];
        index.plugins.push(future);
        let parsed = parse_index(&serde_json::to_vec(&index).unwrap()).unwrap();
        assert!(parsed.plugins[0].supported());
        assert!(!parsed.plugins[1].supported());
        assert_eq!(
            parsed.plugins[1].manifest.validate().unwrap_err().code,
            "PLUGIN_UNSUPPORTED_API"
        );
        index.plugins[1].manifest.component_sha256 = "bad".into();
        assert!(parse_index(&serde_json::to_vec(&index).unwrap()).is_err());
        index = fixture();
        index.schema_version = 2;
        assert_eq!(
            parse_index(&serde_json::to_vec(&index).unwrap())
                .unwrap_err()
                .code,
            "PLUGIN_UNSUPPORTED_CATALOG"
        );
    }
    #[test]
    fn real_report_and_provider_metadata_remain_catalog_browsable() {
        for raw in [
            include_str!("../../../examples/modpack-inspector/plugin.json"),
            include_str!("../../../examples/mcs-provider/plugin.json"),
        ] {
            let manifest: PluginManifest = serde_json::from_str(raw).unwrap();
            let mut index = fixture();
            index.plugins[0].manifest = manifest;
            let icon = if index.plugins[0].manifest.id == "modpack-inspector" {
                include_bytes!("../../../examples/modpack-inspector/icon.png").as_slice()
            } else {
                include_bytes!("../../../examples/mcs-provider/icon.png").as_slice()
            };
            use base64::Engine;
            index.plugins[0].icon_png_base64 =
                Some(base64::engine::general_purpose::STANDARD.encode(icon));
            let parsed = parse_index(&serde_json::to_vec(&index).unwrap()).unwrap();
            assert!(parsed.plugins[0].supported());
        }
    }
    #[test]
    fn rejects_duplicates_bad_urls_unknown_fields_and_oversize() {
        let mut index = fixture();
        index.plugins.push(index.plugins[0].clone());
        assert!(parse_index(&serde_json::to_vec(&index).unwrap()).is_err());
        for url in [
            "http://example.invalid/a",
            "https://user:secret@example.invalid/a",
            "https://example.invalid/a#x",
            "https://",
            "https://example.invalid/a b",
        ] {
            assert!(validate_url(url).is_err(), "{url}");
        }
        let mut value = serde_json::to_value(fixture()).unwrap();
        value["unexpected"] = true.into();
        assert!(parse_index(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(parse_index(&vec![b' '; MAX_CATALOG_BYTES + 1]).is_err());
    }
    #[test]
    fn checked_in_official_catalog_is_a_valid_empty_index_until_published() {
        let index = parse_index(include_bytes!("../../../catalog/index.json")).unwrap();
        assert_eq!(index.name, "CobbleLauncher Official");
        assert!(index.plugins.is_empty());
    }
}
