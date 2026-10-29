// SPDX-License-Identifier: MIT
use serde::{Deserialize, Serialize};
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Preferences {
    background_check: bool,
    channel: Channel,
}
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Channel {
    Stable,
    Preview,
}
impl Default for Channel {
    fn default() -> Self {
        channel_for_version(env!("CARGO_PKG_VERSION"))
    }
}
fn channel_for_version(version: &str) -> Channel {
    if version.contains('-') {
        Channel::Preview
    } else {
        Channel::Stable
    }
}
#[derive(Serialize)]
pub struct UpdateStatus {
    version: String,
    background_check: bool,
    channel: Channel,
    configured: bool,
    can_install: bool,
    available_version: Option<String>,
}
#[derive(Default)]
pub struct UpdateState {
    candidate: Mutex<Option<tauri_plugin_updater::Update>>,
    operation: tokio::sync::Mutex<()>,
}
fn preferences_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join("launcher-updates.json"))
        .map_err(|e| e.to_string())
}
fn read(app: &AppHandle) -> Result<Preferences, String> {
    match std::fs::read(preferences_path(app)?) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "UPDATE_PREFERENCES_INVALID".into())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(_) => Err("UPDATE_PREFERENCES_READ_FAILED".into()),
    }
}
fn configuration(channel: Channel) -> Option<(url::Url, &'static str)> {
    let endpoint = match channel {
        Channel::Stable => option_env!("COBBLE_UPDATER_STABLE_URL").unwrap_or(
            "https://github.com/CobbleLauncher/CobbleLauncher/releases/download/update-stable/latest.json",
        ),
        Channel::Preview => option_env!("COBBLE_UPDATER_PREVIEW_URL").unwrap_or(
            "https://github.com/CobbleLauncher/CobbleLauncher/releases/download/update-preview/latest.json",
        ),
    };
    let key = option_env!("COBBLE_UPDATER_PUBLIC_KEY")
        .unwrap_or(include_str!("../updater.pub"))
        .trim();
    let endpoint = url::Url::parse(endpoint).ok()?;
    if endpoint.scheme() != "https"
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || key.is_empty()
    {
        return None;
    }
    Some((endpoint, key))
}
#[tauri::command]
pub fn launcher_update_status(
    app: AppHandle,
    state: tauri::State<'_, UpdateState>,
) -> Result<UpdateStatus, String> {
    let preferences = read(&app)?;
    let configured = configuration(preferences.channel).is_some();
    Ok(UpdateStatus {
        version: app.package_info().version.to_string(),
        background_check: preferences.background_check,
        channel: preferences.channel,
        configured,
        can_install: configured && cfg!(feature = "updater-distribution"),
        available_version: state
            .candidate
            .lock()
            .map_err(|_| "UPDATE_STATE_UNAVAILABLE")?
            .as_ref()
            .map(|u| u.version.clone()),
    })
}
#[tauri::command]
pub fn set_launcher_update_preferences(
    app: AppHandle,
    state: tauri::State<'_, UpdateState>,
    background_check: bool,
    channel: String,
) -> Result<UpdateStatus, String> {
    let _guard = state.operation.try_lock().map_err(|_| "UPDATE_BUSY")?;
    let channel = match channel.as_str() {
        "stable" => Channel::Stable,
        "preview" => Channel::Preview,
        _ => return Err("UPDATE_CHANNEL_INVALID".into()),
    };
    let preferences = Preferences {
        background_check,
        channel,
    };
    let path = preferences_path(&app)?;
    std::fs::create_dir_all(path.parent().ok_or("UPDATE_PREFERENCES_WRITE_FAILED")?)
        .map_err(|_| "UPDATE_PREFERENCES_WRITE_FAILED")?;
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().ok_or("UPDATE_PREFERENCES_WRITE_FAILED")?)
            .map_err(|_| "UPDATE_PREFERENCES_WRITE_FAILED")?;
    use std::io::Write;
    file.write_all(
        &serde_json::to_vec(&preferences).map_err(|_| "UPDATE_PREFERENCES_WRITE_FAILED")?,
    )
    .map_err(|_| "UPDATE_PREFERENCES_WRITE_FAILED")?;
    file.as_file()
        .sync_all()
        .map_err(|_| "UPDATE_PREFERENCES_WRITE_FAILED")?;
    file.persist(&path)
        .map_err(|_| "UPDATE_PREFERENCES_WRITE_FAILED")?;
    *state
        .candidate
        .lock()
        .map_err(|_| "UPDATE_STATE_UNAVAILABLE")? = None;
    launcher_update_status(app, state.clone())
}
#[tauri::command]
pub async fn check_launcher_update(
    app: AppHandle,
    state: tauri::State<'_, UpdateState>,
    automatic: bool,
) -> Result<UpdateStatus, String> {
    let _guard = state.operation.try_lock().map_err(|_| "UPDATE_BUSY")?;
    let preferences = read(&app)?;
    if automatic && !preferences.background_check {
        return launcher_update_status(app, state.clone());
    }
    let (endpoint, key) = configuration(preferences.channel).ok_or("UPDATE_UNCONFIGURED")?;
    *state
        .candidate
        .lock()
        .map_err(|_| "UPDATE_STATE_UNAVAILABLE")? = None;
    let update = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|_| "UPDATE_CONFIGURATION_INVALID")?
        .pubkey(key)
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "UPDATE_CONFIGURATION_INVALID")?
        .check()
        .await
        .map_err(|_| "UPDATE_CHECK_FAILED")?;
    *state
        .candidate
        .lock()
        .map_err(|_| "UPDATE_STATE_UNAVAILABLE")? = update;
    launcher_update_status(app, state.clone())
}
#[tauri::command]
pub async fn install_launcher_update(
    app: AppHandle,
    state: tauri::State<'_, UpdateState>,
) -> Result<(), String> {
    if !cfg!(feature = "updater-distribution") {
        return Err("UPDATE_PORTABLE".into());
    }
    let _guard = state.operation.try_lock().map_err(|_| "UPDATE_BUSY")?;
    crate::games::launcher_update_safe(&app)?;
    let candidate = state
        .candidate
        .lock()
        .map_err(|_| "UPDATE_STATE_UNAVAILABLE")?
        .clone()
        .ok_or("UPDATE_CHECK_REQUIRED")?;
    let bytes = candidate
        .download(|_, _| {}, || {})
        .await
        .map_err(|_| "UPDATE_DOWNLOAD_FAILED")?;
    crate::games::launcher_update_safe(&app)?;
    candidate
        .install(bytes)
        .map_err(|_| "UPDATE_INSTALL_FAILED".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn updates_are_opt_in_and_default_to_the_build_channel() {
        let settings: Preferences = serde_json::from_str("{}").unwrap();
        assert!(!settings.background_check);
        assert_eq!(
            matches!(settings.channel, Channel::Preview),
            env!("CARGO_PKG_VERSION").contains('-')
        );
        assert!(matches!(
            channel_for_version("0.1.0-beta.2"),
            Channel::Preview
        ));
        assert!(matches!(channel_for_version("0.1.0"), Channel::Stable));
        let saved: Preferences = serde_json::from_str(r#"{"channel":"stable"}"#).unwrap();
        assert!(matches!(saved.channel, Channel::Stable));
        assert!(serde_json::from_str::<Preferences>(r#"{"channel":"other"}"#).is_err());
    }

    #[test]
    fn release_channels_use_https_and_a_real_public_key() {
        use base64::Engine;
        let (stable, key) = configuration(Channel::Stable).unwrap();
        let (preview, _) = configuration(Channel::Preview).unwrap();
        assert_eq!(stable.scheme(), "https");
        assert_eq!(preview.scheme(), "https");
        assert_ne!(stable, preview);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(key)
            .unwrap();
        assert!(decoded.starts_with(b"untrusted comment: minisign public key:"));
        let empty: tauri_plugin_updater::RemoteRelease =
            serde_json::from_value(serde_json::json!({ "version": "0.0.0", "platforms": {} }))
                .unwrap();
        assert!(empty.version.to_string() == "0.0.0");
    }
}
