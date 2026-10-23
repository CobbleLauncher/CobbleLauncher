// SPDX-License-Identifier: MIT
//! Bounded namespaced settings; values never grant new plugin capabilities.
use cobble_plugin_api::{PluginManifest, SettingField, SettingKind, SettingScope};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::AppHandle;

#[path = "extension_settings/declarative.rs"]
pub mod declarative;
#[path = "extension_settings/interchange.rs"]
pub mod interchange;

const MAX_BYTES: usize = 1024 * 1024;
const MAX_INSTANCES: usize = 256;
static WRITES: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    schema_version: u32,
    #[serde(default)]
    global: BTreeMap<String, Value>,
    #[serde(default)]
    instances: BTreeMap<String, BTreeMap<String, Value>>,
}
impl Default for Stored {
    fn default() -> Self {
        Self {
            schema_version: 1,
            global: BTreeMap::new(),
            instances: BTreeMap::new(),
        }
    }
}

fn root() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("无法找到设置目录")?
        .join("CobbleLauncher/extension-settings"))
}
fn instance_key(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Err("游戏 ID 无效".into())
    } else {
        Ok(())
    }
}
fn ordinary(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "无法读取插件设置")?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        return Err("插件设置位置必须是普通文件或目录".into());
    }
    Ok(())
}
fn load(directory: &Path, plugin_id: &str) -> Result<Stored, String> {
    super::valid_id(plugin_id)?;
    match fs::symlink_metadata(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Stored::default()),
        Err(_) => return Err("无法读取插件设置目录".into()),
        Ok(_) => (),
    }
    ordinary(directory, true)?;
    let path = directory.join(format!("{plugin_id}.json"));
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Stored::default()),
        Err(_) => return Err("无法读取插件设置".into()),
        Ok(_) => (),
    }
    ordinary(&path, false)?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "无法读取插件设置")?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取插件设置")?;
    if bytes.len() > MAX_BYTES {
        return Err("插件设置超过大小限制".into());
    }
    let stored: Stored = serde_json::from_slice(&bytes).map_err(|_| "插件设置文件无效")?;
    if stored.schema_version != 1
        || stored.instances.len() > MAX_INSTANCES
        || stored.global.len() > 128
        || stored
            .instances
            .iter()
            .any(|(key, values)| instance_key(key).is_err() || values.len() > 128)
    {
        return Err("插件设置版本或数量不受支持".into());
    }
    Ok(stored)
}
fn effective(
    manifest: &PluginManifest,
    stored: &Stored,
    instance_id: Option<&str>,
) -> Result<BTreeMap<String, Value>, String> {
    manifest.validate().map_err(|_| "插件设置声明无效")?;
    if let Some(instance_id) = instance_id {
        instance_key(instance_id)?;
    }
    let Some(contributions) = &manifest.contributions else {
        return Ok(BTreeMap::new());
    };
    let mut values = contributions.default_settings();
    for (id, value) in &mut values {
        let field = contributions.setting(id).ok_or("插件设置声明无效")?;
        let saved = match field.scope {
            SettingScope::Global => stored.global.get(id),
            SettingScope::Instance => instance_id
                .and_then(|instance_id| stored.instances.get(instance_id))
                .and_then(|values| values.get(id)),
        };
        if let Some(saved) = saved
            && field.validate_value(saved).is_ok()
        {
            *value = saved.clone();
        }
    }
    Ok(values)
}
fn write_value(
    directory: &Path,
    manifest: &PluginManifest,
    field_id: &str,
    value: Value,
    instance_id: Option<&str>,
) -> Result<(), String> {
    manifest.validate().map_err(|_| "插件设置声明无效")?;
    let field = manifest
        .contributions
        .as_ref()
        .and_then(|value| value.setting(field_id))
        .ok_or("插件没有声明此设置")?;
    field
        .validate_value(&value)
        .map_err(|_| "设置值与声明的类型或范围不匹配")?;
    match (field.scope, instance_id) {
        (SettingScope::Global, None) => (),
        (SettingScope::Instance, Some(id)) => instance_key(id)?,
        _ => return Err("设置作用域与目标不匹配".into()),
    }
    let mut stored = load(directory, &manifest.id)?;
    match instance_id {
        None => {
            stored.global.insert(field_id.into(), value);
        }
        Some(id) => {
            if !stored.instances.contains_key(id) && stored.instances.len() >= MAX_INSTANCES {
                return Err("插件游戏设置数量超过限制".into());
            }
            stored
                .instances
                .entry(id.into())
                .or_default()
                .insert(field_id.into(), value);
        }
    }
    persist_values(directory, manifest, stored, instance_id)
}
fn persist_values(
    directory: &Path,
    manifest: &PluginManifest,
    stored: Stored,
    instance_id: Option<&str>,
) -> Result<(), String> {
    if manifest.api_version == cobble_plugin_api::REPORT_CONTEXT_API_VERSION {
        cobble_plugin_host::ReportHostContext::new(
            manifest,
            "en",
            effective(manifest, &stored, instance_id)?,
        )
        .map_err(|_| "插件设置超过运行上下文限制")?;
    }
    let bytes = serde_json::to_vec(&stored).map_err(|_| "无法保存插件设置")?;
    if bytes.len() > MAX_BYTES {
        return Err("插件设置超过大小限制".into());
    }
    fs::create_dir_all(directory).map_err(|_| "无法创建插件设置目录")?;
    ordinary(directory, true)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|_| "无法保存插件设置")?;
    temporary
        .write_all(&bytes)
        .map_err(|_| "无法保存插件设置")?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "无法保存插件设置")?;
    temporary
        .persist(directory.join(format!("{}.json", manifest.id)))
        .map_err(|_| "无法保存插件设置")?;
    Ok(())
}

/// Native callers validate the selected instance before requesting its scope.
pub(super) fn effective_settings(
    manifest: &PluginManifest,
    instance_id: Option<&str>,
) -> Result<BTreeMap<String, Value>, String> {
    manifest.validate().map_err(|_| "插件设置声明无效")?;
    if manifest
        .contributions
        .as_ref()
        .and_then(|value| value.settings.as_ref())
        .is_none()
    {
        return Ok(BTreeMap::new());
    }
    let _guard = WRITES.lock().map_err(|_| "插件设置不可用")?;
    effective(manifest, &load(&root()?, &manifest.id)?, instance_id)
}

pub(super) fn report_context(
    manifest: &PluginManifest,
    instance_id: Option<&str>,
    locale: &str,
) -> Result<cobble_plugin_host::ReportHostContext, String> {
    super::extension_contributions::validate_locale(locale)?;
    cobble_plugin_host::ReportHostContext::new(
        manifest,
        locale,
        effective_settings(manifest, instance_id)?,
    )
    .map_err(|_| "插件运行设置无效".into())
}

#[derive(Serialize)]
pub struct OptionDescriptor {
    value: Value,
    label: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDescriptor {
    id: String,
    #[serde(rename = "type")]
    kind: SettingKind,
    label: String,
    scope: SettingScope,
    advanced: bool,
    default: Value,
    options: Vec<OptionDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum: Option<Number>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum: Option<Number>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_length: Option<u32>,
}
#[derive(Serialize)]
pub struct CategoryDescriptor {
    id: String,
    label: String,
    fields: Vec<FieldDescriptor>,
}
fn field_descriptor(field: &SettingField, locale: &str) -> FieldDescriptor {
    FieldDescriptor {
        id: field.id.clone(),
        kind: field.kind,
        label: field.label.resolve(locale).into(),
        scope: field.scope,
        advanced: field.advanced,
        default: field.default.clone(),
        options: field
            .options
            .iter()
            .map(|option| OptionDescriptor {
                value: option.value.clone(),
                label: option.label.resolve(locale).into(),
            })
            .collect(),
        minimum: field.minimum.clone(),
        maximum: field.maximum.clone(),
        max_length: field.max_length,
    }
}
pub(super) fn categories(manifest: &PluginManifest, locale: &str) -> Vec<CategoryDescriptor> {
    manifest
        .contributions
        .as_ref()
        .and_then(|value| value.settings.as_ref())
        .map(|settings| {
            if let Some(form) = &settings.form {
                return vec![CategoryDescriptor {
                    id: "form".into(),
                    label: form
                        .schema
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or(&manifest.name)
                        .into(),
                    fields: settings
                        .fields()
                        .iter()
                        .map(|field| field_descriptor(field, locale))
                        .collect(),
                }];
            }
            settings
                .categories
                .iter()
                .map(|category| CategoryDescriptor {
                    id: category.id.clone(),
                    label: category.label.resolve(locale).into(),
                    fields: category
                        .fields
                        .iter()
                        .map(|field| field_descriptor(field, locale))
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default()
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    plugin_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance_id: Option<String>,
    values: BTreeMap<String, Value>,
    categories: Vec<CategoryDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    form: Option<Value>,
}
fn snapshot(
    manifest: &PluginManifest,
    instance_id: Option<String>,
    locale: &str,
) -> Result<SettingsSnapshot, String> {
    Ok(SettingsSnapshot {
        plugin_id: manifest.id.clone(),
        values: effective_settings(manifest, instance_id.as_deref())?,
        instance_id,
        categories: categories(manifest, locale),
        form: declarative::form(manifest),
    })
}
#[tauri::command]
pub fn get_extension_settings(
    app: AppHandle,
    plugin_id: String,
    instance_id: Option<String>,
    locale: String,
) -> Result<SettingsSnapshot, String> {
    super::extension_contributions::validate_locale(&locale)?;
    if let Some(id) = &instance_id {
        crate::games::ensure_registered_game(&app, id)?;
    }
    let plugin = super::extension_contributions::installed(&plugin_id)?;
    snapshot(&plugin.manifest, instance_id, &locale)
}
#[tauri::command]
pub fn set_extension_setting(
    app: AppHandle,
    plugin_id: String,
    field_id: String,
    value: Value,
    instance_id: Option<String>,
    locale: String,
) -> Result<SettingsSnapshot, String> {
    super::extension_contributions::validate_locale(&locale)?;
    if let Some(id) = &instance_id {
        crate::games::ensure_registered_game(&app, id)?;
    }
    let plugin = super::extension_contributions::installed(&plugin_id)?;
    {
        let _guard = WRITES.lock().map_err(|_| "插件设置不可用")?;
        write_value(
            &root()?,
            &plugin.manifest,
            &field_id,
            value,
            instance_id.as_deref(),
        )?;
    }
    snapshot(&plugin.manifest, instance_id, &locale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    pub(super) fn manifest() -> PluginManifest {
        serde_json::from_value(json!({"id":"fixture","name":"Fixture","version":"1","api_version":"0.2.0","publisher":"Original fixture","capabilities":["report"],"file_extensions":["json"],"component_sha256":"a".repeat(64),
          "contributions":{"schema_version":1,"commands":[],"settings":{"categories":[{"id":"general","label":{"en":"General"},"fields":[
            {"id":"page_size","type":"enum","label":{"en":"Page size"},"scope":"global","default":50,"options":[{"value":20,"label":{"en":"20"}},{"value":50,"label":{"en":"50"}}]},
            {"id":"enabled","type":"boolean","label":{"en":"Enabled"},"scope":"instance","default":true}]}]}}})).unwrap()
    }
    #[test]
    fn defaults_namespaces_typed_values_and_instance_scope_persist() {
        let root = tempfile::tempdir().unwrap();
        let manifest = manifest();
        assert_eq!(
            effective(&manifest, &load(root.path(), "fixture").unwrap(), None).unwrap()["page_size"],
            50
        );
        assert!(fs::read_dir(root.path()).unwrap().next().is_none());
        write_value(root.path(), &manifest, "page_size", json!(20), None).unwrap();
        write_value(root.path(), &manifest, "enabled", json!(false), Some("abc")).unwrap();
        let stored = load(root.path(), "fixture").unwrap();
        assert_eq!(
            effective(&manifest, &stored, Some("abc")).unwrap()["enabled"],
            false
        );
        assert_eq!(
            effective(&manifest, &stored, Some("def")).unwrap()["enabled"],
            true
        );
        assert_eq!(
            effective(&manifest, &stored, None).unwrap()["page_size"],
            20
        );
        assert!(fs::read(root.path().join("fixture.json")).unwrap().len() < MAX_BYTES);
    }
    #[test]
    fn wrong_scope_unknown_field_and_type_fail_without_modifying_file() {
        let root = tempfile::tempdir().unwrap();
        let manifest = manifest();
        write_value(root.path(), &manifest, "page_size", json!(20), None).unwrap();
        let original = fs::read(root.path().join("fixture.json")).unwrap();
        for (id, value, instance) in [
            ("page_size", json!("20"), None),
            ("page_size", json!(50), Some("abc")),
            ("enabled", json!(false), None),
            ("undeclared", json!(true), None),
        ] {
            assert!(write_value(root.path(), &manifest, id, value, instance).is_err());
            assert_eq!(
                fs::read(root.path().join("fixture.json")).unwrap(),
                original
            );
        }
    }
    #[test]
    fn stale_values_fall_back_and_corrupt_files_are_not_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let manifest = manifest();
        fs::write(root.path().join("fixture.json"),br#"{"schema_version":1,"global":{"page_size":"old-type","removed_setting":true},"instances":{}}"#).unwrap();
        let stored = load(root.path(), "fixture").unwrap();
        assert_eq!(
            effective(&manifest, &stored, None).unwrap()["page_size"],
            50
        );
        fs::write(root.path().join("fixture.json"), b"broken").unwrap();
        assert!(write_value(root.path(), &manifest, "page_size", json!(20), None).is_err());
        assert_eq!(
            fs::read(root.path().join("fixture.json")).unwrap(),
            b"broken"
        );
    }
    #[test]
    fn report_settings_cannot_exceed_runtime_context_budget() {
        let root = tempfile::tempdir().unwrap();
        let mut manifest = manifest();
        let fields = &mut manifest
            .contributions
            .as_mut()
            .unwrap()
            .settings
            .as_mut()
            .unwrap()
            .categories[0]
            .fields;
        for index in 0..5 {
            fields.push(SettingField {
                id: format!("text_{index}"),
                kind: SettingKind::String,
                label: cobble_plugin_api::LocalizedText {
                    en: format!("Text {index}"),
                    zh_cn: None,
                },
                scope: SettingScope::Global,
                advanced: false,
                default: Value::String(String::new()),
                options: vec![],
                minimum: None,
                maximum: None,
                max_length: Some(1024),
            });
        }
        for index in 0..3 {
            write_value(
                root.path(),
                &manifest,
                &format!("text_{index}"),
                json!("🙂".repeat(1024)),
                None,
            )
            .unwrap();
        }
        let original = fs::read(root.path().join("fixture.json")).unwrap();
        assert!(
            write_value(
                root.path(),
                &manifest,
                "text_3",
                json!("🙂".repeat(1024)),
                None
            )
            .is_err()
        );
        assert_eq!(
            fs::read(root.path().join("fixture.json")).unwrap(),
            original
        );
    }
}
