// SPDX-License-Identifier: MIT
//! Native-selected, bounded icon thumbnails. Renderer receives only opaque references and PNG data.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, imageops::FilterType};
use std::{
    io::{Cursor, Read, Write},
    path::Path,
};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ICON_BYTES: usize = 1024 * 1024;
const MAX_COVER_BYTES: usize = 4 * 1024 * 1024;
pub(super) fn default_preset() -> String {
    "grass".into()
}
fn valid_preset(value: &str) -> bool {
    matches!(
        value,
        "grass" | "stone" | "dirt" | "oak" | "water" | "lava" | "end"
    )
}
fn valid_ref(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn plain_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "无法读取图标目录")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("图标目录无效".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("图标目录无效".into());
        }
    }
    Ok(())
}
fn icon_root(create: bool) -> Result<PathBuf, String> {
    let root = dirs::data_local_dir()
        .ok_or("无法找到图标目录")?
        .join("CobbleLauncher/game-icons");
    if create {
        fs::create_dir_all(&root).map_err(|_| "无法创建图标目录")?;
    }
    if root.exists() {
        plain_directory(&root)?;
    }
    Ok(root)
}
#[cfg(test)]
fn normalize(bytes: &[u8]) -> Result<Vec<u8>, String> {
    normalize_size(bytes, 256, 256)
}
pub(crate) fn normalize_size(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("图片不能超过 8 MiB".into());
    }
    let format = image::guess_format(bytes).map_err(|_| "请选择 PNG、JPEG 或 WebP 图片")?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err("请选择 PNG、JPEG 或 WebP 图片".into());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "无法读取图片，请选择不超过 4096×4096 的有效图片")?;
    let thumbnail = image.resize_to_fill(width, height, FilterType::Triangle);
    let mut png = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|_| "无法生成图标")?;
    let bytes = png.into_inner();
    if bytes.len()
        > if width > 256 {
            MAX_COVER_BYTES
        } else {
            MAX_ICON_BYTES
        }
    {
        return Err("图片数据过大".into());
    }
    Ok(bytes)
}
fn selected_image(path: &Path) -> Result<Vec<u8>, String> {
    selected_image_size(path, 256, 256)
}
fn selected_image_size(path: &Path, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|_| "无法读取所选图片")?;
    if !file.metadata().map_err(|_| "无法读取所选图片")?.is_file() {
        return Err("请选择普通图片文件".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取所选图片")?;
    normalize_size(&bytes, width, height)
}
fn store_icon(root: &Path, bytes: &[u8]) -> Result<String, String> {
    plain_directory(root)?;
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|_| "无法生成图标编号")?;
    let id: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let path = root.join(format!("{id}.png"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| "无法保存图标")?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
        return Err("无法保存图标".into());
    }
    Ok(id)
}
fn read_icon(root: &Path, reference: &str) -> Result<String, String> {
    if !valid_ref(reference) {
        return Err("图标编号无效".into());
    }
    plain_directory(root)?;
    let path = root.join(format!("{reference}.png"));
    let metadata = fs::symlink_metadata(&path).map_err(|_| "无法读取图标")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("图标文件无效".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("图标文件无效".into());
        }
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "无法读取图标")?
        .take(MAX_COVER_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取图标")?;
    if bytes.len() > MAX_COVER_BYTES {
        return Err("图片数据过大".into());
    }
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}
fn change(app: &AppHandle, id: &str, update: impl FnOnce(&mut Game)) -> Result<Game, String> {
    let state = app.state::<GameLibrary>();
    let mut library = state.0.lock().map_err(|_| "游戏库不可用")?;
    if let Some(error) = &library.load_error {
        return Err(error.clone());
    }
    let index = library
        .games
        .iter()
        .position(|game| game.id == id)
        .ok_or("找不到这个游戏")?;
    let previous = library.games[index].clone();
    update(&mut library.games[index]);
    if let Err(error) = save(&library.games) {
        library.games[index] = previous;
        return Err(error);
    }
    let game = library.games[index].clone();
    drop(library);
    let _ = app.emit("game-install", &game);
    Ok(game)
}

#[derive(Serialize)]
pub struct GameIcon {
    id: String,
    icon_ref: String,
    data_url: String,
}

#[tauri::command]
pub fn set_game_appearance_preset(
    app: AppHandle,
    id: String,
    preset: String,
) -> Result<Game, String> {
    if !valid_preset(&preset) {
        return Err("请选择有效的游戏外观".into());
    }
    change(&app, &id, |game| {
        game.appearance_preset = preset;
        game.icon_ref = None;
    })
}
#[tauri::command]
pub fn reset_game_icon(app: AppHandle, id: String) -> Result<Game, String> {
    change(&app, &id, |game| game.icon_ref = None)
}
#[tauri::command]
pub async fn pick_game_icon(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
) -> Result<Option<Game>, String> {
    super::ensure_registered_game(&app, &id)?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择游戏图标")
        .add_filter("图片", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_owned();
    let reference = tauri::async_runtime::spawn_blocking(move || {
        let png = selected_image(&path)?;
        store_icon(&icon_root(true)?, &png)
    })
    .await
    .map_err(|_| "图标处理任务意外退出")??;
    change(&app, &id, |game| game.icon_ref = Some(reference)).map(Some)
}
#[tauri::command]
pub async fn get_game_icons(app: AppHandle, ids: Vec<String>) -> Result<Vec<GameIcon>, String> {
    if ids.len() > 64 {
        return Err("每次最多读取 64 个游戏图标".into());
    }
    let selected = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|_| "游戏库不可用")?;
        library
            .games
            .iter()
            .filter(|game| ids.contains(&game.id))
            .filter_map(|game| {
                game.icon_ref
                    .as_ref()
                    .map(|reference| (game.id.clone(), reference.clone()))
            })
            .collect::<Vec<_>>()
    };
    tauri::async_runtime::spawn_blocking(move || {
        let root = icon_root(false)?;
        Ok(selected
            .into_iter()
            .filter_map(|(id, icon_ref)| {
                read_icon(&root, &icon_ref).ok().map(|data_url| GameIcon {
                    id,
                    icon_ref,
                    data_url,
                })
            })
            .collect())
    })
    .await
    .map_err(|_| "图标读取任务意外退出")?
}

#[tauri::command]
pub fn set_game_pinned(app: AppHandle, id: String, pinned: bool) -> Result<Game, String> {
    change(&app, &id, |game| game.pinned = pinned)
}
#[tauri::command]
pub fn reset_game_cover(app: AppHandle, id: String) -> Result<Game, String> {
    change(&app, &id, |game| game.cover_ref = None)
}
#[tauri::command]
pub async fn pick_game_cover(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
) -> Result<Option<Game>, String> {
    super::ensure_registered_game(&app, &id)?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择游戏封面")
        .add_filter("图片", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_owned();
    let reference = tauri::async_runtime::spawn_blocking(move || {
        let png = selected_image_size(&path, 1200, 600)?;
        store_icon(&icon_root(true)?, &png)
    })
    .await
    .map_err(|_| "封面处理任务意外退出")??;
    change(&app, &id, |game| game.cover_ref = Some(reference)).map(Some)
}
#[derive(Serialize)]
pub struct GameCover {
    id: String,
    cover_ref: String,
    data_url: String,
}
#[tauri::command]
pub async fn get_game_covers(app: AppHandle, ids: Vec<String>) -> Result<Vec<GameCover>, String> {
    if ids.len() > 8 {
        return Err("每次最多读取 8 个游戏封面".into());
    }
    let selected = {
        let state = app.state::<GameLibrary>();
        let library = state.0.lock().map_err(|_| "游戏库不可用")?;
        library
            .games
            .iter()
            .filter(|game| ids.contains(&game.id))
            .filter_map(|game| {
                game.cover_ref
                    .as_ref()
                    .map(|value| (game.id.clone(), value.clone()))
            })
            .collect::<Vec<_>>()
    };
    tauri::async_runtime::spawn_blocking(move || {
        let root = icon_root(false)?;
        Ok(selected
            .into_iter()
            .filter_map(|(id, cover_ref)| {
                read_icon(&root, &cover_ref).ok().map(|data_url| GameCover {
                    id,
                    cover_ref,
                    data_url,
                })
            })
            .collect())
    })
    .await
    .map_err(|_| "封面读取任务意外退出")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_game_migrates_without_synthetic_launch_history() {
        let game:Game=serde_json::from_value(serde_json::json!({"id":"game","name":"Game","version":"1.21.1","directory":"synthetic","status":"installed","error":null})).unwrap();
        assert_eq!(game.last_launched_at, None);
        assert_eq!(game.appearance_preset, "grass");
        assert_eq!(game.icon_ref, None);
        assert_eq!(game.cover_ref, None);
        assert!(!game.pinned);
        assert!(valid_preset("water"));
        assert!(!valid_preset("../custom"));
        assert!(!valid_ref("../../private"));
    }
    #[test]
    fn image_is_normalized_and_owned_reference_cannot_read_paths() {
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
            let mut source = Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(500, 300)
                .write_to(&mut source, format)
                .unwrap();
            let png = normalize(source.get_ref()).unwrap();
            let decoded = image::load_from_memory(&png).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (256, 256));
        }
        let mut oversized = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(4097, 1)
            .write_to(&mut oversized, ImageFormat::Png)
            .unwrap();
        assert!(normalize(oversized.get_ref()).is_err());
        let mut input = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(500, 300)
            .write_to(&mut input, ImageFormat::Png)
            .unwrap();
        let png = normalize(input.get_ref()).unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (256, 256));
        let cover = normalize_size(input.get_ref(), 1200, 600).unwrap();
        let decoded_cover = image::load_from_memory(&cover).unwrap();
        assert_eq!((decoded_cover.width(), decoded_cover.height()), (1200, 600));
        let directory = tempfile::tempdir().unwrap();
        let reference = store_icon(directory.path(), &png).unwrap();
        assert!(valid_ref(&reference));
        assert!(
            read_icon(directory.path(), &reference)
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        assert!(read_icon(directory.path(), "../private").is_err());
        assert!(normalize(b"not an image").is_err());
        assert!(normalize(&vec![0; MAX_IMAGE_BYTES + 1]).is_err());
    }
}
