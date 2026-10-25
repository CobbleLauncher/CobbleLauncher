// SPDX-License-Identifier: MIT
//! Capability discovery metadata, independent of execution permissions.
use crate::{
    API_VERSION, PROVIDER_API_VERSION, PluginError, PluginManifest, REPORT_API_VERSION,
    REPORT_CONTEXT_API_VERSION,
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
pub const MESSAGE_COMPOSER_INTERFACE: &str = "launcher:message-composer";
pub const VIEWER_INTERFACE: &str = "launcher:viewer";
pub const REPORT_INTERFACE: &str = "launcher:report";
pub const BLUEPRINT_PROVIDER_INTERFACE: &str = "launcher:blueprint-provider";
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interfaces {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provides: Vec<ProvidedInterface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<InterfaceRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional: Vec<InterfaceRequirement>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvidedInterface {
    pub id: String,
    pub version: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceRequirement {
    pub id: String,
    /// A SemVer requirement, for example ^0.2.0. This never implies WIT compatibility.
    pub version: String,
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.contains(':')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-:./".contains(&b))
}
fn invalid() -> PluginError {
    PluginError::new("PLUGIN_INVALID_PACKAGE", "Invalid interface declaration")
}
impl Interfaces {
    pub fn is_empty(&self) -> bool {
        self.provides.is_empty() && self.requires.is_empty() && self.optional.is_empty()
    }
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.provides.len() + self.requires.len() + self.optional.len() > 16 {
            return Err(invalid());
        }
        let mut provides = HashSet::new();
        for item in &self.provides {
            if !valid_id(&item.id)
                || item.version.len() > 64
                || Version::parse(&item.version).is_err()
                || !provides.insert(&item.id)
            {
                return Err(invalid());
            }
        }
        let mut dependencies = HashSet::new();
        for item in self.requires.iter().chain(&self.optional) {
            if !valid_id(&item.id)
                || item.version.is_empty()
                || item.version.len() > 128
                || VersionReq::parse(&item.version).is_err()
                || !dependencies.insert(&item.id)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
impl InterfaceRequirement {
    pub fn viewer() -> Self {
        Self {
            id: VIEWER_INTERFACE.into(),
            version: "^1.0.0".into(),
        }
    }
    pub fn blueprint_provider() -> Self {
        Self {
            id: BLUEPRINT_PROVIDER_INTERFACE.into(),
            version: "^0.2.0".into(),
        }
    }
    pub fn matches(&self, provided: &ProvidedInterface) -> bool {
        self.id == provided.id
            && VersionReq::parse(&self.version)
                .ok()
                .zip(Version::parse(&provided.version).ok())
                .is_some_and(|(range, version)| range.matches(&version))
    }
    pub fn validate(&self) -> Result<(), PluginError> {
        Interfaces {
            requires: vec![self.clone()],
            ..Default::default()
        }
        .validate()
    }
}
impl PluginManifest {
    /// The exact interfaces whose execution adapters this host implements.
    fn implemented_interface(&self) -> Option<ProvidedInterface> {
        let id = match (self.api_version.as_str(), self.capabilities.as_slice()) {
            (API_VERSION, [capability]) if capability == "viewer" => VIEWER_INTERFACE,
            (REPORT_API_VERSION | REPORT_CONTEXT_API_VERSION, [capability])
                if capability == "report" =>
            {
                REPORT_INTERFACE
            }
            (PROVIDER_API_VERSION, [capability]) if capability == "blueprint-provider" => {
                BLUEPRINT_PROVIDER_INTERFACE
            }
            (crate::MESSAGE_COMPOSER_API_VERSION, [capability])
                if capability == "message-composer" =>
            {
                MESSAGE_COMPOSER_INTERFACE
            }
            (crate::IMAGE_LAYOUT_API_VERSION, [capability]) if capability == "image-layout" => {
                crate::IMAGE_LAYOUT_INTERFACE
            }
            (crate::TOOL_FORM_API_VERSION, [capability]) if capability == "tool-form" => {
                crate::TOOL_FORM_INTERFACE
            }
            _ => return None,
        };
        Some(ProvidedInterface {
            id: id.into(),
            version: self.api_version.clone(),
        })
    }
    /// Legacy packages retain their already implemented ABI without reinstalling.
    pub fn provided_interfaces(&self) -> Vec<ProvidedInterface> {
        let mut interfaces = self.interfaces.provides.clone();
        if let Some(implemented) = self.implemented_interface()
            && !interfaces.iter().any(|i| i.id == implemented.id)
        {
            interfaces.push(implemented);
        }
        if let Some(frontend) = &self.frontend
            && !interfaces
                .iter()
                .any(|i| i.id == crate::WEB_FRONTEND_INTERFACE)
        {
            interfaces.push(ProvidedInterface {
                id: crate::WEB_FRONTEND_INTERFACE.into(),
                version: frontend.api_version.clone(),
            });
        }
        interfaces
    }
    /// SemVer matching is necessary, but never creates a new executable WIT ABI.
    pub fn supports_interface(&self, requirement: &InterfaceRequirement) -> bool {
        self.validate().is_ok()
            && (self
                .implemented_interface()
                .is_some_and(|implemented| requirement.matches(&implemented))
                || self.frontend.as_ref().is_some_and(|frontend| {
                    requirement.matches(&ProvidedInterface {
                        id: crate::WEB_FRONTEND_INTERFACE.into(),
                        version: frontend.api_version.clone(),
                    })
                }))
    }
    pub(crate) fn validate_interface_abi(&self) -> Result<(), PluginError> {
        let implemented = self.implemented_interface();
        if self.interfaces.provides.iter().any(|provided| {
            provided.id == crate::WEB_FRONTEND_INTERFACE
                && self
                    .frontend
                    .as_ref()
                    .is_none_or(|frontend| provided.version != frontend.api_version)
        }) {
            return Err(PluginError::new(
                "PLUGIN_UNSUPPORTED_API",
                "Declared frontend interface does not match the frontend facet",
            ));
        }
        if self.interfaces.provides.iter().any(|provided| {
            matches!(
                provided.id.as_str(),
                VIEWER_INTERFACE
                    | REPORT_INTERFACE
                    | BLUEPRINT_PROVIDER_INTERFACE
                    | MESSAGE_COMPOSER_INTERFACE
                    | crate::IMAGE_LAYOUT_INTERFACE
                    | crate::TOOL_FORM_INTERFACE
            ) && implemented.as_ref() != Some(provided)
        }) {
            return Err(PluginError::new(
                "PLUGIN_UNSUPPORTED_API",
                "Declared interface does not match a supported WIT ABI",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest(capability: &str, version: &str) -> PluginManifest {
        serde_json::from_value(serde_json::json!({"id":"fixture","name":"Fixture","version":"1.0.0","publisher":"Original fixture","api_version":version,"capabilities":[capability],"file_extensions":if capability=="report"{vec!["log"]}else{vec!["schem"]},"component_sha256":"a".repeat(64)})).unwrap()
    }
    #[test]
    fn viewer_and_both_report_abis_infer_only_their_exact_implemented_interface() {
        for (capability, version, id) in [
            ("viewer", "1.0.0", VIEWER_INTERFACE),
            ("report", "0.1.0", REPORT_INTERFACE),
            ("report", "0.2.0", REPORT_INTERFACE),
        ] {
            let mut manifest = manifest(capability, version);
            manifest.validate().unwrap();
            assert_eq!(
                manifest.provided_interfaces(),
                vec![ProvidedInterface {
                    id: id.into(),
                    version: version.into()
                }]
            );
            assert!(manifest.supports_interface(&InterfaceRequirement {
                id: id.into(),
                version: format!("={version}")
            }));
            manifest.interfaces.provides = vec![ProvidedInterface {
                id: id.into(),
                version: version.into(),
            }];
            assert_eq!(
                manifest.provided_interfaces().len(),
                1,
                "explicit declaration is not duplicated"
            );
            manifest.validate().unwrap();
            manifest.interfaces.provides[0].version = "9.0.0".into();
            assert!(
                manifest.validate().is_err(),
                "a declaration cannot invent a supported WIT ABI"
            );
        }
        assert!(
            !manifest("report", "0.2.0").supports_interface(&InterfaceRequirement {
                id: REPORT_INTERFACE.into(),
                version: "^0.1.0".into()
            })
        );
        assert!(!manifest("report", "0.1.0").supports_interface(&InterfaceRequirement::viewer()));
        let mut unknown = manifest("viewer", "1.0.0");
        unknown.interfaces.provides = vec![ProvidedInterface {
            id: "community:unknown".into(),
            version: "1.0.0".into(),
        }];
        unknown.validate().unwrap();
        assert!(!unknown.supports_interface(&InterfaceRequirement {
            id: "community:unknown".into(),
            version: "^1.0.0".into()
        }));
    }
    #[test]
    fn semver_ranges_are_metadata_not_abi_guesses() {
        let req = InterfaceRequirement::blueprint_provider();
        assert!(req.matches(&ProvidedInterface {
            id: req.id.clone(),
            version: "0.2.1".into()
        }));
        assert!(!req.matches(&ProvidedInterface {
            id: req.id.clone(),
            version: "0.3.0".into()
        }));
        assert!(
            InterfaceRequirement {
                id: req.id.clone(),
                version: "bogus".into()
            }
            .validate()
            .is_err()
        );
        assert!(
            Interfaces {
                requires: vec![req.clone()],
                optional: vec![req],
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
