// SPDX-License-Identifier: MIT
//! Declarative metadata resolution. Browsing never compiles or starts a plugin.
use cobble_plugin_api::{
    CommandContribution, ContextCondition, InstalledPlugin, MenuContribution, MenuPosition,
    Workflow,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

pub(super) fn validate_locale(locale: &str) -> Result<(), String> {
    if matches!(locale, "en" | "zh-CN") {
        Ok(())
    } else {
        Err("不支持的界面语言".into())
    }
}
pub(super) fn installed(plugin_id: &str) -> Result<InstalledPlugin, String> {
    super::valid_id(plugin_id)?;
    super::registry()?
        .resolve_enabled(plugin_id)
        .map_err(|error| error.to_string())
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionContext {
    instance_id: Option<String>,
    blueprint_id: Option<String>,
}
struct CheckedContext {
    instance_id: Option<String>,
    blueprint_id: Option<String>,
    stopped: bool,
}
fn checked_context(
    app: &AppHandle,
    context: Option<ExtensionContext>,
) -> Result<CheckedContext, String> {
    let context = context.unwrap_or_default();
    let stopped = match &context.instance_id {
        Some(id) => crate::games::registered_game_stopped(app, id)?,
        None => false,
    };
    if let Some(id) = &context.blueprint_id {
        super::valid_id(id)?;
        let bytes = super::read_file(&super::root("blueprints")?.join(format!("{id}.json")))
            .map_err(|_| "蓝图已不存在")?;
        let blueprint: super::Blueprint =
            serde_json::from_slice(&bytes).map_err(|_| "蓝图元数据无效")?;
        if &blueprint.id != id {
            return Err("蓝图已不存在".into());
        }
    }
    Ok(CheckedContext {
        instance_id: context.instance_id,
        blueprint_id: context.blueprint_id,
        stopped,
    })
}
fn visible(menu: &MenuContribution, context: &CheckedContext) -> bool {
    let position = match menu.position {
        MenuPosition::Tools => true,
        MenuPosition::Instance => context.instance_id.is_some(),
        MenuPosition::Blueprint => context.blueprint_id.is_some(),
    };
    position
        && match menu.when {
            ContextCondition::Always => true,
            ContextCondition::InstanceSelected => context.instance_id.is_some(),
            ContextCondition::InstanceStopped => context.stopped,
            ContextCondition::BlueprintSelected => context.blueprint_id.is_some(),
        }
}
#[derive(Serialize)]
pub struct CommandDescriptor {
    id: String,
    label: String,
    workflow: Workflow,
    menus: Vec<MenuContribution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(rename = "searchKeywords", skip_serializing_if = "Vec::is_empty")]
    search_keywords: Vec<String>,
    #[serde(rename = "quickEntry")]
    quick_entry: bool,
    #[serde(rename = "suggestedShortcut", skip_serializing_if = "Option::is_none")]
    suggested_shortcut: Option<String>,
}
fn command_descriptor(
    command: &CommandContribution,
    context: &CheckedContext,
    locale: &str,
    position: Option<MenuPosition>,
) -> Option<CommandDescriptor> {
    let menus: Vec<_> = command
        .menus
        .iter()
        .filter(|menu| {
            position.is_none_or(|position| menu.position == position) && visible(menu, context)
        })
        .cloned()
        .collect();
    (!menus.is_empty()).then(|| CommandDescriptor {
        id: command.id.clone(),
        label: command.label.resolve(locale).into(),
        workflow: command.workflow,
        menus,
        icon: command.icon.clone(),
        search_keywords: command.search_keywords.clone(),
        quick_entry: command.quick_entry,
        suggested_shortcut: command.suggested_shortcut.clone(),
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionDescriptor {
    plugin_id: String,
    plugin_name: String,
    commands: Vec<CommandDescriptor>,
    settings: Vec<super::extension_settings::CategoryDescriptor>,
}
#[tauri::command]
pub fn list_extension_contributions(
    app: AppHandle,
    locale: String,
    context: Option<ExtensionContext>,
    position: Option<MenuPosition>,
) -> Result<Vec<ExtensionDescriptor>, String> {
    validate_locale(&locale)?;
    let context = checked_context(&app, context)?;
    let plugins = super::registry()?.list().map_err(|_| "无法读取插件列表")?;
    Ok(cobble_plugin_host::interfaces::ready_plugins(&plugins)
        .into_iter()
        .filter_map(|plugin| {
            let contributions = plugin.manifest.contributions.as_ref()?;
            let commands = contributions
                .commands
                .iter()
                .filter_map(|command| command_descriptor(command, &context, &locale, position))
                .collect();
            let settings = super::extension_settings::categories(&plugin.manifest, &locale);
            Some(ExtensionDescriptor {
                plugin_id: plugin.manifest.id.clone(),
                plugin_name: plugin.manifest.name.clone(),
                commands,
                settings,
            })
        })
        .collect())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandBinding {
    plugin_id: String,
    command_id: String,
    label: String,
    workflow: Workflow,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blueprint_id: Option<String>,
}
#[tauri::command]
pub fn resolve_extension_command(
    app: AppHandle,
    plugin_id: String,
    command_id: String,
    locale: String,
    context: Option<ExtensionContext>,
    position: Option<MenuPosition>,
) -> Result<CommandBinding, String> {
    validate_locale(&locale)?;
    let context = checked_context(&app, context)?;
    let plugin = installed(&plugin_id)?;
    let command = plugin
        .manifest
        .contributions
        .as_ref()
        .and_then(|value| {
            value
                .commands
                .iter()
                .find(|command| command.id == command_id)
        })
        .ok_or("插件没有声明此操作")?;
    if !command.menus.iter().any(|menu| {
        position.is_none_or(|position| menu.position == position) && visible(menu, &context)
    }) {
        return Err("此操作不适用于当前选择".into());
    }
    Ok(CommandBinding {
        plugin_id,
        command_id,
        label: command.label.resolve(&locale).into(),
        workflow: command.workflow,
        instance_id: context.instance_id,
        blueprint_id: context.blueprint_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_plugin_api::LocalizedText;
    fn checked(instance: bool, blueprint: bool, stopped: bool) -> CheckedContext {
        CheckedContext {
            instance_id: instance.then(|| "abc".into()),
            blueprint_id: blueprint.then(|| "saved".into()),
            stopped,
        }
    }
    #[test]
    fn positions_and_conditions_use_checked_context() {
        let stopped = MenuContribution {
            position: MenuPosition::Instance,
            when: ContextCondition::InstanceStopped,
        };
        assert!(!visible(&stopped, &checked(false, false, false)));
        assert!(!visible(&stopped, &checked(true, false, false)));
        assert!(visible(&stopped, &checked(true, false, true)));
        let blueprint = MenuContribution {
            position: MenuPosition::Blueprint,
            when: ContextCondition::Always,
        };
        assert!(!visible(&blueprint, &checked(true, false, true)));
        assert!(visible(&blueprint, &checked(false, true, false)));
    }
    #[test]
    fn labels_resolve_with_fallback_and_invisible_commands_are_absent() {
        let command = CommandContribution {
            id: "fixture.inspect".into(),
            label: LocalizedText {
                en: "Inspect".into(),
                zh_cn: Some("检查".into()),
            },
            workflow: Workflow::Report,
            menus: vec![MenuContribution {
                position: MenuPosition::Tools,
                when: ContextCondition::Always,
            }],
            icon: None,
            search_keywords: Vec::new(),
            quick_entry: false,
            suggested_shortcut: None,
        };
        assert_eq!(
            command_descriptor(&command, &checked(false, false, false), "zh-CN", None)
                .unwrap()
                .label,
            "检查"
        );
        let mut discovery = command.clone();
        discovery.icon = Some("search".into());
        discovery.search_keywords = vec!["inspect".into()];
        discovery.quick_entry = true;
        discovery.suggested_shortcut = Some("Ctrl+Shift+I".into());
        let descriptor =
            command_descriptor(&discovery, &checked(false, false, false), "en", None).unwrap();
        let value = serde_json::to_value(descriptor).unwrap();
        assert_eq!(value["searchKeywords"], serde_json::json!(["inspect"]));
        assert_eq!(value["quickEntry"], true);
        assert_eq!(value["suggestedShortcut"], "Ctrl+Shift+I");
        assert!(
            command_descriptor(
                &command,
                &checked(true, false, true),
                "en",
                Some(MenuPosition::Instance)
            )
            .is_none()
        );
        assert!(
            command_descriptor(
                &command,
                &checked(true, false, true),
                "en",
                Some(MenuPosition::Tools)
            )
            .is_some()
        );
        let mut command = command;
        command.label.zh_cn = None;
        assert_eq!(
            command_descriptor(&command, &checked(false, false, false), "zh-CN", None)
                .unwrap()
                .label,
            "Inspect"
        );
        command.menus[0].position = MenuPosition::Instance;
        assert!(command_descriptor(&command, &checked(false, false, false), "en", None).is_none());
        assert!(validate_locale("arbitrary/untrusted").is_err());
    }
}
