// SPDX-License-Identifier: MIT
//! Transient host forms and import-free computation; never persisted as settings.
use crate::{PluginError, PluginManifest, SettingField, SettingScope, SettingsForm};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const TOOL_FORM_API_VERSION: &str = "0.1.0";
pub const TOOL_FORM_INTERFACE: &str = "launcher:tool-form";
pub const MAX_TOOL_FORM_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolForm {
    pub schema_version: u32,
    pub schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub translations: BTreeMap<String, BTreeMap<String, String>>,
}
impl ToolForm {
    pub fn fields(&self) -> Result<Vec<SettingField>, PluginError> {
        SettingsForm {
            schema_version: self.schema_version,
            scope: SettingScope::Global,
            schema: self.schema.clone(),
            ui_schema: self.ui_schema.clone(),
            translations: self.translations.clone(),
        }
        .fields()
    }
    pub fn validate(&self) -> Result<(), PluginError> {
        self.fields().map(|_| ())
    }
    pub fn validate_input(&self, input: &Value) -> Result<(), PluginError> {
        let fields = self.fields()?;
        let values = input.as_object().ok_or_else(invalid)?;
        if values.len() != fields.len()
            || fields.iter().any(|field| {
                values
                    .get(&field.id)
                    .is_none_or(|value| field.validate_value(value).is_err())
            })
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolFormRequest {
    pub locale: String,
    pub input: Value,
}
impl ToolFormRequest {
    pub fn validate(&self, manifest: &PluginManifest) -> Result<(), PluginError> {
        manifest.validate()?;
        if manifest.capabilities != ["tool-form"] || manifest.api_version != TOOL_FORM_API_VERSION {
            return Err(PluginError::new(
                "PLUGIN_UNSUPPORTED_API",
                "Expected tool-form interface",
            ));
        }
        self.validate_envelope()?;
        manifest
            .tool_form
            .as_ref()
            .ok_or_else(invalid)?
            .validate_input(&self.input)
    }
    pub fn validate_envelope(&self) -> Result<(), PluginError> {
        if !matches!(self.locale.as_str(), "en" | "zh-CN")
            || serde_json::to_vec(self).map_err(|_| invalid())?.len() > MAX_TOOL_FORM_BYTES
            || !self.input.is_object()
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolTextBlock {
    pub label: Option<String>,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolFormOutput {
    pub title: String,
    pub blocks: Vec<ToolTextBlock>,
}
fn plain(text: &str, max: usize, multiline: bool) -> bool {
    !text.trim().is_empty()
        && text.len() <= max
        && !text
            .chars()
            .any(|c| c.is_control() && !(multiline && c == '\n'))
}
fn invalid() -> PluginError {
    PluginError::new(
        "PLUGIN_INVALID_TOOL_FORM",
        "Invalid or oversized tool form data",
    )
}
/// Guest validation feedback is untrusted text, never a host error or action.
pub fn tool_form_guest_failure(code: &str, message: &str) -> PluginError {
    if code.is_empty()
        || code.len() > 64
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        || !plain(message, 512, false)
    {
        return invalid();
    }
    PluginError::new("PLUGIN_TOOL_INPUT", message)
}
impl ToolFormOutput {
    pub fn validate(&self) -> Result<(), PluginError> {
        if !plain(&self.title, 256, false)
            || self.blocks.is_empty()
            || self.blocks.len() > 16
            || self.blocks.iter().any(|block| {
                !plain(&block.text, MAX_TOOL_FORM_BYTES, true)
                    || block
                        .label
                        .as_ref()
                        .is_some_and(|label| !plain(label, 256, false))
            })
            || serde_json::to_vec(self).map_err(|_| invalid())?.len() > MAX_TOOL_FORM_BYTES
        {
            return Err(invalid());
        }
        Ok(())
    }
}
impl PluginManifest {
    pub(crate) fn validate_tool_form(&self) -> Result<(), PluginError> {
        let tool = self.api_version == TOOL_FORM_API_VERSION && self.capabilities == ["tool-form"];
        if tool != self.tool_form.is_some() || (tool && self.frontend.is_some()) {
            return Err(invalid());
        }
        if let Some(form) = &self.tool_form {
            form.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    pub fn manifest() -> PluginManifest {
        serde_json::from_value(json!({"id":"tool-fixture","name":"Tool fixture","version":"1.0.0","publisher":"Original fixture","api_version":"0.1.0","capabilities":["tool-form"],"file_extensions":[],"permissions":[],"component_sha256":"a".repeat(64),"tool_form":{"schema_version":1,"schema":{"type":"object","additionalProperties":false,"properties":{"target":{"type":"string","default":"@s","maxLength":16},"count":{"type":"number","default":1,"minimum":1,"maximum":64}},"required":["target","count"]}}})).unwrap()
    }
    #[test]
    fn tool_form_validates_exact_values_without_implicit_grants() {
        let manifest = manifest();
        manifest.validate().unwrap();
        assert!(manifest.effective_permissions().is_empty());
        assert_eq!(manifest.provided_interfaces()[0].id, TOOL_FORM_INTERFACE);
        for input in [
            json!({"target":"@s","count":1}),
            json!({"target":"玩家","count":64}),
        ] {
            ToolFormRequest {
                locale: "en".into(),
                input,
            }
            .validate(&manifest)
            .unwrap();
        }
        for input in [
            json!({"target":"@s","count":0}),
            json!({"target":"@s","count":"1"}),
            json!({"target":"@s"}),
            json!({"target":"@s","count":1,"path":"/private"}),
            json!({"target":"bad\nname","count":1}),
        ] {
            assert!(
                ToolFormRequest {
                    locale: "en".into(),
                    input
                }
                .validate(&manifest)
                .is_err()
            );
        }
        let mut wrong = manifest.clone();
        wrong.capabilities = vec!["report".into()];
        assert!(wrong.validate().is_err());
        let mut wrong = manifest;
        wrong.permissions = Some(vec![crate::PermissionNode::SelectedFileRead {
            extensions: vec!["json".into()],
        }]);
        assert!(wrong.validate().is_err());
    }
    #[test]
    fn tool_form_output_is_bounded_plain_text_only() {
        let output = ToolFormOutput {
            title: "Command".into(),
            blocks: vec![ToolTextBlock {
                label: None,
                text: "/give @s minecraft:stone 1".into(),
            }],
        };
        output.validate().unwrap();
        let mut bad = output.clone();
        bad.blocks[0].text = "bad\u{1b}command".into();
        assert!(bad.validate().is_err());
        let mut bad = output.clone();
        bad.blocks = vec![output.blocks[0].clone(); 17];
        assert!(bad.validate().is_err());
        let mut bad = output;
        bad.blocks[0].text = "x".repeat(MAX_TOOL_FORM_BYTES);
        assert!(bad.validate().is_err());
        assert!(
            serde_json::from_value::<ToolFormOutput>(
                json!({"title":"Command","blocks":[],"exec":"danger"})
            )
            .is_err()
        );
    }
    #[test]
    fn saved_command_form_defaults_and_conditions_match_the_host_contract() {
        let manifest: PluginManifest =
            serde_json::from_str(include_str!("../fixtures/command-form.json"))
                .unwrap();
        manifest.validate().unwrap();
        let form = manifest.tool_form.as_ref().unwrap();
        let values: serde_json::Map<String, Value> = form
            .fields()
            .unwrap()
            .into_iter()
            .map(|field| (field.id, field.default))
            .collect();
        assert_eq!(values.len(), 12);
        for template in ["teleport", "give", "effect"] {
            let mut input = values.clone();
            input.insert("template".into(), Value::String(template.into()));
            for locale in ["en", "zh-CN"] {
                ToolFormRequest {
                    locale: locale.into(),
                    input: Value::Object(input.clone()),
                }
                .validate(&manifest)
                .unwrap();
            }
        }
        assert!(manifest.effective_permissions().is_empty());
    }
    #[test]
    fn guest_validation_feedback_is_bounded_text_with_a_host_owned_error_code() {
        let error = tool_form_guest_failure("INVALID_COORDINATE", "Enter a finite coordinate.");
        assert_eq!(error.code, "PLUGIN_TOOL_INPUT");
        assert_eq!(error.message, "Enter a finite coordinate.");
        for (code, message) in [
            ("HOST_SUCCESS", "bad\nmessage"),
            ("INVALID", ""),
            ("exec://action", "hello"),
        ] {
            assert_eq!(
                tool_form_guest_failure(code, message).code,
                "PLUGIN_INVALID_TOOL_FORM"
            );
        }
        assert_eq!(
            tool_form_guest_failure("INVALID", &"x".repeat(513)).code,
            "PLUGIN_INVALID_TOOL_FORM"
        );
    }
}
