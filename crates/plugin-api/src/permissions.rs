// SPDX-License-Identifier: MIT
//! Closed declarations describe requested operations, never an authorization grant.
use crate::{
    ComposerPermission, PluginError, PluginManifest, REPORT_CONTEXT_API_VERSION, SettingScope,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "permission", deny_unknown_fields)]
pub enum PermissionNode {
    #[serde(rename = "launcher:ui/webview")]
    UiWebview {},
    #[serde(rename = "launcher:game-config/read")]
    GameConfigRead {},
    #[serde(rename = "launcher:game-diagnostics/read")]
    GameDiagnosticsRead {},
    #[serde(rename = "launcher:game-inspection/read")]
    GameInspectionRead { scopes: Vec<crate::InspectionScope> },
    #[serde(rename = "launcher:game-keybindings/read")]
    GameKeybindingsRead {},
    #[serde(rename = "launcher:game-keybindings/write")]
    GameKeybindingsWrite {},
    #[serde(rename = "launcher:resource/selected-folder/read")]
    SelectedFolderRead { extensions: Vec<String> },
    #[serde(rename = "launcher:images/selected/read")]
    SelectedImagesRead {},
    #[serde(rename = "launcher:images/output/propose")]
    ImageOutputPropose {},
    #[serde(rename = "launcher:resource/selected/read")]
    SelectedFileRead { extensions: Vec<String> },
    #[serde(rename = "launcher:provider/https/get")]
    ProviderGet { origins: Vec<String> },
    #[serde(rename = "launcher:composer/draft")]
    ComposerDraft {},
    #[serde(rename = "launcher:composer/context/public-identity")]
    ComposerPublicIdentity {},
    #[serde(rename = "launcher:composer/context/selected-peer")]
    ComposerSelectedPeer {},
    #[serde(rename = "launcher:composer/context/selected-session")]
    ComposerSelectedSession {},
    #[serde(rename = "launcher:settings/read")]
    SettingsRead { keys: Vec<String> },
}
impl PermissionNode {
    fn canonical(&self) -> Self {
        match self {
            Self::GameInspectionRead { scopes } => {
                let mut scopes = scopes.clone();
                scopes.sort();
                scopes.dedup();
                Self::GameInspectionRead { scopes }
            }
            Self::SelectedFolderRead { extensions } => Self::SelectedFolderRead {
                extensions: sorted(extensions.clone()),
            },
            Self::SelectedFileRead { extensions } => Self::SelectedFileRead {
                extensions: sorted(extensions.clone()),
            },
            Self::ProviderGet { origins } => Self::ProviderGet {
                origins: sorted(origins.clone()),
            },
            Self::SettingsRead { keys } => Self::SettingsRead {
                keys: sorted(keys.clone()),
            },
            _ => self.clone(),
        }
    }
    fn covers(&self, requested: &Self) -> bool {
        match (self, requested) {
            (
                Self::GameInspectionRead { scopes: granted },
                Self::GameInspectionRead { scopes: requested },
            ) => requested.iter().all(|scope| granted.contains(scope)),
            (
                Self::SelectedFolderRead {
                    extensions: granted,
                },
                Self::SelectedFolderRead {
                    extensions: requested,
                },
            ) => requested.iter().all(|x| granted.contains(x)),
            (
                Self::SelectedFileRead {
                    extensions: granted,
                },
                Self::SelectedFileRead {
                    extensions: requested,
                },
            ) => requested.iter().all(|x| granted.contains(x)),
            (Self::ProviderGet { origins: granted }, Self::ProviderGet { origins: requested }) => {
                requested.iter().all(|x| granted.contains(x))
            }
            (Self::SettingsRead { keys: granted }, Self::SettingsRead { keys: requested }) => {
                requested.iter().all(|x| granted.contains(x))
            }
            _ => self == requested,
        }
    }
}

#[cfg(test)]
mod diagnostic_permission_tests {
    use super::*;
    #[test]
    fn diagnostics_is_explicit_scoped_and_requires_review_on_upgrade() {
        let manifest: PluginManifest =
            serde_json::from_str(include_str!("../../../examples/crash-doctor/plugin.json"))
                .unwrap();
        manifest.validate_permissions().unwrap();
        let mut previous = manifest.clone();
        previous.frontend = None;
        previous.permissions.as_mut().unwrap().retain(|node| {
            !matches!(
                node,
                PermissionNode::UiWebview {} | PermissionNode::GameDiagnosticsRead {}
            )
        });
        previous.validate_permissions().unwrap();
        assert!(
            manifest
                .permission_delta(&previous)
                .added
                .contains(&PermissionNode::GameDiagnosticsRead {})
        );
        let mut invalid = manifest.clone();
        invalid.frontend = None;
        assert!(invalid.validate_permissions().is_err());
        invalid = manifest;
        invalid.file_extensions.push("json".into());
        assert!(invalid.validate_permissions().is_err());
        assert!(serde_json::from_value::<PermissionNode>(serde_json::json!({"permission":"launcher:game-diagnostics/read","path":"C:/private"})).is_err());
    }
}
#[cfg(test)]
mod inspection_permission_tests {
    use super::*;
    use crate::InspectionScope;
    fn manifest(scopes: serde_json::Value) -> PluginManifest {
        serde_json::from_value(serde_json::json!({"id":"inspector","name":"Inspector","publisher":"Fixture","version":"0.1.0","api_version":"0.2.0","capabilities":["report"],"file_extensions":["json"],"component_sha256":"a".repeat(64),"frontend":{"api_version":"0.1.0","entry":"ui/index.html","assets_sha256":"b".repeat(64)},"permissions":[{"permission":"launcher:ui/webview"},{"permission":"launcher:game-inspection/read","scopes":scopes}]})).unwrap()
    }
    #[test]
    fn explicit_inspection_scopes_need_no_selected_file_grant_and_expand_by_subset() {
        let previous = manifest(serde_json::json!(["mods"]));
        previous.validate_permissions().unwrap();
        let next = manifest(serde_json::json!(["mods", "packs"]));
        next.validate_permissions().unwrap();
        assert_eq!(
            next.permission_delta(&previous).added,
            vec![PermissionNode::GameInspectionRead {
                scopes: vec![InspectionScope::Mods, InspectionScope::Packs]
            }]
        );
        assert!(previous.permission_delta(&next).added.is_empty());
        assert!(
            !next
                .effective_permissions()
                .iter()
                .any(|node| matches!(node, PermissionNode::SelectedFileRead { .. }))
        );
        assert!(
            manifest(serde_json::json!([]))
                .validate_permissions()
                .is_err()
        );
        assert!(
            manifest(serde_json::json!(["mods", "mods"]))
                .validate_permissions()
                .is_err()
        );
        let mut invalid = next;
        invalid.api_version = "0.1.0".into();
        assert!(invalid.validate_permissions().is_err());
        assert!(serde_json::from_value::<PermissionNode>(serde_json::json!({"permission":"launcher:game-inspection/read","scopes":["accounts"]})).is_err());
        assert!(serde_json::from_value::<PermissionNode>(serde_json::json!({"permission":"launcher:game-inspection/read","scopes":["mods"],"path":"C:/private"})).is_err());
    }
    #[test]
    fn host_snapshot_workbench_does_not_need_a_file_association() {
        let mut host = manifest(serde_json::json!(["mods"]));
        host.file_extensions.clear();
        host.validate().unwrap();
        assert!(host.is_inspection_report());
        let mut plain = host.clone();
        plain.frontend = None;
        plain.permissions = None;
        assert!(!plain.is_inspection_report());
        assert!(plain.validate().is_err());
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionDelta {
    pub added: Vec<PermissionNode>,
    pub removed: Vec<PermissionNode>,
}
impl PermissionDelta {
    pub fn requires_approval(&self) -> bool {
        !self.added.is_empty()
    }
}
fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}
impl PluginManifest {
    fn inferred_permissions(&self) -> Vec<PermissionNode> {
        let mut nodes = Vec::new();
        if self.frontend.is_some() {
            nodes.push(PermissionNode::UiWebview {});
            if let Some(scopes) = self
                .permissions
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find_map(|node| {
                    if let PermissionNode::GameInspectionRead { scopes } = node {
                        Some(scopes)
                    } else {
                        None
                    }
                })
            {
                nodes.push(
                    PermissionNode::GameInspectionRead {
                        scopes: scopes.clone(),
                    }
                    .canonical(),
                );
            }
            for node in [
                PermissionNode::GameKeybindingsRead {},
                PermissionNode::GameKeybindingsWrite {},
                PermissionNode::GameDiagnosticsRead {},
            ] {
                if self
                    .permissions
                    .as_ref()
                    .is_some_and(|nodes| nodes.contains(&node))
                {
                    nodes.push(node);
                }
            }
            if self
                .permissions
                .as_ref()
                .is_some_and(|nodes| nodes.contains(&PermissionNode::GameConfigRead {}))
            {
                nodes.push(PermissionNode::GameConfigRead {});
            }
        }
        if self.report_protocols == [crate::CONFIG_FOLDER_PROTOCOL] {
            nodes.push(PermissionNode::SelectedFolderRead {
                extensions: sorted(self.file_extensions.clone()),
            });
        }
        if self.capabilities == ["image-layout"] {
            nodes.push(PermissionNode::SelectedImagesRead {});
            nodes.push(PermissionNode::ImageOutputPropose {});
        }
        if (self.capabilities == ["report"] || self.capabilities == ["viewer"])
            && !self
                .permissions
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|node| matches!(node, PermissionNode::GameInspectionRead { .. }))
        {
            nodes.push(PermissionNode::SelectedFileRead {
                extensions: sorted(self.file_extensions.clone()),
            });
        }
        if self.capabilities == ["blueprint-provider"] {
            nodes.push(PermissionNode::ProviderGet {
                origins: sorted(self.provider_origins.clone()),
            });
        }
        for permission in &self.composer_permissions {
            nodes.push(match permission {
                ComposerPermission::Draft => PermissionNode::ComposerDraft {},
                ComposerPermission::PublicIdentity => PermissionNode::ComposerPublicIdentity {},
                ComposerPermission::SelectedPeer => PermissionNode::ComposerSelectedPeer {},
                ComposerPermission::SelectedSession => PermissionNode::ComposerSelectedSession {},
            });
        }
        if self.capabilities == ["report"] && self.api_version == REPORT_CONTEXT_API_VERSION {
            let keys = self
                .contributions
                .as_ref()
                .and_then(|c| c.settings.as_ref())
                .map(|settings| {
                    settings
                        .fields()
                        .into_iter()
                        .map(|field| {
                            format!(
                                "{}:{}",
                                match field.scope {
                                    SettingScope::Global => "global",
                                    SettingScope::Instance => "instance",
                                },
                                field.id
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let keys = sorted(keys);
            if !keys.is_empty() {
                nodes.push(PermissionNode::SettingsRead { keys });
            }
        }
        nodes.sort();
        nodes.dedup();
        nodes
    }
    /// Requested operations only; host consent and per-invocation scope remain mandatory.
    pub fn effective_permissions(&self) -> Vec<PermissionNode> {
        let mut nodes = self
            .permissions
            .as_ref()
            .map(|nodes| {
                nodes
                    .iter()
                    .map(PermissionNode::canonical)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| self.inferred_permissions());
        nodes.sort();
        nodes.dedup();
        nodes
    }
    pub fn validate_permissions(&self) -> Result<(), PluginError> {
        let requested = self.permissions.as_deref().unwrap_or_default();
        if requested.iter().any(|node| matches!(node,PermissionNode::GameInspectionRead { scopes } if scopes.is_empty() || scopes.len()>10)) || (requested.iter().any(|node|matches!(node,PermissionNode::GameInspectionRead { .. })) && (self.frontend.is_none() || self.capabilities != ["report"] || self.api_version != REPORT_CONTEXT_API_VERSION || (!self.file_extensions.is_empty() && self.file_extensions != ["json"]) || !self.report_protocols.is_empty())) {
            return Err(PluginError::new("PLUGIN_INVALID_PERMISSIONS", "Inspection requires explicit bounded scopes and a JSON report 0.2 frontend"));
        }
        if requested.contains(&PermissionNode::GameDiagnosticsRead {})
            && (self.frontend.is_none()
                || self.capabilities != ["report"]
                || !self.report_protocols.is_empty()
                || !self
                    .file_extensions
                    .iter()
                    .all(|extension| matches!(extension.as_str(), "log" | "txt")))
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Game diagnostics require a plain-text report frontend",
            ));
        }
        if requested.iter().any(|node| {
            matches!(
                node,
                PermissionNode::GameKeybindingsRead {} | PermissionNode::GameKeybindingsWrite {}
            )
        }) && (self.frontend.is_none()
            || !requested.contains(&PermissionNode::GameKeybindingsRead {}))
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Keybinding operations require a frontend and explicit read scope",
            ));
        }
        if self.frontend.is_some() && self.permissions.is_none() {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Frontend execution requires explicit permission declarations",
            ));
        }
        if self
            .permissions
            .as_ref()
            .is_some_and(|nodes| nodes.contains(&PermissionNode::GameConfigRead {}))
            && (self.frontend.is_none()
                || self.capabilities != ["report"]
                || self.api_version != REPORT_CONTEXT_API_VERSION
                || self.report_protocols != [crate::CONFIG_FOLDER_PROTOCOL])
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Managed game configuration reads require a context-aware frontend report implementation",
            ));
        }
        if self.report_protocols == [crate::CONFIG_FOLDER_PROTOCOL] && self.permissions.is_none() {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Selected-folder snapshots require explicit scoped permission declarations",
            ));
        }
        if self.capabilities == ["image-layout"] && self.permissions.is_none() {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Image layout requires explicit selected-image and output-proposal declarations",
            ));
        }
        if let Some(nodes) = &self.permissions {
            let canonical = self.effective_permissions();
            let duplicate_scopes = nodes.iter().any(|node| match node {
                PermissionNode::GameInspectionRead { scopes } => {
                    let mut unique = scopes.clone();
                    unique.sort();
                    unique.dedup();
                    unique.len() != scopes.len()
                }
                PermissionNode::SelectedFolderRead { extensions } => {
                    sorted(extensions.clone()).len() != extensions.len()
                }
                PermissionNode::SelectedFileRead { extensions } => {
                    sorted(extensions.clone()).len() != extensions.len()
                }
                PermissionNode::ProviderGet { origins } => {
                    sorted(origins.clone()).len() != origins.len()
                }
                PermissionNode::SettingsRead { keys } => sorted(keys.clone()).len() != keys.len(),
                _ => false,
            });
            if nodes.len() > 8
                || duplicate_scopes
                || canonical.len() != nodes.len()
                || canonical != self.inferred_permissions()
            {
                return Err(PluginError::new(
                    "PLUGIN_INVALID_PERMISSIONS",
                    "Permission nodes must exactly describe the supported workflow and declared scopes",
                ));
            }
        }
        Ok(())
    }
    pub fn permission_delta(&self, previous: &PluginManifest) -> PermissionDelta {
        let mut current = self.effective_permissions();
        current.sort();
        let mut previous = previous.effective_permissions();
        previous.sort();
        PermissionDelta {
            added: current
                .iter()
                .filter(|node| !previous.iter().any(|old| old.covers(node)))
                .cloned()
                .collect(),
            removed: previous
                .iter()
                .filter(|node| !current.iter().any(|new| new.covers(node)))
                .cloned()
                .collect(),
        }
    }
    pub fn requires_permission_approval(&self, previous: &PluginManifest) -> bool {
        self.permission_delta(previous).requires_approval()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keybinding_write_is_explicit_and_requires_frontend_read_scope() {
        let mut manifest: PluginManifest =
            serde_json::from_str(include_str!("../../../examples/config-doctor/plugin.json"))
                .unwrap();
        manifest.validate_permissions().unwrap();
        let mut previous = manifest.clone();
        previous
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::GameKeybindingsWrite {}));
        previous.validate_permissions().unwrap();
        assert_eq!(
            manifest.permission_delta(&previous).added,
            vec![PermissionNode::GameKeybindingsWrite {}]
        );
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::GameKeybindingsRead {}));
        assert!(manifest.validate_permissions().is_err());
        previous.frontend = None;
        assert!(previous.validate_permissions().is_err());
    }
    fn report() -> PluginManifest {
        serde_json::from_value(serde_json::json!({"id":"doctor","name":"Doctor","publisher":"test","version":"1.0.0","api_version":"0.2.0","capabilities":["report"],"file_extensions":["json","toml"],"component_sha256":"0".repeat(64)})).unwrap()
    }
    #[test]
    fn legacy_and_explicit_requests_are_equivalent_and_closed() {
        let old = report();
        old.validate().unwrap();
        let mut explicit = old.clone();
        explicit.permissions = Some(vec![PermissionNode::SelectedFileRead {
            extensions: vec!["toml".into(), "json".into()],
        }]);
        explicit.validate().unwrap();
        assert_eq!(
            old.effective_permissions(),
            explicit.effective_permissions()
        );
        assert!(!explicit.requires_permission_approval(&old));
        explicit.permissions = Some(vec![]);
        assert_eq!(
            explicit.validate().unwrap_err().code,
            "PLUGIN_INVALID_PERMISSIONS"
        );
        for invalid in [
            r#"{"permission":"launcher:*"}"#,
            r#"{"permission":"launcher:composer/draft","path":"/tmp"}"#,
        ] {
            assert!(serde_json::from_str::<PermissionNode>(invalid).is_err());
        }
        explicit.permissions = Some(vec![PermissionNode::SelectedFileRead {
            extensions: vec!["json".into(), "json".into(), "toml".into()],
        }]);
        assert!(explicit.validate().is_err());
    }
    #[test]
    fn extensions_origins_and_context_expansion_require_review_but_reductions_do_not() {
        let old = report();
        let mut new = old.clone();
        new.file_extensions.push("yaml".into());
        new.validate().unwrap();
        assert!(new.requires_permission_approval(&old));
        assert!(!old.requires_permission_approval(&new));
        let mut provider = old.clone();
        provider.capabilities = vec!["blueprint-provider".into()];
        provider.file_extensions.clear();
        provider.provider_origins = vec!["https://one.example".into()];
        provider.validate().unwrap();
        let mut expanded = provider.clone();
        expanded.provider_origins.push("https://two.example".into());
        expanded.validate().unwrap();
        assert!(expanded.requires_permission_approval(&provider));
        assert!(!provider.requires_permission_approval(&expanded));
        let mut composer = old;
        composer.api_version = "0.1.0".into();
        composer.capabilities = vec!["message-composer".into()];
        composer.file_extensions.clear();
        composer.composer_permissions = vec![ComposerPermission::Draft];
        composer.validate().unwrap();
        let mut contextual = composer.clone();
        contextual
            .composer_permissions
            .push(ComposerPermission::SelectedPeer);
        contextual.validate().unwrap();
        assert!(contextual.requires_permission_approval(&composer));
    }

    #[test]
    fn settings_scopes_are_declared_context_not_write_authority() {
        let old = report();
        let mut value = serde_json::to_value(&old).unwrap();
        value["contributions"] = serde_json::json!({"schema_version":1,"commands":[],"settings":{"categories":[{"id":"display","label":{"en":"Display"},"fields":[{"id":"compact","type":"boolean","label":{"en":"Compact"},"scope":"global","advanced":false,"default":false}]}]}});
        let mut with_settings: PluginManifest = serde_json::from_value(value).unwrap();
        with_settings.validate().unwrap();
        assert!(
            with_settings
                .effective_permissions()
                .contains(&PermissionNode::SettingsRead {
                    keys: vec!["global:compact".into()]
                })
        );
        assert!(with_settings.requires_permission_approval(&old));
        with_settings.permissions = Some(vec![PermissionNode::ComposerDraft {}]);
        assert!(with_settings.validate().is_err());
    }
    #[test]
    fn mcmeta_report_support_does_not_raise_per_plugin_extension_limit() {
        let mut manifest = report();
        manifest.file_extensions = vec!["mcmeta".into()];
        manifest.validate().unwrap();
        manifest.permissions = Some(manifest.effective_permissions());
        manifest.validate().unwrap();
        manifest.permissions = None;
        manifest.file_extensions = [
            "log",
            "txt",
            "json",
            "mcmeta",
            "mrpack",
            "toml",
            "properties",
            "yaml",
            "yml",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert!(manifest.validate().is_err());
    }
}
