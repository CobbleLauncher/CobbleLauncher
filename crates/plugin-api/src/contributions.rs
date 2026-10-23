// SPDX-License-Identifier: MIT
//! Declarative host-rendered contributions. Visibility never grants permission.
use crate::{PluginError, PluginManifest};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalizedText {
    pub en: String,
    #[serde(default, rename = "zh-CN", skip_serializing_if = "Option::is_none")]
    pub zh_cn: Option<String>,
}
impl LocalizedText {
    pub fn resolve(&self, locale: &str) -> &str {
        if matches!(locale, "zh" | "zh-CN") {
            self.zh_cn.as_deref().unwrap_or(&self.en)
        } else {
            &self.en
        }
    }
    fn valid(&self) -> bool {
        std::iter::once(self.en.as_str())
            .chain(self.zh_cn.as_deref())
            .all(|text| {
                !text.trim().is_empty()
                    && text.len() <= 512
                    && text.chars().count() <= 120
                    && !text.chars().any(char::is_control)
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Workflow {
    Report,
    Viewer,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuPosition {
    Tools,
    Instance,
    Blueprint,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCondition {
    #[default]
    Always,
    InstanceSelected,
    InstanceStopped,
    BlueprintSelected,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuContribution {
    pub position: MenuPosition,
    #[serde(default)]
    pub when: ContextCondition,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandContribution {
    pub id: String,
    pub label: LocalizedText,
    pub workflow: Workflow,
    pub menus: Vec<MenuContribution>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingScope {
    Global,
    Instance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingKind {
    Boolean,
    Enum,
    String,
    Number,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingOption {
    pub value: Value,
    pub label: LocalizedText,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingField {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: SettingKind,
    pub label: LocalizedText,
    pub scope: SettingScope,
    #[serde(default)]
    pub advanced: bool,
    pub default: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<SettingOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
}
impl SettingField {
    /// Validate one value against its declared type/range. No coercion is performed.
    pub fn validate_value(&self, value: &Value) -> Result<(), PluginError> {
        let valid = match self.kind {
            SettingKind::Boolean => value.is_boolean(),
            SettingKind::Enum => self.options.iter().any(|option| option.value == *value),
            SettingKind::String => value.as_str().is_some_and(|text| {
                text.len() <= 4096
                    && text.chars().count() <= self.max_length.unwrap_or(256) as usize
                    && !text.chars().any(char::is_control)
            }),
            SettingKind::Number => value.as_f64().is_some_and(|number| {
                safe_number(number)
                    && self
                        .minimum
                        .as_ref()
                        .and_then(Number::as_f64)
                        .is_none_or(|minimum| number >= minimum)
                    && self
                        .maximum
                        .as_ref()
                        .and_then(Number::as_f64)
                        .is_none_or(|maximum| number <= maximum)
            }),
        };
        if valid {
            Ok(())
        } else {
            Err(invalid("Setting value does not match its declaration"))
        }
    }
    pub(crate) fn validate(&self) -> Result<(), PluginError> {
        if !local_id(&self.id) || !self.label.valid() {
            return Err(invalid("Invalid setting identity or label"));
        }
        if self.kind != SettingKind::Enum && !self.options.is_empty()
            || self.kind != SettingKind::Number
                && (self.minimum.is_some() || self.maximum.is_some())
            || self.kind != SettingKind::String && self.max_length.is_some()
            || self
                .max_length
                .is_some_and(|length| !(1..=1024).contains(&length))
        {
            return Err(invalid("Setting constraints do not match its type"));
        }
        if self.kind == SettingKind::Enum {
            if self.options.is_empty() || self.options.len() > 32 {
                return Err(invalid("Enum settings need 1–32 choices"));
            }
            let mut values = BTreeSet::new();
            for option in &self.options {
                let scalar = match &option.value {
                    Value::Bool(_) => true,
                    Value::String(text) => {
                        text.len() <= 1024 && !text.chars().any(char::is_control)
                    }
                    Value::Number(number) => number.as_f64().is_some_and(safe_number),
                    _ => false,
                };
                if !scalar || !option.label.valid() || !values.insert(option.value.to_string()) {
                    return Err(invalid("Invalid or duplicate setting choice"));
                }
            }
        }
        for bound in self.minimum.iter().chain(self.maximum.iter()) {
            if !bound.as_f64().is_some_and(safe_number) {
                return Err(invalid("Invalid numeric setting bound"));
            }
        }
        if let (Some(minimum), Some(maximum)) = (&self.minimum, &self.maximum)
            && minimum.as_f64() > maximum.as_f64()
        {
            return Err(invalid("Setting minimum exceeds maximum"));
        }
        self.validate_value(&self.default)
    }
}
fn safe_number(value: f64) -> bool {
    value.is_finite() && value.abs() <= 1_000_000_000_000.0
}
pub(crate) fn local_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}
fn invalid(message: &str) -> PluginError {
    PluginError::new("PLUGIN_INVALID_CONTRIBUTIONS", message)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingCategory {
    pub id: String,
    pub label: LocalizedText,
    pub fields: Vec<SettingField>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsContribution {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<SettingCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<crate::SettingsForm>,
}
impl SettingsContribution {
    /// Call after manifest validation; invalid schemas never become executable settings.
    pub fn fields(&self) -> Vec<SettingField> {
        match &self.form {
            Some(form) => form.fields().unwrap_or_default(),
            None => self
                .categories
                .iter()
                .flat_map(|category| category.fields.clone())
                .collect(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contributions {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandContribution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<SettingsContribution>,
}
impl Contributions {
    pub fn setting(&self, id: &str) -> Option<SettingField> {
        self.settings
            .as_ref()?
            .fields()
            .into_iter()
            .find(|field| field.id == id)
    }
    pub fn default_settings(&self) -> BTreeMap<String, Value> {
        self.settings
            .iter()
            .flat_map(SettingsContribution::fields)
            .map(|field| (field.id.clone(), field.default.clone()))
            .collect()
    }
    pub fn validate_settings(&self, values: &BTreeMap<String, Value>) -> Result<(), PluginError> {
        if let Some(form) = self
            .settings
            .as_ref()
            .and_then(|settings| settings.form.as_ref())
        {
            form.validate()?;
        }
        let fields = self
            .settings
            .as_ref()
            .map(SettingsContribution::fields)
            .unwrap_or_default();
        if values.len() != fields.len()
            || fields.iter().any(|field| {
                values
                    .get(&field.id)
                    .is_none_or(|value| field.validate_value(value).is_err())
            })
        {
            return Err(invalid(
                "Settings must contain exactly the declared fields and valid values",
            ));
        }
        Ok(())
    }
    pub fn validate(&self, manifest: &PluginManifest) -> Result<(), PluginError> {
        if self.schema_version != 1 || self.commands.len() > 16 {
            return Err(invalid("Unsupported contribution schema or command count"));
        }
        let prefix = format!("{}.", manifest.id);
        let mut commands = BTreeSet::new();
        for command in &self.commands {
            let expected = match command.workflow {
                Workflow::Report => "report",
                Workflow::Viewer => "viewer",
            };
            if !command.id.strip_prefix(&prefix).is_some_and(local_id)
                || !commands.insert(&command.id)
                || !command.label.valid()
                || manifest.capabilities != [expected]
                || command.menus.is_empty()
                || command.menus.len() > 3
            {
                return Err(invalid("Invalid command, workflow or namespace"));
            }
            for (index, menu) in command.menus.iter().enumerate() {
                if command.menus[..index]
                    .iter()
                    .any(|other| other.position == menu.position)
                {
                    return Err(invalid("Duplicate command menu position"));
                }
            }
        }
        if let Some(settings) = &self.settings {
            if let Some(form) = &settings.form {
                if !settings.categories.is_empty() {
                    return Err(invalid(
                        "Use either legacy categories or a declarative form",
                    ));
                }
                return form.validate();
            }
            if settings.categories.is_empty() || settings.categories.len() > 8 {
                return Err(invalid("Settings need 1–8 categories"));
            }
            let mut categories = BTreeSet::new();
            let mut fields = BTreeSet::new();
            for category in &settings.categories {
                if !local_id(&category.id)
                    || !categories.insert(&category.id)
                    || !category.label.valid()
                    || category.fields.is_empty()
                {
                    return Err(invalid("Invalid setting category"));
                }
                for field in &category.fields {
                    if !fields.insert(&field.id) || fields.len() > 64 {
                        return Err(invalid("Duplicate setting ID or too many settings"));
                    }
                    field.validate()?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn manifest() -> PluginManifest {
        serde_json::from_value(json!({"id":"example","name":"Example","version":"0.1.0","api_version":"0.2.0","publisher":"Fixture","capabilities":["report"],"file_extensions":["json"],"component_sha256":"a".repeat(64),
            "contributions":{"schema_version":1,"commands":[{"id":"example.inspect","label":{"en":"Inspect","zh-CN":"检查"},"workflow":"report","menus":[{"position":"tools"}]}],"settings":{"categories":[{"id":"general","label":{"en":"General"},"fields":[
                {"id":"page_size","type":"enum","label":{"en":"Rows"},"scope":"global","default":50,"options":[{"value":20,"label":{"en":"20"}},{"value":50,"label":{"en":"50"}}]},
                {"id":"enabled","type":"boolean","label":{"en":"Enabled"},"scope":"instance","advanced":true,"default":true},
                {"id":"name","type":"string","label":{"en":"Name"},"scope":"global","default":"a","max_length":3},
                {"id":"count","type":"number","label":{"en":"Count"},"scope":"global","default":2,"minimum":1,"maximum":3}
            ]}]}}})).unwrap()
    }
    #[test]
    fn valid_contributions_bind_workflow_and_localize_without_execution() {
        let manifest = manifest();
        manifest.validate().unwrap();
        let contributions = manifest.contributions.as_ref().unwrap();
        assert_eq!(contributions.commands[0].label.resolve("zh-CN"), "检查");
        assert_eq!(contributions.commands[0].label.resolve("fr"), "Inspect");
        assert_eq!(contributions.default_settings()["page_size"], json!(50));
        assert!(contributions.setting("enabled").unwrap().advanced);
        assert!(contributions.setting("unknown").is_none());
    }
    #[test]
    fn settings_values_are_typed_and_ranges_are_enforced() {
        let manifest = manifest();
        let contributions = manifest.contributions.unwrap();
        for (id, good, bad) in [
            ("page_size", json!(20), json!(100)),
            ("enabled", json!(false), json!("false")),
            ("name", json!("配置"), json!("four")),
            ("count", json!(3), json!(4)),
        ] {
            let field = contributions.setting(id).unwrap();
            assert!(field.validate_value(&good).is_ok());
            assert!(field.validate_value(&bad).is_err());
        }
        assert!(
            contributions
                .setting("count")
                .unwrap()
                .validate_value(&json!(1e20))
                .is_err()
        );
    }
    #[test]
    fn rejects_namespace_workflow_defaults_duplicates_and_unknown_fields() {
        let original = serde_json::to_value(manifest()).unwrap();
        for (pointer, value) in [
            ("/contributions/schema_version", json!(2)),
            ("/contributions/commands/0/id", json!("other.inspect")),
            ("/contributions/commands/0/workflow", json!("viewer")),
            (
                "/contributions/settings/categories/0/fields/0/default",
                json!(100),
            ),
            (
                "/contributions/settings/categories/0/fields/1/id",
                json!("page_size"),
            ),
            (
                "/contributions/settings/categories/0/fields/3/minimum",
                json!(4),
            ),
        ] {
            let mut value_manifest = original.clone();
            *value_manifest.pointer_mut(pointer).unwrap() = value;
            let manifest: PluginManifest = serde_json::from_value(value_manifest).unwrap();
            assert!(manifest.validate().is_err(), "{pointer}");
        }
        let mut unknown = original;
        unknown["contributions"]["commands"][0]["script"] = json!("exec");
        assert!(serde_json::from_value::<PluginManifest>(unknown).is_err());
    }
    #[test]
    fn legacy_manifests_omit_optional_contributions_on_roundtrip() {
        let mut value = serde_json::to_value(manifest()).unwrap();
        value.as_object_mut().unwrap().remove("contributions");
        value["api_version"] = json!("0.1.0");
        let manifest: PluginManifest = serde_json::from_value(value).unwrap();
        manifest.validate().unwrap();
        assert!(manifest.contributions.is_none());
        assert!(
            serde_json::to_value(manifest)
                .unwrap()
                .get("contributions")
                .is_none()
        );
    }
}
