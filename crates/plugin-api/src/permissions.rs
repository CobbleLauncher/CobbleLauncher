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
    #[serde(rename = "launcher:storage/private")]
    PrivateStorage {},
    #[serde(rename = "launcher:clipboard/write")]
    ClipboardWrite {},
    #[serde(rename = "launcher:exports/text")]
    TextExport {},
    #[serde(rename = "launcher:blueprints/generated-save")]
    GeneratedBlueprintSave {},
    #[serde(rename = "launcher:game-config/write")]
    GameConfigWrite {},
    #[serde(rename = "launcher:storage/cache/clear")]
    StorageCacheClear {},
    #[serde(rename = "launcher:instances/move-request")]
    InstanceMoveRequest {},
    #[serde(rename = "launcher:instances/copy-request")]
    InstanceCopyRequest {},
    #[serde(rename = "launcher:game-recovery/read")]
    GameRecoveryRead {},
    #[serde(rename = "launcher:game-recovery/open-request")]
    GameRecoveryOpenRequest {},
    #[serde(rename = "launcher:play-manifest/export")]
    PlayManifestExport {},
    #[serde(rename = "launcher:files/import-request")]
    FileInboxImportRequest {},
    #[serde(rename = "launcher:game-config/read")]
    GameConfigRead {},
    #[serde(rename = "launcher:game-diagnostics/read")]
    GameDiagnosticsRead {},
    #[serde(rename = "launcher:ai/chat")]
    AiChat {},
    #[serde(rename = "launcher:ai/configure")]
    AiConfigure {},
    #[serde(rename = "launcher:mesh/events/send")]
    MeshEventsSend { namespaces: Vec<String> },
    #[serde(rename = "launcher:mesh/events/receive")]
    MeshEventsReceive { namespaces: Vec<String> },
    #[serde(rename = "launcher:game-inspection/read")]
    GameInspectionRead { scopes: Vec<crate::InspectionScope> },
    #[serde(rename = "launcher:world-snapshots/read")]
    WorldSnapshotsRead {},
    #[serde(rename = "launcher:worlds/seed/read")]
    WorldSeedRead {},
    #[serde(rename = "launcher:world-snapshots/restore-request")]
    WorldSnapshotsRestoreRequest {},
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
            Self::MeshEventsSend { namespaces } => Self::MeshEventsSend {
                namespaces: sorted(namespaces.clone()),
            },
            Self::MeshEventsReceive { namespaces } => Self::MeshEventsReceive {
                namespaces: sorted(namespaces.clone()),
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
            (
                Self::MeshEventsSend {
                    namespaces: granted,
                },
                Self::MeshEventsSend {
                    namespaces: requested,
                },
            )
            | (
                Self::MeshEventsReceive {
                    namespaces: granted,
                },
                Self::MeshEventsReceive {
                    namespaces: requested,
                },
            ) => requested.iter().all(|x| granted.contains(x)),
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
        previous.permissions = Some(vec![PermissionNode::SelectedFileRead {
            extensions: previous.file_extensions.clone(),
        }]);
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
mod private_storage_permission_tests {
    use super::*;

    fn manifest() -> PluginManifest {
        serde_json::from_value(serde_json::json!({
            "id": "private-data", "name": "Private data", "publisher": "Fixture",
            "version": "0.1.0", "api_version": "0.2.0", "capabilities": ["report"],
            "file_extensions": [], "component_sha256": "a".repeat(64),
            "frontend": {"api_version": "0.1.0", "entry": "ui/index.html", "assets_sha256": "b".repeat(64)},
            "permissions": [{"permission": "launcher:ui/webview"}, {"permission": "launcher:storage/private"}]
        }))
        .unwrap()
    }

    #[test]
    fn private_data_is_explicit_and_does_not_infer_file_access() {
        let mut current = manifest();
        current.validate().unwrap();
        assert!(current.is_private_data_report());
        assert_eq!(
            current.effective_permissions(),
            vec![
                PermissionNode::UiWebview {},
                PermissionNode::PrivateStorage {}
            ]
        );
        current.permissions.as_mut().unwrap().pop();
        assert!(!current.is_private_data_report());
        assert!(current.validate().is_err());
        assert!(
            !current
                .effective_permissions()
                .contains(&PermissionNode::PrivateStorage {})
        );
    }

    #[test]
    fn adding_private_data_requires_review_and_an_existing_frontend() {
        let mut previous = manifest();
        previous.file_extensions = vec!["json".into()];
        previous.permissions = Some(vec![
            PermissionNode::UiWebview {},
            PermissionNode::SelectedFileRead {
                extensions: vec!["json".into()],
            },
        ]);
        previous.validate().unwrap();
        let mut current = previous.clone();
        current
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::PrivateStorage {});
        current.validate().unwrap();
        assert_eq!(
            current.permission_delta(&previous).added,
            vec![PermissionNode::PrivateStorage {}]
        );
        assert!(current.requires_permission_approval(&previous));
        current.frontend = None;
        assert!(current.validate_permissions().is_err());
    }

    #[test]
    fn private_data_declaration_rejects_paths_scopes_and_other_parameters() {
        let node = serde_json::json!({"permission": "launcher:storage/private"});
        assert_eq!(
            serde_json::from_value::<PermissionNode>(node.clone()).unwrap(),
            PermissionNode::PrivateStorage {}
        );
        for extra in ["path", "scopes", "keys", "plugin_id"] {
            let mut invalid = node.clone();
            invalid[extra] = serde_json::json!("arbitrary");
            assert!(serde_json::from_value::<PermissionNode>(invalid).is_err());
        }
    }

    #[test]
    fn storage_can_be_used_by_an_existing_non_report_frontend() {
        let mut current = manifest();
        current.api_version = "0.1.0".into();
        current.capabilities = vec!["image-layout".into()];
        current.permissions.as_mut().unwrap().extend([
            PermissionNode::SelectedImagesRead {},
            PermissionNode::ImageOutputPropose {},
        ]);
        current.validate().unwrap();
        assert!(!current.is_private_data_report());
    }
}
#[cfg(test)]
mod maintenance_permission_tests {
    use super::*;
    fn fixture(nodes: serde_json::Value) -> PluginManifest {
        serde_json::from_value(serde_json::json!({"id":"maintenance-fixture","name":"Maintenance fixture","version":"0.1.0","api_version":"0.2.0","publisher":"Fixture","capabilities":["report"],"file_extensions":[],"component_sha256":"a".repeat(64),"frontend":{"api_version":"0.1.0","entry":"ui/index.html","assets_sha256":"b".repeat(64)},"permissions":nodes})).unwrap()
    }
    #[test]
    fn maintenance_storage_actions_require_storage_scope_and_explicit_frontend() {
        for permission in [
            "launcher:storage/cache/clear",
            "launcher:instances/move-request",
            "launcher:instances/copy-request",
        ] {
            let mut manifest = fixture(
                serde_json::json!([{"permission":"launcher:ui/webview"},{"permission":"launcher:game-inspection/read","scopes":["storage"]},{"permission":permission}]),
            );
            manifest.validate().unwrap();
            manifest
                .permissions
                .as_mut()
                .unwrap()
                .retain(|node| !matches!(node, PermissionNode::GameInspectionRead { .. }));
            assert!(manifest.validate_permissions().is_err());
            manifest.frontend = None;
            assert!(manifest.validate_permissions().is_err());
            assert!(
                serde_json::from_value::<PermissionNode>(
                    serde_json::json!({"permission":permission,"path":"/private"})
                )
                .is_err()
            );
        }
    }
    #[test]
    fn maintenance_host_workflows_do_not_infer_file_reads_or_private_storage() {
        for permission in [
            "launcher:play-manifest/export",
            "launcher:files/import-request",
        ] {
            let manifest = fixture(
                serde_json::json!([{"permission":"launcher:ui/webview"},{"permission":permission}]),
            );
            manifest.validate().unwrap();
            assert!(manifest.is_host_workflow_report());
            assert!(
                !manifest.effective_permissions().iter().any(|node| matches!(
                    node,
                    PermissionNode::SelectedFileRead { .. } | PermissionNode::PrivateStorage {}
                ))
            );
            let mut missing = manifest;
            missing.frontend = None;
            assert!(missing.validate().is_err());
        }
    }
    #[test]
    fn maintenance_config_write_and_recovery_request_depend_on_their_exact_read_grants() {
        let mut config: PluginManifest =
            serde_json::from_str(include_str!("../../../examples/config-doctor/plugin.json"))
                .unwrap();
        config
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::GameConfigWrite {}));
        let previous = config.clone();
        config
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::GameConfigWrite {});
        config.validate().unwrap();
        assert!(config.requires_permission_approval(&previous));
        config
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::GameConfigRead {}));
        assert!(config.validate_permissions().is_err());
        let mut recovery: PluginManifest =
            serde_json::from_str(include_str!("../../../examples/crash-doctor/plugin.json"))
                .unwrap();
        recovery.permissions.as_mut().unwrap().retain(|node| {
            !matches!(
                node,
                PermissionNode::GameRecoveryRead {} | PermissionNode::GameRecoveryOpenRequest {}
            )
        });
        recovery
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::GameRecoveryOpenRequest {});
        assert!(recovery.validate_permissions().is_err());
        recovery
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::GameRecoveryRead {});
        recovery.validate_permissions().unwrap();
    }
}
#[cfg(test)]
mod text_export_permission_tests {
    use super::*;
    #[test]
    fn text_export_is_explicit_parameterless_frontend_access_requiring_upgrade_review() {
        let mut previous: PluginManifest =
            serde_json::from_str(include_str!("../../../examples/crash-doctor/plugin.json"))
                .unwrap();
        previous
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::TextExport {}));
        previous.validate_permissions().unwrap();
        assert!(
            !previous
                .effective_permissions()
                .contains(&PermissionNode::TextExport {})
        );
        let mut current = previous.clone();
        current
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::TextExport {});
        current.validate_permissions().unwrap();
        assert_eq!(
            current.permission_delta(&previous).added,
            vec![PermissionNode::TextExport {}]
        );
        assert!(current.requires_permission_approval(&previous));
        assert_eq!(
            serde_json::to_value(PermissionNode::TextExport {}).unwrap(),
            serde_json::json!({"permission":"launcher:exports/text"})
        );
        for invalid in [
            serde_json::json!({"permission":"launcher:exports/file"}),
            serde_json::json!({"permission":"launcher:exports/text","path":"/private"}),
            serde_json::json!({"permission":"launcher:exports/text","extensions":["txt"]}),
            serde_json::json!({"permission":"launcher:exports/text","read":true}),
            serde_json::json!({"permission":"launcher:exports/text","scopes":["worlds"]}),
        ] {
            assert!(serde_json::from_value::<PermissionNode>(invalid).is_err());
        }
        current
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::TextExport {});
        assert!(current.validate_permissions().is_err());
        current.permissions.as_mut().unwrap().pop();
        current.frontend = None;
        assert!(current.validate_permissions().is_err());
    }
    #[test]
    fn text_export_remains_independent_from_image_read_storage_and_clipboard_access() {
        let manifest: PluginManifest=serde_json::from_value(serde_json::json!({
            "id":"image-export-fixture","name":"Image export fixture","version":"0.1.0","api_version":"0.1.0","publisher":"Fixture",
            "capabilities":["image-layout"],"file_extensions":[],"component_sha256":"a".repeat(64),
            "frontend":{"api_version":"0.1.0","entry":"ui/index.html","assets_sha256":"b".repeat(64)},
            "permissions":[{"permission":"launcher:ui/webview"},{"permission":"launcher:images/selected/read"},{"permission":"launcher:images/output/propose"},{"permission":"launcher:storage/private"},{"permission":"launcher:clipboard/write"},{"permission":"launcher:exports/text"}]
        })).unwrap();
        manifest.validate().unwrap();
        let mut without = manifest.clone();
        without
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::TextExport {}));
        without.validate().unwrap();
        assert_eq!(
            manifest.permission_delta(&without).added,
            vec![PermissionNode::TextExport {}]
        );
    }
}
#[cfg(test)]
mod clipboard_permission_tests {
    use super::*;

    #[test]
    fn local_frontend_report_has_no_implicit_resource_access() {
        let mut manifest: PluginManifest =
            serde_json::from_str(include_str!("../../../examples/anvil-planner/plugin.json"))
                .unwrap();
        manifest.validate().unwrap();
        assert!(manifest.is_local_frontend_report());
        assert_eq!(manifest.effective_permissions().len(), 2);
        manifest.frontend = None;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn clipboard_write_is_declared_frontend_only_and_never_implies_read() {
        let mut previous: PluginManifest =
            serde_json::from_str(include_str!("../../../examples/crash-doctor/plugin.json"))
                .unwrap();
        previous
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::ClipboardWrite {}));
        previous.validate_permissions().unwrap();
        assert!(
            !previous
                .effective_permissions()
                .contains(&PermissionNode::ClipboardWrite {})
        );
        let mut current = previous.clone();
        current
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::ClipboardWrite {});
        current.validate_permissions().unwrap();
        assert_eq!(
            current.permission_delta(&previous).added,
            vec![PermissionNode::ClipboardWrite {}]
        );
        assert!(current.requires_permission_approval(&previous));
        assert_eq!(
            serde_json::to_value(PermissionNode::ClipboardWrite {}).unwrap(),
            serde_json::json!({"permission": "launcher:clipboard/write"})
        );
        for invalid in [
            serde_json::json!({"permission": "launcher:clipboard/read"}),
            serde_json::json!({"permission": "launcher:clipboard/write", "read": true}),
            serde_json::json!({"permission": "launcher:clipboard/write", "scopes": ["text"]}),
            serde_json::json!({"permission": "launcher:clipboard/write", "path": "/private"}),
        ] {
            assert!(serde_json::from_value::<PermissionNode>(invalid).is_err());
        }
        current.frontend = None;
        assert!(current.validate_permissions().is_err());
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
    #[test]
    fn diagnostic_recovery_inspection_preserves_text_abi_and_selected_log_access() {
        let mut current: PluginManifest =
            serde_json::from_str(include_str!("../../../examples/crash-doctor/plugin.json"))
                .unwrap();
        current.id = "another-diagnostic-tool".into();
        current.name = "Another diagnostic tool".into();
        current.validate().unwrap();
        assert_eq!(current.api_version, crate::REPORT_API_VERSION);
        assert!(
            current
                .effective_permissions()
                .contains(&PermissionNode::SelectedFileRead {
                    extensions: vec!["log".into(), "txt".into()]
                })
        );
        assert!(!current.is_inspection_report());
    }
    #[test]
    fn diagnostic_recovery_inspection_rejects_extra_scopes_missing_grants_and_protocols() {
        let current: PluginManifest =
            serde_json::from_str(include_str!("../../../examples/crash-doctor/plugin.json"))
                .unwrap();
        for required in [
            PermissionNode::GameDiagnosticsRead {},
            PermissionNode::GameRecoveryRead {},
            PermissionNode::InstanceCopyRequest {},
            PermissionNode::PrivateStorage {},
            PermissionNode::SelectedFileRead {
                extensions: vec!["log".into(), "txt".into()],
            },
        ] {
            let mut invalid = current.clone();
            invalid
                .permissions
                .as_mut()
                .unwrap()
                .retain(|node| node != &required);
            assert!(invalid.validate().is_err(), "missing grant {required:?}");
        }
        let mut invalid = current.clone();
        for node in invalid.permissions.as_mut().unwrap() {
            if let PermissionNode::GameInspectionRead { scopes } = node {
                scopes.push(InspectionScope::Worlds);
            }
        }
        assert!(invalid.validate().is_err());
        invalid = current;
        invalid
            .report_protocols
            .push(crate::CONFIG_FOLDER_PROTOCOL.into());
        assert!(invalid.validate().is_err());
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
    /// An explicitly granted Mesh frontend does not imply file associations.
    pub fn is_mesh_events_frontend(&self) -> bool {
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
                        PermissionNode::MeshEventsSend { .. }
                            | PermissionNode::MeshEventsReceive { .. }
                    )
                })
    }
    /// A text-report frontend can inspect the two declared scopes needed for
    /// independent diagnosis trials without pretending its Wasm ABI is JSON 0.2.
    fn is_diagnostic_recovery_inspection(&self) -> bool {
        let requested = self.permissions.as_deref().unwrap_or_default();
        self.frontend.is_some()
            && self.api_version == crate::REPORT_API_VERSION
            && self.capabilities == ["report"]
            && self.file_extensions == ["log", "txt"]
            && self.report_protocols.is_empty()
            && [PermissionNode::GameDiagnosticsRead {}, PermissionNode::GameRecoveryRead {}, PermissionNode::InstanceCopyRequest {}, PermissionNode::PrivateStorage {}, PermissionNode::SelectedFileRead { extensions: vec!["log".into(), "txt".into()] }].iter().all(|node| requested.contains(node))
            && requested.iter().any(|node| matches!(node, PermissionNode::GameInspectionRead { scopes } if scopes.len() == 2 && scopes.contains(&crate::InspectionScope::Mods) && scopes.contains(&crate::InspectionScope::Storage)))
    }
    fn inferred_permissions(&self) -> Vec<PermissionNode> {
        let mut nodes = Vec::new();
        if self.frontend.is_some() {
            nodes.push(PermissionNode::UiWebview {});
            for node in self.permissions.as_deref().unwrap_or_default() {
                if matches!(
                    node,
                    PermissionNode::MeshEventsSend { .. }
                        | PermissionNode::MeshEventsReceive { .. }
                ) {
                    nodes.push(node.canonical());
                }
            }
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
                PermissionNode::PrivateStorage {},
                PermissionNode::ClipboardWrite {},
                PermissionNode::TextExport {},
                PermissionNode::GeneratedBlueprintSave {},
                PermissionNode::GameConfigWrite {},
                PermissionNode::StorageCacheClear {},
                PermissionNode::InstanceMoveRequest {},
                PermissionNode::InstanceCopyRequest {},
                PermissionNode::GameRecoveryRead {},
                PermissionNode::GameRecoveryOpenRequest {},
                PermissionNode::PlayManifestExport {},
                PermissionNode::FileInboxImportRequest {},
                PermissionNode::GameKeybindingsRead {},
                PermissionNode::GameKeybindingsWrite {},
                PermissionNode::GameDiagnosticsRead {},
                PermissionNode::AiChat {},
                PermissionNode::AiConfigure {},
                PermissionNode::WorldSnapshotsRead {},
                PermissionNode::WorldSeedRead {},
                PermissionNode::WorldSnapshotsRestoreRequest {},
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
            && !self.is_private_data_report()
            && !self.is_host_workflow_report()
            && !self.is_mesh_events_frontend()
            && !self.is_local_frontend_report()
            && (!self
                .permissions
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|node| matches!(node, PermissionNode::GameInspectionRead { .. }))
                || self.report_protocols == [crate::MRPACK_PAGE_PROTOCOL]
                || self.is_diagnostic_recovery_inspection())
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
        if requested
            .iter()
            .filter(|node| matches!(node, PermissionNode::MeshEventsSend { .. }))
            .count()
            > 1
            || requested
                .iter()
                .filter(|node| matches!(node, PermissionNode::MeshEventsReceive { .. }))
                .count()
                > 1
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Each Mesh direction has one namespace declaration",
            ));
        }
        for node in requested {
            if let PermissionNode::MeshEventsSend { namespaces }
            | PermissionNode::MeshEventsReceive { namespaces } = node
                && (self.frontend.is_none()
                    || namespaces.is_empty()
                    || namespaces.len() > crate::MAX_MESH_NAMESPACES
                    || namespaces
                        .iter()
                        .any(|namespace| !crate::valid_mesh_namespace(namespace)))
            {
                return Err(PluginError::new(
                    "PLUGIN_INVALID_PERMISSIONS",
                    "Mesh events require a frontend and one to eight exact, non-reserved namespaces",
                ));
            }
        }
        let workflow = requested.iter().any(|node| {
            matches!(
                node,
                PermissionNode::GameConfigWrite {}
                    | PermissionNode::StorageCacheClear {}
                    | PermissionNode::InstanceMoveRequest {}
                    | PermissionNode::InstanceCopyRequest {}
                    | PermissionNode::GameRecoveryRead {}
                    | PermissionNode::GameRecoveryOpenRequest {}
                    | PermissionNode::PlayManifestExport {}
                    | PermissionNode::FileInboxImportRequest {}
            )
        });
        if workflow && self.frontend.is_none() {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Maintenance workflows require a frontend and explicit declarations",
            ));
        }
        if requested.contains(&PermissionNode::GameConfigWrite {})
            && !requested.contains(&PermissionNode::GameConfigRead {})
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Configuration writes require configuration read permission",
            ));
        }
        if requested.contains(&PermissionNode::GameRecoveryOpenRequest {})
            && !requested.contains(&PermissionNode::GameRecoveryRead {})
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Recovery requests require recovery history read permission",
            ));
        }
        if requested.iter().any(|node| matches!(node, PermissionNode::StorageCacheClear {} | PermissionNode::InstanceMoveRequest {} | PermissionNode::InstanceCopyRequest {}))
            && !requested.iter().any(|node| matches!(node, PermissionNode::GameInspectionRead { scopes } if scopes.contains(&crate::InspectionScope::Storage))) {
            return Err(PluginError::new("PLUGIN_INVALID_PERMISSIONS", "Storage actions require the existing storage inspection scope"));
        }
        if requested.contains(&PermissionNode::GeneratedBlueprintSave {}) && self.frontend.is_none()
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Generated blueprints require a frontend",
            ));
        }
        if requested.contains(&PermissionNode::TextExport {}) && self.frontend.is_none() {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Text export requires a frontend and an explicit declaration",
            ));
        }
        if requested.contains(&PermissionNode::ClipboardWrite {}) && self.frontend.is_none() {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Clipboard writes require a frontend and an explicit declaration",
            ));
        }
        if requested.contains(&PermissionNode::PrivateStorage {}) && self.frontend.is_none() {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "Private plugin data requires a frontend and an explicit declaration",
            ));
        }
        if (requested.contains(&PermissionNode::AiChat {})
            || requested.contains(&PermissionNode::AiConfigure {}))
            && (self.frontend.is_none() || self.id != "ai-assistant")
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PERMISSIONS",
                "AI chat and profile controls are reserved for the AI Assistant frontend",
            ));
        }
        if requested.contains(&PermissionNode::WorldSeedRead {})
            && (self.frontend.is_none() || !requested.iter().any(|node| matches!(node,
                PermissionNode::GameInspectionRead { scopes } if scopes.contains(&crate::InspectionScope::Worlds)))) {
            return Err(PluginError::new("PLUGIN_INVALID_PERMISSIONS", "Saved world seed reads require a frontend and explicit worlds inspection scope"));
        }
        let mixed_mrpack_inspection = self.report_protocols == [crate::MRPACK_PAGE_PROTOCOL]
            && self.file_extensions == ["json", "mrpack"]
            && requested.contains(&PermissionNode::SelectedFileRead {
                extensions: vec!["json".into(), "mrpack".into()],
            });
        let diagnostic_recovery_inspection = self.is_diagnostic_recovery_inspection();
        if requested.iter().any(|node| matches!(node,PermissionNode::GameInspectionRead { scopes } if scopes.is_empty() || scopes.len()>10)) || (requested.iter().any(|node|matches!(node,PermissionNode::GameInspectionRead { .. })) && !diagnostic_recovery_inspection && (self.frontend.is_none() || self.capabilities != ["report"] || self.api_version != REPORT_CONTEXT_API_VERSION || ((!self.file_extensions.is_empty() && self.file_extensions != ["json"]) || !self.report_protocols.is_empty()) && !mixed_mrpack_inspection)) {
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
        if requested.iter().any(|node| matches!(node, PermissionNode::WorldSnapshotsRead {} | PermissionNode::WorldSnapshotsRestoreRequest {}))
            && (self.frontend.is_none()
                || !requested.iter().any(|node| matches!(node, PermissionNode::GameInspectionRead { scopes } if scopes.contains(&crate::InspectionScope::Worlds)))
                || (requested.contains(&PermissionNode::WorldSnapshotsRestoreRequest {}) && !requested.contains(&PermissionNode::WorldSnapshotsRead {})))
        {
            return Err(PluginError::new("PLUGIN_INVALID_PERMISSIONS", "World snapshot operations require a frontend, worlds inspection scope, and explicit snapshot read permission"));
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
                PermissionNode::MeshEventsSend { namespaces }
                | PermissionNode::MeshEventsReceive { namespaces } => {
                    sorted(namespaces.clone()).len() != namespaces.len()
                }
                _ => false,
            });
            if nodes.len() > 16
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
mod world_snapshot_permission_tests {
    use super::*;
    #[test]
    fn restore_request_needs_explicit_read_and_world_scope() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../examples/world-dashboard/plugin.json"
        ))
        .unwrap();
        manifest.permissions.as_mut().unwrap().retain(|node| {
            !matches!(
                node,
                PermissionNode::WorldSnapshotsRead {}
                    | PermissionNode::WorldSnapshotsRestoreRequest {}
            )
        });
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::WorldSnapshotsRestoreRequest {});
        assert!(manifest.validate_permissions().is_err());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::WorldSnapshotsRead {});
        assert!(manifest.validate_permissions().is_ok());
        let mut previous = manifest.clone();
        previous
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::WorldSnapshotsRestoreRequest {}));
        assert!(manifest.requires_permission_approval(&previous));
        assert!(serde_json::from_value::<PermissionNode>(serde_json::json!({"permission":"launcher:world-snapshots/restore-request","path":"C:/private"})).is_err());
    }
}
#[cfg(test)]
mod mixed_inspection_permission_tests {
    use super::*;
    #[test]
    fn mrpack_report_and_mod_inventory_keep_distinct_explicit_grants() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../examples/modpack-inspector/plugin.json"
        ))
        .unwrap();
        assert!(manifest.validate_permissions().is_ok());
        assert!(!manifest.is_inspection_report());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::SelectedFileRead { .. }));
        assert!(manifest.validate_permissions().is_err());
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::SelectedFileRead {
                extensions: vec!["json".into(), "mrpack".into()],
            });
        manifest.report_protocols.clear();
        assert!(manifest.validate_permissions().is_err());
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
    #[test]
    fn world_seed_is_a_separate_explicit_grant_bound_to_worlds_inspection() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../examples/world-dashboard/plugin.json"
        ))
        .unwrap();
        let old = manifest.clone();
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .push(PermissionNode::WorldSeedRead {});
        manifest.validate().unwrap();
        assert!(
            manifest
                .permission_delta(&old)
                .added
                .contains(&PermissionNode::WorldSeedRead {})
        );
        manifest
            .permissions
            .as_mut()
            .unwrap()
            .retain(|node| !matches!(node, PermissionNode::GameInspectionRead { .. }));
        assert!(manifest.validate().is_err());
        let mut no_frontend = old;
        no_frontend.frontend = None;
        no_frontend.permissions = Some(vec![PermissionNode::WorldSeedRead {}]);
        assert!(no_frontend.validate().is_err());
        assert!(
            serde_json::from_value::<PermissionNode>(
                serde_json::json!({"permission":"launcher:worlds/seed/read","path":"/private"})
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod generated_blueprint_manifest_tests {
    use super::*;
    #[test]
    fn generated_blueprint_manifest_validates_without_implicit_files() {
        let manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../examples/geometry-builder/plugin.json"
        ))
        .unwrap();
        manifest.validate().unwrap();
        assert!(manifest.is_local_frontend_report());
        assert!(
            manifest
                .effective_permissions()
                .contains(&PermissionNode::GeneratedBlueprintSave {})
        );
        assert!(
            !manifest
                .effective_permissions()
                .iter()
                .any(|node| matches!(node, PermissionNode::SelectedFileRead { .. }))
        );
        let mut invalid = manifest;
        invalid.frontend = None;
        assert!(invalid.validate().is_err());
        assert!(serde_json::from_value::<PermissionNode>(serde_json::json!({"permission":"launcher:blueprints/generated-save","path":"../world"})).is_err());
    }
}
