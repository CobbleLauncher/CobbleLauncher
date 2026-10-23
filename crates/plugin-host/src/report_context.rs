// SPDX-License-Identifier: MIT
//! Immutable, declared settings and locale copied into one isolated invocation.
use cobble_plugin_api::{PluginError, PluginManifest};
use serde::Serialize;
use std::collections::BTreeMap;

pub const MAX_CONTEXT_BYTES: usize = 16 * 1024;
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReportHostContext {
    locale: String,
    settings: BTreeMap<String, serde_json::Value>,
}
impl ReportHostContext {
    pub fn locale(&self) -> &str {
        &self.locale
    }
    pub fn settings_json(&self) -> Result<String, PluginError> {
        serde_json::to_string(&self.settings).map_err(|_| invalid())
    }
    pub(crate) fn validate(&self) -> Result<(), PluginError> {
        if !matches!(self.locale.as_str(), "en" | "zh-CN")
            || serde_json::to_vec(self).map_err(|_| invalid())?.len() > MAX_CONTEXT_BYTES
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn new(
        manifest: &PluginManifest,
        locale: &str,
        settings: BTreeMap<String, serde_json::Value>,
    ) -> Result<Self, PluginError> {
        manifest.validate()?;
        if manifest.api_version != cobble_plugin_api::REPORT_CONTEXT_API_VERSION {
            return Err(invalid());
        }
        let mut effective = manifest
            .contributions
            .as_ref()
            .map(|value| value.default_settings())
            .unwrap_or_default();
        for (id, value) in settings {
            let field = manifest
                .contributions
                .as_ref()
                .and_then(|contributions| contributions.setting(&id))
                .ok_or_else(invalid)?;
            field.validate_value(&value)?;
            effective.insert(id, value);
        }
        let context = Self {
            locale: locale.into(),
            settings: effective,
        };
        context.validate()?;
        Ok(context)
    }
}
fn invalid() -> PluginError {
    PluginError::new("PLUGIN_INVALID_CONTEXT", "Invalid report context")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> PluginManifest {
        serde_json::from_str(include_str!(
            "../../../examples/modpack-inspector/plugin.json"
        ))
        .unwrap()
    }
    #[test]
    fn snapshot_validates_declared_types_defaults_locale_and_abi() {
        let manifest = manifest();
        let defaults = ReportHostContext::new(&manifest, "en", BTreeMap::new()).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&defaults.settings_json().unwrap()).unwrap();
        assert_eq!(value["page_size"], 50);
        assert_eq!(value["show_dependencies"], true);
        let custom = ReportHostContext::new(
            &manifest,
            "zh-CN",
            BTreeMap::from([("show_dependencies".into(), serde_json::json!(false))]),
        )
        .unwrap();
        assert_eq!(custom.locale(), "zh-CN");
        assert!(custom.settings_json().unwrap().contains("false"));
        assert!(ReportHostContext::new(&manifest, "fr", BTreeMap::new()).is_err());
        assert!(
            ReportHostContext::new(
                &manifest,
                "en",
                BTreeMap::from([("undeclared".into(), serde_json::json!(true))])
            )
            .is_err()
        );
        assert!(
            ReportHostContext::new(
                &manifest,
                "en",
                BTreeMap::from([("show_dependencies".into(), serde_json::json!("false"))])
            )
            .is_err()
        );
        assert!(
            ReportHostContext::new(
                &manifest,
                "en",
                BTreeMap::from([("page_size".into(), serde_json::json!(51))])
            )
            .is_err()
        );
        let mut legacy = manifest;
        legacy.api_version = cobble_plugin_api::REPORT_API_VERSION.into();
        assert!(ReportHostContext::new(&legacy, "en", BTreeMap::new()).is_err());
    }
}
