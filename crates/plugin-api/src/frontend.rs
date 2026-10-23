// SPDX-License-Identifier: MIT
//! Static, hashed frontend bundles. This metadata never authorizes browser IPC.
use crate::PluginError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub const FRONTEND_API_VERSION: &str = "0.1.0";
pub const WEB_FRONTEND_INTERFACE: &str = "launcher:web-frontend";
pub const FRONTEND_ASSET_INDEX: &str = "frontend-assets.json";
pub const MAX_FRONTEND_INDEX_BYTES: usize = 64 * 1024;
pub const MAX_FRONTEND_ASSETS: usize = 128;
pub const MAX_FRONTEND_ASSET_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_FRONTEND_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_FRONTEND_ENTRY_BYTES: u64 = 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontendManifest {
    pub api_version: String,
    pub entry: String,
    pub assets_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontendAsset {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontendAssetIndex {
    pub schema_version: u32,
    pub files: Vec<FrontendAsset>,
}
fn invalid() -> PluginError {
    PluginError::new(
        "PLUGIN_INVALID_FRONTEND",
        "Invalid or oversized static frontend bundle",
    )
}
fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
/// Safe package-relative paths only; serving still verifies caller, index and file hash.
pub fn frontend_asset_media_type(path: &str) -> Option<&'static str> {
    if path.len() > 160
        || !path.starts_with("ui/")
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
        || path.split('/').any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || part.ends_with('.')
                || matches!(
                    part.to_ascii_lowercase().split('.').next().unwrap_or(""),
                    "con"
                        | "prn"
                        | "aux"
                        | "nul"
                        | "com1"
                        | "com2"
                        | "com3"
                        | "com4"
                        | "com5"
                        | "com6"
                        | "com7"
                        | "com8"
                        | "com9"
                        | "lpt1"
                        | "lpt2"
                        | "lpt3"
                        | "lpt4"
                        | "lpt5"
                        | "lpt6"
                        | "lpt7"
                        | "lpt8"
                        | "lpt9"
                )
        })
    {
        return None;
    }
    match path.rsplit_once('.')?.1 {
        "html" => Some("text/html; charset=utf-8"),
        "js" | "mjs" => Some("text/javascript; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "json" => Some("application/json"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "woff2" => Some("font/woff2"),
        _ => None,
    }
}
impl FrontendManifest {
    pub fn validate_metadata(&self) -> Result<(), PluginError> {
        if self.api_version.is_empty()
            || self.api_version.len() > 40
            || self.entry != "ui/index.html"
            || !digest(&self.assets_sha256)
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), PluginError> {
        self.validate_metadata()?;
        if self.api_version != FRONTEND_API_VERSION {
            return Err(PluginError::new(
                "PLUGIN_UNSUPPORTED_API",
                "Unsupported frontend API",
            ));
        }
        Ok(())
    }
}
impl FrontendAssetIndex {
    pub fn validate(&self, frontend: &FrontendManifest) -> Result<(), PluginError> {
        frontend.validate_metadata()?;
        let mut paths = BTreeSet::new();
        let mut total = 0u64;
        let mut entry = false;
        if self.schema_version != 1
            || self.files.is_empty()
            || self.files.len() > MAX_FRONTEND_ASSETS
        {
            return Err(invalid());
        }
        for file in &self.files {
            if frontend_asset_media_type(&file.path).is_none()
                || !digest(&file.sha256)
                || file.bytes > MAX_FRONTEND_ASSET_BYTES
                || !paths.insert(file.path.to_ascii_lowercase())
            {
                return Err(invalid());
            }
            if file.path == frontend.entry {
                if file.bytes == 0 || file.bytes > MAX_FRONTEND_ENTRY_BYTES {
                    return Err(invalid());
                }
                entry = true;
            }
            total = total.checked_add(file.bytes).ok_or_else(invalid)?;
        }
        if !entry
            || total > MAX_FRONTEND_TOTAL_BYTES
            || serde_json::to_vec(self).map_err(|_| invalid())?.len() > MAX_FRONTEND_INDEX_BYTES
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frontend() -> FrontendManifest {
        FrontendManifest {
            api_version: "0.1.0".into(),
            entry: "ui/index.html".into(),
            assets_sha256: "a".repeat(64),
        }
    }
    fn index() -> FrontendAssetIndex {
        FrontendAssetIndex {
            schema_version: 1,
            files: vec![FrontendAsset {
                path: "ui/index.html".into(),
                sha256: "b".repeat(64),
                bytes: 100,
            }],
        }
    }
    #[test]
    fn frontend_index_is_closed_and_bounded() {
        index().validate(&frontend()).unwrap();
        let mut i = index();
        i.files.push(i.files[0].clone());
        assert!(i.validate(&frontend()).is_err());
        let mut i = index();
        i.files[0].bytes = MAX_FRONTEND_ENTRY_BYTES + 1;
        assert!(i.validate(&frontend()).is_err());
        for path in [
            "../ui/index.html",
            "ui/../secret.js",
            "ui/a%2fsecret.js",
            "ui/con.js",
            "ui/test.svg",
            "/ui/index.html",
            "ui/a.js?x",
        ] {
            assert!(frontend_asset_media_type(path).is_none(), "{path}");
        }
        assert!(serde_json::from_value::<FrontendManifest>(serde_json::json!({"api_version":"0.1.0","entry":"ui/index.html","assets_sha256":"a".repeat(64),"url":"https://example.com"})).is_err());
    }
    #[test]
    fn frontend_composes_with_report_and_requires_reviewed_scoped_permissions() {
        let mut manifest:crate::PluginManifest=serde_json::from_value(serde_json::json!({"id":"doctor","name":"Doctor","publisher":"Original","version":"0.1.0","api_version":"0.2.0","capabilities":["report"],"file_extensions":["toml"],"report_protocols":["cobble.config-folder@1"],"component_sha256":"a".repeat(64),"frontend":{"api_version":"0.1.0","entry":"ui/index.html","assets_sha256":"b".repeat(64)},"permissions":[{"permission":"launcher:resource/selected/read","extensions":["toml"]},{"permission":"launcher:resource/selected-folder/read","extensions":["toml"]},{"permission":"launcher:ui/webview"},{"permission":"launcher:game-config/read"}]})).unwrap();
        manifest.validate().unwrap();
        for (id, version) in [
            (crate::REPORT_INTERFACE, "^0.2.0"),
            (WEB_FRONTEND_INTERFACE, "^0.1.0"),
        ] {
            assert!(manifest.supports_interface(&crate::InterfaceRequirement {
                id: id.into(),
                version: version.into()
            }));
        }
        assert_eq!(manifest.provided_interfaces().len(), 2);
        let mut old = manifest.clone();
        old.frontend = None;
        old.permissions.as_mut().unwrap().retain(|node| {
            !matches!(
                node,
                crate::PermissionNode::UiWebview {} | crate::PermissionNode::GameConfigRead {}
            )
        });
        old.validate().unwrap();
        assert!(manifest.requires_permission_approval(&old));
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, crate::PermissionNode::UiWebview {}));
        assert!(manifest.validate().is_err());
        let mut foreign = old;
        foreign
            .permissions
            .as_mut()
            .unwrap()
            .push(crate::PermissionNode::GameConfigRead {});
        assert!(foreign.validate().is_err());
    }
}
