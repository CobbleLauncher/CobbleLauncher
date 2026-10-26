// SPDX-License-Identifier: MIT
use cobble_auth::{AuthError, DeviceLogin, LoginPoll};

fn login_error(error: cobble_auth::BoxError) -> String {
    if matches!(
        error.downcast_ref::<AuthError>(),
        Some(AuthError::VaultUnavailable)
    ) {
        "AUTH_VAULT_UNAVAILABLE".into()
    } else {
        error.to_string()
    }
}

#[tauri::command]
pub async fn begin_microsoft_login() -> Result<DeviceLogin, String> {
    cobble_auth::begin_microsoft_login()
        .await
        .map_err(login_error)
}

#[tauri::command]
pub async fn poll_microsoft_login(flow_id: String) -> Result<LoginPoll, String> {
    cobble_auth::poll_microsoft_login(&flow_id)
        .await
        .map_err(login_error)
}

#[tauri::command]
pub fn cancel_microsoft_login(flow_id: String) -> Result<(), String> {
    cobble_auth::cancel_microsoft_login(&flow_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_microsoft_login(flow_id: String) -> Result<(), String> {
    let url = cobble_auth::microsoft_login_url(&flow_id).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || open::that(url).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}
