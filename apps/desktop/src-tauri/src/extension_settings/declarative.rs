// SPDX-License-Identifier: MIT
//! Standard declarative form snapshots share the existing settings file and host context.
use super::*;
use serde_json::json;
pub(super) fn form(manifest: &PluginManifest) -> Option<Value> {
    let settings = manifest.contributions.as_ref()?.settings.as_ref()?;
    serde_json::to_value(settings)
        .ok()?
        .get("form")
        .filter(|value| !value.is_null())
        .cloned()
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    instance_path: String,
    schema_path: String,
    keyword: String,
    message: String,
    params: Value,
}
#[derive(Serialize)]
pub struct SaveResult {
    snapshot: Option<SettingsSnapshot>,
    errors: Vec<ValidationError>,
}
fn error(id: &str, keyword: &str, message: &str, params: Value) -> ValidationError {
    let escaped = id.replace('~', "~0").replace('/', "~1");
    ValidationError {
        instance_path: if keyword == "required" || keyword == "additionalProperties" {
            String::new()
        } else {
            format!("/{escaped}")
        },
        schema_path: if matches!(keyword, "required" | "additionalProperties") {
            format!("#/{keyword}")
        } else if id.is_empty() {
            "#".into()
        } else {
            format!("#/properties/{escaped}/{keyword}")
        },
        keyword: keyword.into(),
        message: message.into(),
        params,
    }
}
fn errors(manifest: &PluginManifest, values: &BTreeMap<String, Value>) -> Vec<ValidationError> {
    let Some(contributions) = &manifest.contributions else {
        return vec![error("", "schema", "Settings form unavailable", json!({}))];
    };
    let defaults = contributions.default_settings();
    let mut errors = vec![];
    for id in defaults.keys() {
        let Some(value) = values.get(id) else {
            errors.push(error(
                id,
                "required",
                "Value is required",
                json!({"missingProperty":id}),
            ));
            continue;
        };
        let Some(field) = contributions.setting(id) else {
            continue;
        };
        if field.validate_value(value).is_err() {
            let keyword = match field.kind {
                SettingKind::Enum => "enum",
                SettingKind::Boolean => "type",
                SettingKind::Number => {
                    if value.is_number() {
                        if value.as_f64().is_some_and(|value| {
                            field
                                .minimum
                                .as_ref()
                                .and_then(Number::as_f64)
                                .is_some_and(|minimum| value < minimum)
                        }) {
                            "minimum"
                        } else {
                            "maximum"
                        }
                    } else {
                        "type"
                    }
                }
                SettingKind::String => {
                    if value.is_string() {
                        "maxLength"
                    } else {
                        "type"
                    }
                }
            };
            errors.push(error(
                id,
                keyword,
                "Value does not match this setting",
                json!({}),
            ));
        }
    }
    for id in values.keys() {
        if !defaults.contains_key(id) {
            errors.push(error(
                id,
                "additionalProperties",
                "Unknown setting",
                json!({"additionalProperty":id}),
            ))
        }
    }
    errors
}
fn save(
    directory: &Path,
    manifest: &PluginManifest,
    values: BTreeMap<String, Value>,
    instance_id: Option<&str>,
) -> Result<Vec<ValidationError>, String> {
    manifest.validate().map_err(|_| "插件设置声明无效")?;
    let definition = form(manifest).ok_or("插件没有声明设置表单")?;
    let scope: SettingScope =
        serde_json::from_value(definition.get("scope").cloned().ok_or("插件设置声明无效")?)
            .map_err(|_| "插件设置声明无效")?;
    if scope != super::interchange::scope(instance_id) {
        return Err("设置作用域与目标不匹配".into());
    }
    if let Some(id) = instance_id {
        instance_key(id)?
    }
    if values.len() > 128 {
        return Err("插件设置数量超过限制".into());
    }
    let errors = errors(manifest, &values);
    if !errors.is_empty() {
        return Ok(errors);
    }
    if manifest
        .contributions
        .as_ref()
        .ok_or("插件设置声明无效")?
        .validate_settings(&values)
        .is_err()
    {
        return Ok(vec![error(
            "",
            "schema",
            "Settings do not match the declared schema",
            json!({}),
        )]);
    }
    let mut stored = load(directory, &manifest.id)?;
    match instance_id {
        None => stored.global.extend(values),
        Some(id) => {
            if !stored.instances.contains_key(id) && stored.instances.len() >= MAX_INSTANCES {
                return Err("插件游戏设置数量超过限制".into());
            }
            stored
                .instances
                .entry(id.into())
                .or_default()
                .extend(values);
        }
    }
    persist_values(directory, manifest, stored, instance_id)?;
    Ok(vec![])
}
#[tauri::command]
pub fn save_extension_settings_form(
    app: AppHandle,
    plugin_id: String,
    values: BTreeMap<String, Value>,
    instance_id: Option<String>,
    locale: String,
) -> Result<SaveResult, String> {
    super::super::extension_contributions::validate_locale(&locale)?;
    if let Some(id) = &instance_id {
        crate::games::ensure_registered_game(&app, id)?
    }
    let plugin = super::super::extension_contributions::installed(&plugin_id)?;
    let errors = {
        let _guard = WRITES.lock().map_err(|_| "插件设置不可用")?;
        save(&root()?, &plugin.manifest, values, instance_id.as_deref())?
    };
    let snapshot = if errors.is_empty() {
        Some(snapshot(&plugin.manifest, instance_id, &locale)?)
    } else {
        None
    };
    Ok(SaveResult { snapshot, errors })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> PluginManifest {
        let mut value = serde_json::to_value(super::super::tests::manifest()).unwrap();
        value["contributions"]["settings"] = json!({"form":{"schema_version":1,"scope":"global","schema":{"type":"object","additionalProperties":false,"required":["show_details"],"properties":{"show_details":{"type":"boolean","default":true,"title":"Show details"}}}}});
        serde_json::from_value(value).unwrap()
    }
    #[test]
    fn form_save_reopens_as_guest_settings_and_invalid_form_never_writes() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = manifest();
        let invalid = BTreeMap::from([("show_details".into(), json!("bad"))]);
        let errors = save(dir.path(), &manifest, invalid, None).unwrap();
        assert_eq!(errors[0].instance_path, "/show_details");
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
        assert!(
            save(
                dir.path(),
                &manifest,
                BTreeMap::from([("show_details".into(), json!(false))]),
                None
            )
            .unwrap()
            .is_empty()
        );
        let stored = load(dir.path(), &manifest.id).unwrap();
        let effective = effective(&manifest, &stored, None).unwrap();
        assert_eq!(effective["show_details"], false);
        let yaml = super::super::interchange::export(&manifest, &stored, None).unwrap();
        let preview = super::super::interchange::preview(&manifest, &stored, None, &yaml).unwrap();
        let imported: BTreeMap<String, Value> =
            serde_json::from_value(serde_json::to_value(preview).unwrap()["values"].clone())
                .unwrap();
        assert_eq!(imported["show_details"], false);
        let context =
            cobble_plugin_host::ReportHostContext::new(&manifest, "en", effective).unwrap();
        assert!(
            context
                .settings_json()
                .unwrap()
                .contains("\"show_details\":false")
        );
        assert!(
            save(
                dir.path(),
                &manifest,
                BTreeMap::from([("show_details".into(), json!(true))]),
                Some("instance")
            )
            .is_err()
        );
    }
    #[test]
    fn config_doctor_form_preserves_existing_format_and_defaults_new_boolean() {
        let dir = tempfile::tempdir().unwrap();
        let manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../../examples/config-doctor/plugin.json"
        ))
        .unwrap();
        let mut stored = Stored::default();
        stored.global.insert("format".into(), json!("toml"));
        let before = effective(&manifest, &stored, None).unwrap();
        assert_eq!(before["format"], "toml");
        assert_eq!(before["warn_duplicate_properties"], true);
        persist_values(dir.path(), &manifest, stored, None).unwrap();
        write_value(
            dir.path(),
            &manifest,
            "warn_duplicate_properties",
            json!(false),
            None,
        )
        .unwrap();
        let saved = effective(&manifest, &load(dir.path(), &manifest.id).unwrap(), None).unwrap();
        assert_eq!(saved["format"], "toml");
        assert_eq!(saved["warn_duplicate_properties"], false);
        let categories = categories(&manifest, "en");
        assert_eq!(categories[0].id, "form");
        assert_eq!(categories[0].fields.len(), 2);
    }
}
