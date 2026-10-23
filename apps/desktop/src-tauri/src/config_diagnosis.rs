// SPDX-License-Identifier: MIT
//! Read-only selected-config snapshot -> existing sandboxed report workflow.
use super::*;
#[tauri::command]
pub async fn analyze_game_config_report(
    app: AppHandle,
    game_id: String,
    config_id: String,
    expected_revision: String,
    plugin_id: String,
    locale: Option<String>,
) -> Result<Option<Report>, String> {
    let locale = locale.unwrap_or_else(|| "en".into());
    super::super::extension_contributions::validate_locale(&locale)?;
    let directory = crate::games::game_directory(&app, &game_id)?;
    let cancel = state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .begin(plugin_id.clone())?;
    let result = async {
        let id = plugin_id.clone();
        let (plugin, component, bytes, media_type) =
            tauri::async_runtime::spawn_blocking(move || {
                let plugin = selected_plugin(&id)?;
                let (bytes, extension) = crate::games::game_config::report_snapshot(
                    &directory,
                    &config_id,
                    &expected_revision,
                )?;
                let media_type = compatible_media(&plugin.manifest.file_extensions, &extension)?;
                let component = package(&plugin)?;
                Ok::<_, String>((plugin, component, bytes, media_type))
            })
            .await
            .map_err(|_| "无法读取配置文件")??;
        crate::games::ensure_registered_game(&app, &game_id)?;
        execute_report(
            plugin,
            component,
            bytes,
            media_type,
            cancel.clone(),
            locale,
            Some(game_id),
        )
        .await
    }
    .await;
    state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .finish(&cancel, result)
}
fn compatible_media(extensions: &[String], extension: &str) -> Result<&'static str, String> {
    if !extensions.iter().any(|allowed| allowed == extension) {
        return Err("插件不支持此文件类型".into());
    }
    report_media_type(extension).ok_or_else(|| "插件不支持此文件类型".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_config_respects_declared_extension_and_canonical_media() {
        for (extension, media) in [
            ("json", "application/json"),
            ("toml", "application/toml"),
            ("properties", "text/x-java-properties"),
            ("yaml", "application/yaml"),
            ("yml", "application/yaml"),
        ] {
            assert_eq!(
                compatible_media(&[extension.into()], extension).unwrap(),
                media
            );
            assert!(compatible_media(&["log".into()], extension).is_err());
        }
        assert!(compatible_media(&["exe".into()], "exe").is_err());
    }
}
