// SPDX-License-Identifier: MIT
//! Window-owned immutable image-layout proposals; pixels/output paths never reach Wasm.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use cobble_plugin_api::{
    ImageLayoutRequest, ImageOrdering, ImageSheet, InterfaceRequirement, LayoutImage,
    PluginManifest,
};
use cobble_plugin_host::PluginRegistry;
use image::RgbaImage;
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::Path,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::WebviewWindow;
#[path = "screenshot_sheet_render.rs"]
mod render;
const MAX_PNG: usize = 8 * 1024 * 1024;
static STORE: OnceLock<Mutex<HashMap<String, Ticket>>> = OnceLock::new();
static WORK: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
static SAVES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
struct Source {
    descriptor: LayoutImage,
    pixels: RgbaImage,
}
struct Artifact {
    id: String,
    manifest: PluginManifest,
    game_id: String,
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    sources: Vec<PathBuf>,
}
struct Ticket {
    request_id: String,
    plugin_id: String,
    cancel: Arc<AtomicBool>,
    artifact: Option<Arc<Artifact>>,
}
#[derive(Serialize)]
pub struct Preview {
    preview_id: String,
    data_url: String,
    width: u32,
    height: u32,
}
#[derive(Serialize)]
pub struct Choice {
    plugin_id: String,
    name: String,
}
fn store() -> &'static Mutex<HashMap<String, Ticket>> {
    STORE.get_or_init(Default::default)
}
fn registry() -> Result<PluginRegistry, String> {
    Ok(PluginRegistry::new(registry_path()?)
        .with_trusted_roots(crate::plugins::plugin_signatures::trusted_roots()?))
}
fn registry_path() -> Result<PathBuf, String> {
    Ok(dirs::data_local_dir()
        .ok_or("PLUGIN_UNAVAILABLE")?
        .join("CobbleLauncher/plugins"))
}
fn requirement() -> InterfaceRequirement {
    InterfaceRequirement {
        id: cobble_plugin_api::IMAGE_LAYOUT_INTERFACE.into(),
        version: "^0.1.0".into(),
    }
}
fn selected(
    registry: &PluginRegistry,
    id: &str,
) -> Result<cobble_plugin_api::InstalledPlugin, String> {
    let plugin = registry
        .resolve_enabled(id)
        .map_err(|_| "PLUGIN_UNAVAILABLE")?;
    if !plugin.manifest.supports_interface(&requirement())
        || plugin.manifest.capabilities != ["image-layout"]
    {
        return Err("PLUGIN_UNAVAILABLE".into());
    }
    Ok(plugin)
}
fn verify(manifest: &PluginManifest) -> Result<(), String> {
    if selected(&registry()?, &manifest.id)?.manifest != *manifest {
        return Err("PLUGIN_CHANGED".into());
    }
    Ok(())
}
#[tauri::command]
pub fn list_screenshot_sheet_plugins() -> Result<Vec<Choice>, String> {
    Ok(registry()?
        .resolve_interface(&requirement(), None)
        .map_err(|_| "PLUGIN_UNAVAILABLE")?
        .implementations
        .into_iter()
        .map(|p| Choice {
            plugin_id: p.plugin_id,
            name: p.name,
        })
        .collect())
}
fn revoke_matching(window: &str, request_id: Option<&str>) {
    if let Ok(mut store) = store().lock()
        && store
            .get(window)
            .is_some_and(|t| request_id.is_none_or(|id| id == t.request_id))
        && let Some(ticket) = store.remove(window)
    {
        ticket.cancel.store(true, Ordering::Release);
    }
}
pub(crate) fn revoke_window(window: &str) {
    revoke_matching(window, None)
}
pub(crate) fn revoke_plugin(id: &str) {
    if let Ok(mut store) = store().lock() {
        store.retain(|_, ticket| {
            if ticket.plugin_id == id {
                ticket.cancel.store(true, Ordering::Release);
                false
            } else {
                true
            }
        });
    }
}
#[tauri::command]
pub fn cancel_screenshot_sheet(window: WebviewWindow, request_id: String) -> Result<(), String> {
    revoke_matching(window.label(), Some(&request_id));
    Ok(())
}
fn valid_request(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
fn begin(window: &str, request_id: &str, plugin_id: &str) -> Result<Arc<AtomicBool>, String> {
    if !valid_request(request_id) {
        return Err("INVALID_SELECTION".into());
    }
    let mut store = store().lock().map_err(|_| "SHEET_BUSY")?;
    if !store.contains_key(window) && store.len() >= 4 {
        return Err("SHEET_BUSY".into());
    }
    if let Some(previous) = store.remove(window) {
        previous.cancel.store(true, Ordering::Release);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    store.insert(
        window.into(),
        Ticket {
            request_id: request_id.into(),
            plugin_id: plugin_id.into(),
            cancel: cancel.clone(),
            artifact: None,
        },
    );
    Ok(cancel)
}
fn prepare(
    game: &Path,
    game_id: &str,
    ids: &[String],
    locale: String,
    columns: u8,
    ordering: ImageOrdering,
    cancel: &AtomicBool,
) -> Result<(Vec<Source>, ImageLayoutRequest, Vec<PathBuf>), String> {
    if ids.is_empty()
        || ids.len() > 12
        || ids.iter().collect::<HashSet<_>>().len() != ids.len()
        || columns > 4
        || !matches!(locale.as_str(), "en" | "zh-CN")
    {
        return Err("INVALID_SELECTION".into());
    }
    let mut sources = Vec::new();
    let mut paths = Vec::new();
    for id in ids {
        if cancel.load(Ordering::Acquire) {
            return Err("CANCEL_REQUESTED".into());
        }
        let (metadata, pixels) =
            super::screenshots::sheet_image(game, game_id, id).map_err(|_| "SCREENSHOT_CHANGED")?;
        let mut name = String::new();
        for ch in metadata.name.chars() {
            if name.len() + ch.len_utf8() > 160 {
                break;
            }
            name.push(if ch.is_control() || matches!(ch, '/' | '\\') {
                '_'
            } else {
                ch
            });
        }
        paths.push(game.join("screenshots").join(&metadata.name));
        sources.push(Source {
            descriptor: LayoutImage {
                id: id.clone(),
                name,
                width: pixels.width() as u16,
                height: pixels.height() as u16,
                modified_at: metadata.modified_at,
            },
            pixels,
        });
    }
    let request = ImageLayoutRequest {
        locale,
        images: sources.iter().map(|s| s.descriptor.clone()).collect(),
        columns,
        ordering,
    };
    request.validate_bounds().map_err(|_| "INVALID_SELECTION")?;
    Ok((sources, request, paths))
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn preview_screenshot_sheet(
    app: AppHandle,
    window: WebviewWindow,
    game_id: String,
    screenshot_ids: Vec<String>,
    plugin_id: String,
    request_id: String,
    columns: u8,
    ordering: ImageOrdering,
    locale: Option<String>,
) -> Result<Option<Preview>, String> {
    let game = game_directory(&app, &game_id)?;
    ensure_registered_game(&app, &game_id)?;
    let _permit = WORK.try_acquire().map_err(|_| "SHEET_BUSY")?;
    let cancel = begin(window.label(), &request_id, &plugin_id)?;
    let worker_cancel = cancel.clone();
    let worker_game_id = game_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let host = std::env::current_exe()
            .map_err(|_| "PLUGIN_UNAVAILABLE")?
            .with_file_name(if cfg!(windows) {
                "cobble-plugin-host.exe"
            } else {
                "cobble-plugin-host"
            });
        compose_owned(
            &registry_path()?,
            &host,
            ComposeJob {
                game,
                game_id: worker_game_id,
                screenshot_ids,
                plugin_id,
                locale: locale.unwrap_or_else(|| "en".into()),
                columns,
                ordering,
            },
            worker_cancel,
        )
    })
    .await
    .map_err(|_| "SHEET_RENDER_FAILED")?;
    if cancel.load(Ordering::Acquire) {
        return Ok(None);
    }
    let artifact = match result {
        Ok(a) => Arc::new(a),
        Err(error) => {
            revoke_matching(window.label(), Some(&request_id));
            return Err(error);
        }
    };
    ensure_registered_game(&app, &game_id)?;
    verify(&artifact.manifest)?;
    let preview = Preview {
        preview_id: artifact.id.clone(),
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(&artifact.bytes)),
        width: artifact.width,
        height: artifact.height,
    };
    let mut store = store().lock().map_err(|_| "SHEET_BUSY")?;
    let Some(ticket) = store
        .get_mut(window.label())
        .filter(|t| t.request_id == request_id && Arc::ptr_eq(&t.cancel, &cancel))
    else {
        return Ok(None);
    };
    if cancel.load(Ordering::Acquire) {
        return Ok(None);
    }
    ticket.artifact = Some(artifact);
    Ok(Some(preview))
}
fn artifact(window: &str, id: &str) -> Result<(Arc<Artifact>, Arc<AtomicBool>), String> {
    let store = store().lock().map_err(|_| "SHEET_BUSY")?;
    let ticket = store.get(window).ok_or("SHEET_EXPIRED")?;
    let artifact = ticket
        .artifact
        .as_ref()
        .filter(|a| a.id == id)
        .ok_or("SHEET_EXPIRED")?;
    if ticket.cancel.load(Ordering::Acquire) {
        return Err("SHEET_EXPIRED".into());
    }
    Ok((artifact.clone(), ticket.cancel.clone()))
}
#[tauri::command]
pub async fn save_screenshot_sheet(
    app: AppHandle,
    window: WebviewWindow,
    preview_id: String,
) -> Result<bool, String> {
    let _permit = SAVES.try_acquire().map_err(|_| "SHEET_BUSY")?;
    let (artifact, cancel) = artifact(window.label(), &preview_id)?;
    ensure_registered_game(&app, &artifact.game_id)?;
    verify(&artifact.manifest)?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("保存截图拼图")
        .set_file_name("screenshots.png")
        .add_filter("PNG", &["png"])
        .save_file()
        .await
    else {
        return Ok(false);
    };
    if cancel.load(Ordering::Acquire) {
        return Ok(false);
    }
    ensure_registered_game(&app, &artifact.game_id)?;
    verify(&artifact.manifest)?;
    let path = file.path().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || save(&path, &artifact, &cancel))
        .await
        .map_err(|_| "SHEET_SAVE_FAILED")?
}
fn save(path: &Path, artifact: &Artifact, cancel: &AtomicBool) -> Result<bool, String> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_none_or(|e| !e.eq_ignore_ascii_case("png"))
    {
        return Err("SHEET_SAVE_FAILED".into());
    }
    if let Ok(target) = path.canonicalize()
        && artifact
            .sources
            .iter()
            .filter_map(|p| p.canonicalize().ok())
            .any(|source| same_path(&source, &target))
    {
        return Err("OUTPUT_IS_SOURCE".into());
    }
    if cancel.load(Ordering::Acquire) {
        return Ok(false);
    }
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().ok_or("SHEET_SAVE_FAILED")?)
        .map_err(|_| "SHEET_SAVE_FAILED")?;
    temporary
        .write_all(&artifact.bytes)
        .map_err(|_| "SHEET_SAVE_FAILED")?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "SHEET_SAVE_FAILED")?;
    if cancel.load(Ordering::Acquire) {
        return Ok(false);
    }
    temporary.persist(path).map_err(|_| "SHEET_SAVE_FAILED")?;
    Ok(true)
}
fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}

struct ComposeJob {
    game: PathBuf,
    game_id: String,
    screenshot_ids: Vec<String>,
    plugin_id: String,
    locale: String,
    columns: u8,
    ordering: ImageOrdering,
}
fn compose_owned(
    root: &Path,
    host: &Path,
    job: ComposeJob,
    cancel: Arc<AtomicBool>,
) -> Result<Artifact, String> {
    let ComposeJob {
        game,
        game_id,
        screenshot_ids,
        plugin_id,
        locale,
        columns,
        ordering,
    } = job;
    let registry = PluginRegistry::new(root.to_path_buf())
        .with_trusted_roots(crate::plugins::plugin_signatures::trusted_roots()?);
    let plugin = selected(&registry, &plugin_id)?;
    let (sources, request, paths) = prepare(
        &game,
        &game_id,
        &screenshot_ids,
        locale,
        columns,
        ordering,
        &cancel,
    )?;
    request
        .validate(&plugin.manifest)
        .map_err(|_| "PLUGIN_UNAVAILABLE")?;
    let (manifest, component) = registry
        .read_installed_package(&plugin.manifest.id)
        .map_err(|_| "PLUGIN_UNAVAILABLE")?;
    if manifest != plugin.manifest {
        return Err("PLUGIN_CHANGED".into());
    }
    let sheet =
        cobble_plugin_host::run_image_layout(host, &component, &manifest, &request, cancel.clone())
            .map_err(|_| "SHEET_RENDER_FAILED")?;
    if cancel.load(Ordering::Acquire) {
        return Err("CANCEL_REQUESTED".into());
    }
    if selected(&registry, &manifest.id)?.manifest != manifest {
        return Err("PLUGIN_CHANGED".into());
    }
    let (bytes, width, height) = render::render(&sources, &request, &sheet, &cancel)?;
    Ok(Artifact {
        id: next_pack_id()?,
        manifest,
        game_id,
        bytes,
        width,
        height,
        sources: paths,
    })
}
#[cfg(test)]
#[path = "screenshot_sheet_tests.rs"]
mod tests;
