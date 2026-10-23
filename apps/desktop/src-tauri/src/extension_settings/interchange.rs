// SPDX-License-Identifier: MIT
//! YAML is bounded interchange only; JSON persistence and capability grants stay unchanged.
use super::*;

const MAX_YAML_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    plugin_id: String,
    scope: SettingScope,
    values: BTreeMap<String, Value>,
}

#[derive(Serialize)]
pub struct ImportPreview {
    values: BTreeMap<String, Value>,
}

pub(super) fn scope(instance_id: Option<&str>) -> SettingScope {
    if instance_id.is_some() {
        SettingScope::Instance
    } else {
        SettingScope::Global
    }
}

pub(super) fn export(
    manifest: &PluginManifest,
    stored: &Stored,
    instance_id: Option<&str>,
) -> Result<String, String> {
    let values = effective(manifest, stored, instance_id)?;
    let values = values
        .into_iter()
        .filter(|(id, _)| {
            manifest
                .contributions
                .as_ref()
                .and_then(|c| c.setting(id))
                .is_some_and(|field| field.scope == scope(instance_id))
        })
        .collect();
    let yaml = serde_saphyr::to_string(&Document {
        schema_version: 1,
        plugin_id: manifest.id.clone(),
        scope: scope(instance_id),
        values,
    })
    .map_err(|_| "无法导出 YAML 设置")?;
    if yaml.len() > MAX_YAML_BYTES {
        return Err("YAML 设置超过64 KiB限制".into());
    }
    Ok(yaml)
}

pub(super) fn preview(
    manifest: &PluginManifest,
    stored: &Stored,
    instance_id: Option<&str>,
    yaml: &str,
) -> Result<ImportPreview, String> {
    if yaml.len() > MAX_YAML_BYTES {
        return Err("YAML 设置超过64 KiB限制".into());
    }
    manifest.validate().map_err(|_| "插件设置声明无效")?;
    if let Some(id) = instance_id {
        instance_key(id)?;
    }
    let mut budget = serde_saphyr::Budget::default();
    budget.max_depth = 8;
    budget.flow_nesting_limit = 8;
    budget.max_events = 2048;
    budget.max_aliases = 0;
    budget.max_anchors = 0;
    let mut options = serde_saphyr::Options::default();
    options.budget = Some(budget);
    options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::Error;
    options.merge_keys = serde_saphyr::MergeKeyPolicy::Error;
    options.reject_unsupported_tags = true;
    options.strict_booleans = true;
    let document: Document = serde_saphyr::from_str_with_options(yaml, options)
        .map_err(|_| "YAML 设置格式无效或包含不支持的内容")?;
    if document.schema_version != 1
        || document.plugin_id != manifest.id
        || document.scope != scope(instance_id)
    {
        return Err("YAML 设置的插件、版本或作用域与当前目标不匹配".into());
    }
    if document.values.len() > 128 {
        return Err("YAML 设置数量超过限制".into());
    }
    let mut prospective = effective(manifest, stored, instance_id)?;
    for (id, value) in &document.values {
        let field = manifest
            .contributions
            .as_ref()
            .and_then(|c| c.setting(id))
            .ok_or("YAML 包含插件未声明的设置")?;
        if field.scope != scope(instance_id) {
            return Err("YAML 设置作用域与当前目标不匹配".into());
        }
        field
            .validate_value(value)
            .map_err(|_| "YAML 设置值与声明的类型或范围不匹配")?;
        prospective.insert(id.clone(), value.clone());
    }
    if manifest.api_version == cobble_plugin_api::REPORT_CONTEXT_API_VERSION {
        cobble_plugin_host::ReportHostContext::new(manifest, "en", prospective)
            .map_err(|_| "插件设置超过运行上下文限制")?;
    }
    Ok(ImportPreview {
        values: document.values,
    })
}

#[tauri::command]
pub fn export_extension_settings_yaml(
    app: AppHandle,
    plugin_id: String,
    instance_id: Option<String>,
    locale: String,
) -> Result<String, String> {
    super::super::extension_contributions::validate_locale(&locale)?;
    if let Some(id) = &instance_id {
        crate::games::ensure_registered_game(&app, id)?;
    }
    let plugin = super::super::extension_contributions::installed(&plugin_id)?;
    let _guard = WRITES.lock().map_err(|_| "插件设置不可用")?;
    export(
        &plugin.manifest,
        &load(&root()?, &plugin_id)?,
        instance_id.as_deref(),
    )
}

#[tauri::command]
pub fn preview_extension_settings_yaml(
    app: AppHandle,
    plugin_id: String,
    instance_id: Option<String>,
    locale: String,
    yaml: String,
) -> Result<ImportPreview, String> {
    super::super::extension_contributions::validate_locale(&locale)?;
    if let Some(id) = &instance_id {
        crate::games::ensure_registered_game(&app, id)?;
    }
    let plugin = super::super::extension_contributions::installed(&plugin_id)?;
    let _guard = WRITES.lock().map_err(|_| "插件设置不可用")?;
    preview(
        &plugin.manifest,
        &load(&root()?, &plugin_id)?,
        instance_id.as_deref(),
        &yaml,
    )
}

#[cfg(test)]
#[path = "interchange/tests.rs"]
mod tests;
