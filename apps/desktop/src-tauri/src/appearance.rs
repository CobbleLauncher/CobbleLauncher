// SPDX-License-Identifier: MIT
use cobble_auth::{ProfileAppearance, SkinPreview};
use std::{
    collections::HashMap,
    io::Read,
    sync::{Mutex, OnceLock},
};
use tauri::WebviewWindow;
static DRAFTS: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();

#[tauri::command]
pub async fn get_profile_appearance(
    id: String,
    refresh: Option<bool>,
) -> Result<ProfileAppearance, String> {
    cobble_auth::get_profile_appearance(&id, refresh.unwrap_or(false))
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub async fn pick_profile_skin(
    window: WebviewWindow,
    id: String,
) -> Result<Option<SkinPreview>, String> {
    cobble_auth::get_player_profile(Some(&id)).map_err(|error| error.to_string())?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择皮肤")
        .add_filter("PNG 皮肤", &["png"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    std::fs::File::open(file.path())
        .map_err(|error| error.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let preview =
        cobble_auth::preview_skin(&bytes, "classic").map_err(|error| error.to_string())?;
    DRAFTS
        .get_or_init(Default::default)
        .lock()
        .map_err(|error| error.to_string())?
        .insert(id, bytes);
    Ok(Some(preview))
}
#[tauri::command]
pub async fn apply_profile_skin(id: String, model: String) -> Result<ProfileAppearance, String> {
    let bytes = DRAFTS
        .get_or_init(Default::default)
        .lock()
        .map_err(|error| error.to_string())?
        .get(&id)
        .cloned()
        .ok_or("请先选择皮肤文件")?;
    let appearance = cobble_auth::save_profile_skin(&id, bytes, &model)
        .await
        .map_err(|error| error.to_string())?;
    DRAFTS
        .get_or_init(Default::default)
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&id);
    Ok(appearance)
}
