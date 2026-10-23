// SPDX-License-Identifier: MIT
//! Supported standard JSON Schema / JSON Forms subset, not a page language.
use crate::{LocalizedText, PluginError, SettingField, SettingKind, SettingOption, SettingScope};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsForm {
    pub schema_version: u32,
    pub scope: SettingScope,
    pub schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub translations: BTreeMap<String, BTreeMap<String, String>>,
}
fn invalid() -> PluginError {
    PluginError::new(
        "PLUGIN_UNSUPPORTED_SETTINGS_SCHEMA",
        "Unsupported or inconsistent declarative settings schema",
    )
}
fn object(value: &Value) -> Result<&Map<String, Value>, PluginError> {
    value.as_object().ok_or_else(invalid)
}
fn keys(value: &Map<String, Value>, allowed: &[&str]) -> Result<(), PluginError> {
    if value.keys().any(|key| !allowed.contains(&key.as_str())) {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn plain(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}
impl SettingsForm {
    pub fn fields(&self) -> Result<Vec<SettingField>, PluginError> {
        if self.schema_version != 1
            || serde_json::to_vec(self).map_err(|_| invalid())?.len() > 32 * 1024
        {
            return Err(invalid());
        }
        let root = object(&self.schema)?;
        keys(
            root,
            &[
                "$schema",
                "type",
                "additionalProperties",
                "properties",
                "required",
                "title",
            ],
        )?;
        if root
            .get("$schema")
            .is_some_and(|v| v != "https://json-schema.org/draft/2020-12/schema")
            || root.get("type") != Some(&Value::String("object".into()))
            || root.get("additionalProperties") != Some(&Value::Bool(false))
        {
            return Err(invalid());
        }
        if root
            .get("title")
            .is_some_and(|v| !v.as_str().is_some_and(|s| plain(s, 120)))
        {
            return Err(invalid());
        }
        let properties = object(root.get("properties").ok_or_else(invalid)?)?;
        if properties.is_empty() || properties.len() > 64 {
            return Err(invalid());
        }
        let required = root
            .get("required")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        let required: Vec<_> = required
            .iter()
            .map(|value| value.as_str().ok_or_else(invalid))
            .collect::<Result<_, _>>()?;
        let property_keys: BTreeSet<_> = properties.keys().map(String::as_str).collect();
        if required.len() != properties.len()
            || required.iter().copied().collect::<BTreeSet<_>>() != property_keys
        {
            return Err(invalid());
        }
        let mut fields = Vec::new();
        let mut translation_keys = BTreeSet::new();
        for (id, value) in properties {
            if !crate::contributions::local_id(id) {
                return Err(invalid());
            }
            let prop = object(value)?;
            keys(
                prop,
                &[
                    "type",
                    "title",
                    "default",
                    "enum",
                    "minimum",
                    "maximum",
                    "maxLength",
                ],
            )?;
            let kind = match prop.get("type").and_then(Value::as_str) {
                Some("string") if prop.contains_key("enum") => SettingKind::Enum,
                Some("string") => SettingKind::String,
                Some("boolean") => SettingKind::Boolean,
                Some("number") => SettingKind::Number,
                _ => return Err(invalid()),
            };
            let title = prop
                .get("title")
                .map(|v| v.as_str().filter(|s| plain(s, 120)).ok_or_else(invalid))
                .transpose()?
                .unwrap_or(id);
            let label_key = format!("{id}.label");
            translation_keys.insert(label_key.clone());
            let label = self.label(&label_key, title);
            let options = if kind == SettingKind::Enum {
                let values = prop["enum"].as_array().ok_or_else(invalid)?;
                values
                    .iter()
                    .map(|value| {
                        let text = value.as_str().ok_or_else(invalid)?;
                        let key = format!("{id}.{text}");
                        translation_keys.insert(key.clone());
                        Ok(SettingOption {
                            value: value.clone(),
                            label: self.label(&key, text),
                        })
                    })
                    .collect::<Result<Vec<_>, PluginError>>()?
            } else {
                Vec::new()
            };
            if kind != SettingKind::Enum && prop.contains_key("enum") {
                return Err(invalid());
            }
            let number = |key: &str| -> Result<Option<serde_json::Number>, PluginError> {
                prop.get(key)
                    .map(|v| v.as_number().cloned().ok_or_else(invalid))
                    .transpose()
            };
            let max_length = prop
                .get("maxLength")
                .map(|v| {
                    v.as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(invalid)
                })
                .transpose()?;
            let field = SettingField {
                id: id.clone(),
                kind,
                label,
                scope: self.scope,
                advanced: false,
                default: prop.get("default").cloned().ok_or_else(invalid)?,
                options,
                minimum: number("minimum")?,
                maximum: number("maximum")?,
                max_length,
            };
            if kind == SettingKind::Enum
                && (prop.contains_key("minimum")
                    || prop.contains_key("maximum")
                    || prop.contains_key("maxLength"))
            {
                return Err(invalid());
            }
            field.validate().map_err(|_| invalid())?;
            fields.push(field);
        }
        if let Some(ui) = &self.ui_schema {
            let mut seen = BTreeSet::new();
            let mut count = 0;
            validate_ui(
                ui,
                &property_keys,
                &mut seen,
                &mut translation_keys,
                0,
                &mut count,
            )?;
            if seen != property_keys {
                return Err(invalid());
            }
        }
        for (locale, entries) in &self.translations {
            if !matches!(locale.as_str(), "en" | "zh-CN")
                || entries.len() > 256
                || entries.iter().any(|(key, value)| {
                    key.len() > 160 || !translation_keys.contains(key) || !plain(value, 512)
                })
            {
                return Err(invalid());
            }
        }
        Ok(fields)
    }
    fn label(&self, key: &str, fallback: &str) -> LocalizedText {
        LocalizedText {
            en: self
                .translations
                .get("en")
                .and_then(|map| map.get(key))
                .cloned()
                .unwrap_or_else(|| fallback.into()),
            zh_cn: self
                .translations
                .get("zh-CN")
                .and_then(|map| map.get(key))
                .cloned(),
        }
    }
    pub fn validate(&self) -> Result<(), PluginError> {
        self.fields().map(|_| ())
    }
}
fn validate_ui<'a>(
    value: &Value,
    properties: &BTreeSet<&'a str>,
    seen: &mut BTreeSet<&'a str>,
    translations: &mut BTreeSet<String>,
    depth: usize,
    count: &mut usize,
) -> Result<(), PluginError> {
    *count += 1;
    if depth > 4 || *count > 128 {
        return Err(invalid());
    }
    let node = object(value)?;
    match node.get("type").and_then(Value::as_str) {
        Some("Control") => {
            keys(node, &["type", "scope", "label"])?;
            let id = node
                .get("scope")
                .and_then(Value::as_str)
                .and_then(|s| s.strip_prefix("#/properties/"))
                .ok_or_else(invalid)?;
            let id = properties.get(id).copied().ok_or_else(invalid)?;
            if !seen.insert(id) {
                return Err(invalid());
            }
            if node.get("label").is_some_and(|v| {
                v != &Value::Bool(false) && !v.as_str().is_some_and(|s| plain(s, 120))
            }) {
                return Err(invalid());
            }
        }
        Some(kind @ ("VerticalLayout" | "HorizontalLayout" | "Group")) => {
            keys(
                node,
                if kind == "Group" {
                    &["type", "elements", "label"]
                } else {
                    &["type", "elements"]
                },
            )?;
            if let Some(label) = node.get("label") {
                let label = label
                    .as_str()
                    .filter(|s| plain(s, 120))
                    .ok_or_else(invalid)?;
                translations.insert(label.into());
            }
            let elements = node
                .get("elements")
                .and_then(Value::as_array)
                .filter(|v| !v.is_empty())
                .ok_or_else(invalid)?;
            for element in elements {
                validate_ui(element, properties, seen, translations, depth + 1, count)?;
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn form() -> SettingsForm {
        serde_json::from_value(serde_json::json!({"schema_version":1,"scope":"global","schema":{"type":"object","additionalProperties":false,"properties":{"format":{"type":"string","enum":["auto","json"],"default":"auto","title":"Format"},"warn_duplicate_properties":{"type":"boolean","default":true}},"required":["format","warn_duplicate_properties"]},"ui_schema":{"type":"VerticalLayout","elements":[{"type":"Control","scope":"#/properties/format"},{"type":"Control","scope":"#/properties/warn_duplicate_properties"}]},"translations":{"zh-CN":{"format.label":"格式","format.auto":"自动"}}})).unwrap()
    }
    #[test]
    fn settings_form_normalizes_defaults_scopes_and_uses_native_value_validation() {
        let fields = form().fields().unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].id, "format");
        assert_eq!(fields[0].default, "auto");
        assert_eq!(fields[0].scope, SettingScope::Global);
        assert_eq!(fields[0].label.zh_cn.as_deref(), Some("格式"));
        assert!(
            fields[0]
                .validate_value(&Value::String("unknown".into()))
                .is_err()
        );
        let manifest: crate::PluginManifest =
            serde_json::from_str(include_str!("../../../examples/config-doctor/plugin.json"))
                .unwrap();
        manifest.validate().unwrap();
        let contributions = manifest.contributions.as_ref().unwrap();
        let defaults = contributions.default_settings();
        assert_eq!(defaults["format"], "auto");
        assert_eq!(defaults["warn_duplicate_properties"], true);
        contributions.validate_settings(&defaults).unwrap();
        let mut bad = defaults;
        bad.insert(
            "warn_duplicate_properties".into(),
            Value::String("true".into()),
        );
        assert!(contributions.validate_settings(&bad).is_err());
        assert!(
            manifest
                .effective_permissions()
                .contains(&crate::PermissionNode::SettingsRead {
                    keys: vec![
                        "global:format".into(),
                        "global:warn_duplicate_properties".into()
                    ]
                })
        );
    }
    #[test]
    fn settings_form_rejects_unsupported_schema_controls_and_translation_keys() {
        for keyword in ["$ref", "allOf", "if", "patternProperties"] {
            let mut f = form();
            f.schema[keyword] = Value::String("unsupported".into());
            assert!(f.validate().is_err(), "{keyword}");
        }
        let mut f = form();
        f.schema["properties"]["format"]["type"] = "array".into();
        assert!(f.validate().is_err());
        let mut f = form();
        f.ui_schema.as_mut().unwrap()["elements"][0]["scope"] = "#/properties/secret".into();
        assert!(f.validate().is_err());
        let mut f = form();
        f.ui_schema.as_mut().unwrap()["elements"][0]["options"] =
            serde_json::json!({"eval":"script"});
        assert!(f.validate().is_err());
        let mut f = form();
        f.translations
            .get_mut("zh-CN")
            .unwrap()
            .insert("unknown.expression".into(), "不允许".into());
        assert!(f.validate().is_err());
    }
}
