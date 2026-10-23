// SPDX-License-Identifier: MIT
//! Read-only, bounded previews of direct screenshots owned by a registered game.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageDecoder, ImageFormat, imageops::FilterType};
use std::{
    collections::VecDeque,
    io::{Cursor, Read},
    path::Path,
    sync::OnceLock,
};
const MAX_FILES: usize = 10_000;
const MAX_SOURCE: u64 = 32 * 1024 * 1024;
const MAX_PIXELS: u64 = 32 * 1024 * 1024;
const MAX_PREVIEW: usize = 8 * 1024 * 1024;
const CACHE_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Debug, Serialize)]
pub struct Screenshot {
    pub id: String,
    pub name: String,
    pub modified_at: Option<u64>,
    pub size_bytes: u64,
}
#[derive(Serialize)]
pub struct ScreenshotPage {
    pub items: Vec<Screenshot>,
    pub total: u32,
    pub next_offset: Option<u32>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ScreenshotPreview {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub original_width: u32,
    pub original_height: u32,
}
struct Handle {
    game: PathBuf,
    screenshot: Screenshot,
}
static HANDLES: OnceLock<Mutex<VecDeque<Handle>>> = OnceLock::new();
fn handles() -> &'static Mutex<VecDeque<Handle>> {
    HANDLES.get_or_init(Default::default)
}
struct Cached {
    game_id: String,
    id: String,
    thumbnail: bool,
    preview: ScreenshotPreview,
}
#[derive(Default)]
struct PreviewCache {
    items: VecDeque<Cached>,
    bytes: usize,
}
impl PreviewCache {
    fn get(&mut self, game: &str, id: &str, thumbnail: bool) -> Option<ScreenshotPreview> {
        let index = self.items.iter().position(|entry| {
            entry.game_id == game && entry.id == id && entry.thumbnail == thumbnail
        })?;
        let entry = self.items.remove(index)?;
        let preview = entry.preview.clone();
        self.items.push_back(entry);
        Some(preview)
    }
    fn insert(&mut self, entry: Cached) {
        let size = entry.preview.data_url.len();
        if size > CACHE_BYTES {
            return;
        }
        while self.bytes + size > CACHE_BYTES || self.items.len() >= 32 {
            let Some(old) = self.items.pop_front() else {
                break;
            };
            self.bytes -= old.preview.data_url.len();
        }
        self.bytes += size;
        self.items.push_back(entry);
    }
}
static CACHE: OnceLock<Mutex<PreviewCache>> = OnceLock::new();
static IMAGE_WORK: Mutex<()> = Mutex::new(());
static PREVIEW_REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(8);
fn cache() -> &'static Mutex<PreviewCache> {
    CACHE.get_or_init(Default::default)
}
fn linked(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
fn directory(game: &Path) -> Result<Option<PathBuf>, String> {
    for path in [game.to_path_buf(), game.join("screenshots")] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if linked(&metadata) || !metadata.is_dir() => {
                return Err("Screenshot folder must be a real directory".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("Cannot read screenshots folder".into()),
        }
    }
    Ok(Some(game.join("screenshots")))
}
fn image_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp"
            )
        })
}
fn entry(name: String, metadata: &fs::Metadata) -> Screenshot {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok());
    let signature = serde_json::to_vec(&(
        &name,
        metadata.len(),
        modified.map(|value| value.as_nanos()),
    ))
    .unwrap_or_default();
    Screenshot {
        id: cobble_plugin_host::digest(&signature),
        name,
        modified_at: modified.and_then(|value| u64::try_from(value.as_millis()).ok()),
        size_bytes: metadata.len(),
    }
}
fn inventory(game: &Path) -> Result<Vec<Screenshot>, String> {
    let Some(directory) = directory(game)? else {
        return Ok(Vec::new());
    };
    let mut images = Vec::new();
    for (count, item) in fs::read_dir(directory)
        .map_err(|_| "Cannot read screenshots folder")?
        .enumerate()
    {
        if count >= MAX_FILES {
            return Err("Screenshot folder has more than 10000 entries".into());
        }
        let item = item.map_err(|_| "Cannot read screenshot entry")?;
        let Some(name) = item.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !image_name(&name) {
            continue;
        }
        let metadata =
            fs::symlink_metadata(item.path()).map_err(|_| "Cannot read screenshot entry")?;
        if metadata.is_file() && !linked(&metadata) {
            images.push(entry(name, &metadata));
        }
    }
    images.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| b.name.cmp(&a.name))
    });
    Ok(images)
}
fn page(game: &Path, offset: u32, limit: u32) -> Result<ScreenshotPage, String> {
    if !(1..=80).contains(&limit) {
        return Err("Screenshot page size must be 1–80".into());
    }
    let items = inventory(game)?;
    let total = items.len() as u32;
    let selected = items
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect::<Vec<_>>();
    {
        let mut handles = handles()
            .lock()
            .map_err(|_| "Screenshot handles unavailable")?;
        for screenshot in &selected {
            handles
                .retain(|handle| !(handle.game == game && handle.screenshot.id == screenshot.id));
            while handles.len() >= 640 {
                handles.pop_front();
            }
            handles.push_back(Handle {
                game: game.to_path_buf(),
                screenshot: screenshot.clone(),
            });
        }
    }
    let end = offset.saturating_add(selected.len() as u32);
    Ok(ScreenshotPage {
        items: selected,
        total,
        next_offset: (end < total).then_some(end),
    })
}
fn selected(game: &Path, id: &str) -> Result<(PathBuf, Screenshot), String> {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid screenshot ID".into());
    }
    let screenshot = handles()
        .lock()
        .map_err(|_| "Screenshot handles unavailable")?
        .iter()
        .find(|handle| handle.game == game && handle.screenshot.id == id)
        .map(|handle| handle.screenshot.clone())
        .ok_or("Screenshot is no longer listed; refresh the gallery")?;
    let root = directory(game)?.ok_or("Screenshots folder is missing")?;
    let path = root.join(&screenshot.name);
    if !unchanged(&path, &screenshot) {
        return Err("Screenshot changed or was removed; refresh the gallery".into());
    }
    Ok((path, screenshot))
}
fn unchanged(path: &Path, screenshot: &Screenshot) -> bool {
    fs::symlink_metadata(path).ok().is_some_and(|metadata| {
        metadata.is_file()
            && !linked(&metadata)
            && entry(screenshot.name.clone(), &metadata).id == screenshot.id
    })
}
fn normalize(bytes: &[u8], thumbnail: bool) -> Result<ScreenshotPreview, String> {
    let format = image::guess_format(bytes).map_err(|_| "Screenshot is not a supported image")?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err("Screenshot is not a supported image".into());
    }
    let dimensions = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| "Cannot read screenshot dimensions")?;
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > 8192
        || dimensions.1 > 8192
        || u64::from(dimensions.0) * u64::from(dimensions.1) > MAX_PIXELS
    {
        return Err("Screenshot dimensions exceed preview limits; open the original image".into());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| "Cannot decode screenshot within preview limits")?;
    let orientation = decoder
        .orientation()
        .map_err(|_| "Cannot read screenshot orientation")?;
    let mut image = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| "Cannot decode screenshot within preview limits")?;
    image.apply_orientation(orientation);
    let dimensions = (image.width(), image.height());
    let maximum = if thumbnail { 320 } else { 1280 };
    let preview = if image.width() > maximum || image.height() > maximum {
        image.resize(maximum, maximum, FilterType::Triangle)
    } else {
        image
    };
    let (width, height) = (preview.width(), preview.height());
    let mut output = Cursor::new(Vec::new());
    preview
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|_| "Cannot create screenshot preview")?;
    let output = output.into_inner();
    if output.len() > MAX_PREVIEW {
        return Err("Screenshot preview exceeds its size limit".into());
    }
    Ok(ScreenshotPreview {
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(output)),
        width,
        height,
        original_width: dimensions.0,
        original_height: dimensions.1,
    })
}
fn preview(
    game: &Path,
    game_id: &str,
    id: &str,
    thumbnail: bool,
) -> Result<ScreenshotPreview, String> {
    let (path, screenshot) = selected(game, id)?;
    let cache_key = format!("{game_id}\0{}", game.display());
    if let Some(preview) = cache()
        .lock()
        .map_err(|_| "Screenshot cache unavailable")?
        .get(&cache_key, id, thumbnail)
    {
        return Ok(preview);
    }
    let _work = IMAGE_WORK
        .lock()
        .map_err(|_| "Screenshot decoder unavailable")?;
    if let Some(preview) = cache()
        .lock()
        .map_err(|_| "Screenshot cache unavailable")?
        .get(&cache_key, id, thumbnail)
    {
        return Ok(preview);
    }
    if screenshot.size_bytes > MAX_SOURCE {
        return Err("Screenshot exceeds the 32 MiB preview limit; open the original image".into());
    }
    if !unchanged(&path, &screenshot) {
        return Err("Screenshot changed; refresh the gallery".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .map_err(|_| "Cannot read screenshot")?
        .take(MAX_SOURCE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read screenshot")?;
    if bytes.len() as u64 > MAX_SOURCE || !unchanged(&path, &screenshot) {
        return Err("Screenshot changed or exceeds preview limits".into());
    }
    let preview = normalize(&bytes, thumbnail)?;
    cache()
        .lock()
        .map_err(|_| "Screenshot cache unavailable")?
        .insert(Cached {
            game_id: cache_key,
            id: id.into(),
            thumbnail,
            preview: preview.clone(),
        });
    Ok(preview)
}
fn owned_game(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    ensure_registered_game(app, id)?;
    game_directory(app, id)
}
/// Reuse the authorized bounded thumbnail/cache path; pixels stay in native memory.
pub(super) fn sheet_image(
    game: &Path,
    game_id: &str,
    id: &str,
) -> Result<(Screenshot, image::RgbaImage), String> {
    let (_, screenshot) = selected(game, id)?;
    let preview = preview(game, game_id, id, true)?;
    let encoded = preview
        .data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or("SHEET_RENDER_FAILED")?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "SHEET_RENDER_FAILED")?;
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|_| "SHEET_RENDER_FAILED")?;
    Ok((screenshot, image.thumbnail(256, 256).to_rgba8()))
}
#[cfg(test)]
pub(super) fn register_sheet_fixture(game: &Path) -> Result<Vec<Screenshot>, String> {
    Ok(page(game, 0, 80)?.items)
}
#[tauri::command]
pub async fn list_game_screenshots(
    app: AppHandle,
    id: String,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<ScreenshotPage, String> {
    let game = owned_game(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        page(&game, offset.unwrap_or(0), limit.unwrap_or(40))
    })
    .await
    .map_err(|_| "Screenshot listing failed")?
}
#[tauri::command]
pub async fn get_game_screenshot(
    app: AppHandle,
    id: String,
    screenshot_id: String,
    thumbnail: bool,
) -> Result<ScreenshotPreview, String> {
    let permit = PREVIEW_REQUESTS
        .try_acquire()
        .map_err(|_| "Screenshot previews are busy; retry shortly")?;
    let game = owned_game(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        preview(&game, &id, &screenshot_id, thumbnail)
    })
    .await
    .map_err(|_| "Screenshot preview failed")?
}
#[tauri::command]
pub async fn open_game_screenshot(
    app: AppHandle,
    id: String,
    screenshot_id: String,
) -> Result<(), String> {
    let game = owned_game(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (path, screenshot) = selected(&game, &screenshot_id)?;
        if !unchanged(&path, &screenshot) {
            return Err("Screenshot changed; refresh the gallery".into());
        }
        open::that(path).map_err(|_| "Cannot open screenshot".into())
    })
    .await
    .map_err(|_| "Screenshot open failed")?
}
#[tauri::command]
pub async fn open_game_screenshots_folder(app: AppHandle, id: String) -> Result<(), String> {
    let game = owned_game(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let directory = directory(&game)?.ok_or("Screenshots folder has not been created yet")?;
        open::that(directory).map_err(|_| "Cannot open screenshots folder".into())
    })
    .await
    .map_err(|_| "Screenshot folder open failed")?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn screenshot_root() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("screenshots")).unwrap();
        root
    }
    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba([35, 90, 150, 255]),
        ));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }
    #[test]
    fn screenshot_inventory_is_paged_read_only_and_ids_are_scoped_to_exact_game_files() {
        let missing = tempfile::tempdir().unwrap();
        assert_eq!(page(missing.path(), 0, 40).unwrap().total, 0);
        assert!(!missing.path().join("screenshots").exists());
        let root = screenshot_root();
        for name in ["one.png", "two.JPG", "three.webp"] {
            fs::write(
                root.path().join("screenshots").join(name),
                b"metadata-only fixture",
            )
            .unwrap();
        }
        fs::write(root.path().join("screenshots/notes.txt"), b"foreign").unwrap();
        fs::create_dir(root.path().join("screenshots/directory.png")).unwrap();
        let first = page(root.path(), 0, 2).unwrap();
        assert_eq!(first.total, 3);
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.next_offset, Some(2));
        assert_eq!(first.items[0].id.len(), 64);
        let last = page(root.path(), 2, 2).unwrap();
        assert_eq!(last.items.len(), 1);
        assert_eq!(last.next_offset, None);
        assert!(page(root.path(), 0, 81).is_err());
        assert!(selected(root.path(), "../notes.txt").is_err());
        assert!(selected(missing.path(), &first.items[0].id).is_err());
        let path = root.path().join("screenshots").join(&first.items[0].name);
        fs::write(path, b"changed generation with another size").unwrap();
        assert!(selected(root.path(), &first.items[0].id).is_err());
    }
    #[test]
    fn real_preview_preserves_aspect_ratio_bounds_decode_and_never_serves_changed_cached_file() {
        let root = screenshot_root();
        let path = root.path().join("screenshots/scene.png");
        fs::write(&path, png(640, 360)).unwrap();
        let item = page(root.path(), 0, 40).unwrap().items.remove(0);
        let thumbnail = preview(root.path(), "screenshot-preview-fixture", &item.id, true).unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (320, 180));
        assert_eq!(
            (thumbnail.original_width, thumbnail.original_height),
            (640, 360)
        );
        assert!(thumbnail.data_url.starts_with("data:image/png;base64,"));
        let full = preview(root.path(), "screenshot-preview-fixture", &item.id, false).unwrap();
        assert_eq!((full.width, full.height), (640, 360));
        assert!(normalize(b"not an image", true).is_err());
        assert!(normalize(&png(8193, 1), true).is_err());
        fs::write(&path, b"changed after cached thumbnail").unwrap();
        assert!(preview(root.path(), "screenshot-preview-fixture", &item.id, true).is_err());
        let oversized = root.path().join("screenshots/oversized.png");
        fs::File::create(&oversized)
            .unwrap()
            .set_len(MAX_SOURCE + 1)
            .unwrap();
        let item = page(root.path(), 0, 40)
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.name == "oversized.png")
            .unwrap();
        assert!(
            preview(root.path(), "screenshot-preview-fixture", &item.id, true)
                .unwrap_err()
                .contains("32 MiB")
        );
    }
    #[test]
    fn preview_cache_has_bounded_entry_and_byte_budgets() {
        let mut cache = PreviewCache::default();
        for i in 0..40 {
            cache.insert(Cached {
                game_id: "fixture".into(),
                id: i.to_string(),
                thumbnail: true,
                preview: ScreenshotPreview {
                    data_url: "small fixture".into(),
                    width: 1,
                    height: 1,
                    original_width: 1,
                    original_height: 1,
                },
            });
        }
        assert_eq!(cache.items.len(), 32);
        assert!(cache.get("fixture", "0", true).is_none());
        assert!(cache.get("fixture", "39", true).is_some());
        assert!(cache.bytes <= CACHE_BYTES);
    }
    #[cfg(windows)]
    #[test]
    fn screenshot_folder_junction_is_rejected_without_reading_foreign_images() {
        use std::os::windows::process::CommandExt;
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        fs::write(external.path().join("private.png"), b"untouched fixture").unwrap();
        let link = root.path().join("screenshots");
        let result = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(external.path())
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(page(root.path(), 0, 40).is_err());
        fs::remove_dir(link).unwrap();
        assert_eq!(
            fs::read(external.path().join("private.png")).unwrap(),
            b"untouched fixture"
        );
    }
    #[test]
    fn normalized_jpeg_applies_embedded_orientation_before_resizing() {
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            24,
            12,
            image::Rgb([20, 80, 160]),
        ));
        let mut jpeg = Cursor::new(Vec::new());
        image.write_to(&mut jpeg, ImageFormat::Jpeg).unwrap();
        let original = jpeg.into_inner();
        let payload = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut oriented = original[..2].to_vec();
        oriented.extend_from_slice(&[0xff, 0xe1]);
        oriented.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        oriented.extend_from_slice(payload);
        oriented.extend_from_slice(&original[2..]);
        let preview = normalize(&oriented, true).unwrap();
        assert_eq!((preview.width, preview.height), (12, 24));
        assert_eq!((preview.original_width, preview.original_height), (12, 24));
    }
}
