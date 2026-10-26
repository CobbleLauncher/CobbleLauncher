#[path = "plugin_generated_blueprint.rs"]
pub mod plugin_generated_blueprint;
// SPDX-License-Identifier: MIT
#[path = "blueprint_export.rs"]
pub mod blueprint_export;
#[path = "blueprint_library.rs"]
pub mod blueprint_library;
#[path = "blueprint_material_handoffs.rs"]
pub mod blueprint_material_handoffs;
#[path = "blueprint_provider.rs"]
pub mod blueprint_provider;
#[path = "blueprint_viewers.rs"]
pub mod blueprint_viewers;
#[path = "bundled_plugins.rs"]
pub mod bundled_plugins;
#[path = "extension_contributions.rs"]
pub mod extension_contributions;
#[path = "extension_settings.rs"]
pub mod extension_settings;
#[path = "pack_inspection.rs"]
pub mod pack_inspection;
#[path = "pack_inspection_pages.rs"]
pub mod pack_inspection_pages;
#[path = "plugin_access_log.rs"]
pub mod plugin_access_log;
#[path = "plugin_ai.rs"]
pub mod plugin_ai;
#[path = "plugin_composers.rs"]
pub mod plugin_composers;
#[path = "plugin_config_workspace.rs"]
pub mod plugin_config_workspace;
#[path = "plugin_confirm.rs"]
pub mod plugin_confirm;
#[path = "plugin_data.rs"]
pub mod plugin_data;
#[path = "plugin_details.rs"]
pub mod plugin_details;
#[path = "plugin_file_inbox.rs"]
pub mod plugin_file_inbox;
#[path = "plugin_frontend_bridge.rs"]
pub mod plugin_frontend_bridge;
#[path = "plugin_frontend_exports.rs"]
pub mod plugin_frontend_exports;
#[path = "plugin_frontend_handoffs.rs"]
pub mod plugin_frontend_handoffs;
#[path = "plugin_handoffs.rs"]
pub mod plugin_handoffs;
#[path = "plugin_lang_package.rs"]
pub mod plugin_lang_package;
#[path = "plugin_material_handoffs.rs"]
pub mod plugin_material_handoffs;
#[path = "plugin_mesh_events.rs"]
pub mod plugin_mesh_events;
#[path = "plugin_play_manifest.rs"]
pub mod plugin_play_manifest;
#[path = "plugin_recovery_workspace.rs"]
pub mod plugin_recovery_workspace;
#[path = "plugin_reports.rs"]
pub mod plugin_reports;
#[path = "plugin_signatures.rs"]
pub mod plugin_signatures;
#[path = "plugin_views.rs"]
pub mod plugin_views;
#[path = "tool_forms.rs"]
pub mod tool_forms;
#[path = "viewer_lifecycle.rs"]
mod viewer_lifecycle;
#[path = "viewer_textures.rs"]
pub mod viewer_textures;
use cobble_plugin_api::{InstalledPlugin, MAX_FILE_BYTES, PluginManifest, SceneChunk, ViewerScene};
use cobble_plugin_host::{PluginRegistry, PluginSession};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Read,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::WebviewWindow;
fn root(name: &str) -> Result<PathBuf, String> {
    Ok(dirs::data_local_dir()
        .ok_or("无法找到数据目录")?
        .join("CobbleLauncher")
        .join(name))
}
fn registry() -> Result<PluginRegistry, String> {
    let registry = PluginRegistry::new(root("plugins")?)
        .with_trusted_roots(plugin_signatures::trusted_roots()?);
    let bundled = super::runtime_resources::bundled_plugins()?.join("schematic-viewer");
    #[cfg(debug_assertions)]
    let bundled = if bundled.exists() {
        bundled
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/schematic-viewer-package")
    };
    let seeded = root("plugins")?.join(".bundled-seeded");
    blueprint_viewers::seed_default(&registry, &bundled, &seeded)?;
    Ok(registry)
}
fn next_id() -> String {
    format!(
        "{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
fn valid_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        Err("资源标识无效".into())
    } else {
        Ok(())
    }
}
pub(crate) fn saved_blueprint(id: &str) -> Result<(String, Vec<u8>, BlueprintFormat), String> {
    valid_id(id)?;
    let directory = root("blueprints")?;
    let blueprint: Blueprint =
        serde_json::from_slice(&read_file(&directory.join(format!("{id}.json")))?)
            .map_err(|error| error.to_string())?;
    if blueprint.id != id {
        return Err("蓝图标识不匹配".into());
    }
    Ok((
        blueprint.name,
        read_file(&directory.join(format!("{id}.{}", blueprint.format.extension())))?,
        blueprint.format,
    ))
}
#[derive(Clone)]
struct PendingPackage {
    path: PathBuf,
    manifest: PluginManifest,
    signature: cobble_plugin_host::SignatureStatus,
    _staging: Option<Arc<tempfile::TempDir>>,
}
#[derive(Default)]
struct PackageApproval {
    revision: u64,
    pending: Option<PendingPackage>,
}
impl PackageApproval {
    fn begin(&mut self) -> u64 {
        self.revision = self.revision.wrapping_add(1);
        self.pending = None;
        self.revision
    }
    fn approve(
        &mut self,
        revision: u64,
        package: PendingPackage,
    ) -> Result<PluginManifest, String> {
        if self.revision != revision {
            return Err("插件预览已取消，请重新选择".into());
        }
        let manifest = package.manifest.clone();
        self.pending = Some(package);
        Ok(manifest)
    }
}
static PACKAGE: OnceLock<Mutex<PackageApproval>> = OnceLock::new();
pub(super) fn begin_plugin_preview() -> Result<u64, String> {
    Ok(PACKAGE
        .get_or_init(Default::default)
        .lock()
        .map_err(|error| error.to_string())?
        .begin())
}
#[tauri::command]
pub fn cancel_plugin_preview() -> Result<(), String> {
    begin_plugin_preview().map(|_| ())
}
#[cfg(test)]
fn staged_package(manifest: PluginManifest, bytes: &[u8]) -> Result<PendingPackage, String> {
    staged_package_with_icon(manifest, bytes, None)
}
fn staged_package_with_icon(
    manifest: PluginManifest,
    bytes: &[u8],
    icon_bytes: Option<&[u8]>,
) -> Result<PendingPackage, String> {
    manifest.validate().map_err(|error| error.to_string())?;
    if bytes.len() > cobble_plugin_api::MAX_COMPONENT_BYTES
        || cobble_plugin_host::digest(bytes) != manifest.component_sha256.to_ascii_lowercase()
    {
        return Err("插件组件与目录中的 SHA-256 不匹配，或超过大小限制".into());
    }
    let staging = Arc::new(
        tempfile::Builder::new()
            .prefix("cobble-plugin-preview-")
            .tempdir()
            .map_err(|error| error.to_string())?,
    );
    std::fs::write(
        staging.path().join("plugin.json"),
        serde_json::to_vec(&manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(staging.path().join("component.wasm"), bytes)
        .map_err(|error| error.to_string())?;
    if let Some(icon) = &manifest.icon {
        let bytes = icon_bytes.ok_or("Plugin catalog icon is missing")?;
        cobble_plugin_host::validate_icon_png(bytes, &icon.sha256).map_err(|e| e.to_string())?;
        std::fs::write(staging.path().join(&icon.file), bytes).map_err(|e| e.to_string())?;
    }
    cobble_plugin_host::read_package(staging.path()).map_err(|error| error.to_string())?;
    Ok(PendingPackage {
        path: staging.path().to_owned(),
        manifest,
        signature: cobble_plugin_host::SignatureStatus::Unsigned,
        _staging: Some(staging),
    })
}
pub(super) fn prepare_catalog_package(
    manifest: PluginManifest,
    bytes: &[u8],
    icon_bytes: Option<&[u8]>,
    revision: u64,
) -> Result<PluginManifest, String> {
    let package = staged_package_with_icon(manifest, bytes, icon_bytes)?;
    PACKAGE
        .get_or_init(Default::default)
        .lock()
        .map_err(|error| error.to_string())?
        .approve(revision, package)
}
pub(super) fn prepare_catalog_bundle(
    manifest: PluginManifest,
    artifact: &cobble_plugin_host::catalog::CatalogPackage,
    bytes: &[u8],
    revision: u64,
) -> Result<PluginManifest, String> {
    let roots = plugin_signatures::trusted_roots()?;
    let (staging, signature) =
        crate::catalog_install::extract_bundle(&manifest, artifact, bytes, &roots)?;
    PACKAGE
        .get_or_init(Default::default)
        .lock()
        .map_err(|error| error.to_string())?
        .approve(
            revision,
            PendingPackage {
                path: staging.path().to_owned(),
                manifest,
                signature,
                _staging: Some(staging),
            },
        )
}
#[derive(Clone)]
struct PendingBlueprint {
    queue_id: String,
    path: PathBuf,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingBlueprintDescriptor {
    queue_id: String,
    name: String,
    format: BlueprintFormat,
}
static PENDING_FILE: OnceLock<Mutex<Option<PendingBlueprint>>> = OnceLock::new();
#[tauri::command]
pub fn peek_pending_schematic() -> Result<Option<PendingBlueprintDescriptor>, String> {
    PENDING_FILE
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|pending| {
            let name = pending
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            Ok(PendingBlueprintDescriptor {
                queue_id: pending.queue_id.clone(),
                format: BlueprintFormat::from_name(&name)?,
                name,
            })
        })
        .transpose()
}
fn clear_pending(pending: &mut Option<PendingBlueprint>, expected: &str) {
    if pending
        .as_ref()
        .is_some_and(|current| current.queue_id == expected)
    {
        *pending = None;
    }
}
pub fn queue_file(arguments: impl Iterator<Item = String>) -> bool {
    if let Some(path) = arguments.map(PathBuf::from).find(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "schem" | "litematic"
                )
            })
            && path.is_file()
    }) && let Ok(mut pending) = PENDING_FILE.get_or_init(Default::default).lock()
    {
        *pending = Some(PendingBlueprint {
            queue_id: next_id(),
            path,
        });
        return true;
    }
    false
}
#[tauri::command]
pub async fn take_pending_schematic(
    plugin_id: Option<String>,
    expected_queue_id: Option<String>,
) -> Result<Option<OpenViewer>, String> {
    let path = PENDING_FILE
        .get_or_init(Default::default)
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let Some(pending) = path else {
        return Ok(None);
    };
    if expected_queue_id
        .as_deref()
        .is_some_and(|id| id != pending.queue_id)
    {
        return Err("Pending blueprint changed; choose again".into());
    }
    let path = pending.path.clone();
    let name = path
        .file_name()
        .ok_or("蓝图名称无效")?
        .to_string_lossy()
        .into_owned();
    let format = BlueprintFormat::from_name(&name)?;
    let pending_id = pending.queue_id;
    tauri::async_runtime::spawn_blocking(move || {
        let opened = open(read_file(&path)?, name, plugin_id, format)?;
        if let Ok(mut pending) = PENDING_FILE.get_or_init(Default::default).lock() {
            clear_pending(&mut pending, &pending_id);
        }
        Ok(Some(opened))
    })
    .await
    .map_err(|error| error.to_string())?
}
struct Resource {
    session: Mutex<PluginSession>,
    cancel: Arc<AtomicBool>,
    bytes: Vec<u8>,
    name: String,
    plugin_id: String,
    viewer_manifest: PluginManifest,
    format: BlueprintFormat,
    source: Option<BlueprintSource>,
}
static RESOURCES: OnceLock<Mutex<HashMap<String, Arc<Resource>>>> = OnceLock::new();
fn resources() -> &'static Mutex<HashMap<String, Arc<Resource>>> {
    RESOURCES.get_or_init(Default::default)
}
fn resource(id: &str) -> Result<Arc<Resource>, String> {
    resource_in(&registry()?, id)
}
pub(crate) fn viewer_texture_palette(id: &str) -> Result<Vec<String>, String> {
    let active = resource(id)?;
    let session = active.session.lock().map_err(|_| "VIEWER_UNAVAILABLE")?;
    Ok(session.summary().palette.clone())
}
fn resource_in(registry: &PluginRegistry, id: &str) -> Result<Arc<Resource>, String> {
    let resource = resources()
        .lock()
        .map_err(|e| e.to_string())?
        .get(id)
        .cloned()
        .ok_or("蓝图已关闭，请重新打开")?;
    if !resource_current_in(registry, &resource) {
        let _ = close_viewer(id.to_string());
        return Err("Viewer is disabled or changed; reopen this blueprint".into());
    }
    Ok(resource)
}
fn resource_current_in(registry: &PluginRegistry, resource: &Resource) -> bool {
    !resource.cancel.load(Ordering::Acquire)
        && registry
            .validate_viewer_session(&resource.viewer_manifest)
            .is_ok()
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenViewer {
    resource_id: String,
    name: String,
    scene: ViewerScene,
    format: BlueprintFormat,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blueprint {
    id: String,
    name: String,
    plugin_id: String,
    #[serde(default)]
    format: BlueprintFormat,
    #[serde(default)]
    favorite: bool,
    #[serde(default)]
    has_thumbnail: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<BlueprintSource>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct BlueprintSource {
    provider: String,
    id: String,
    version: u32,
    author: String,
    attribution: String,
    license: Option<String>,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum BlueprintFormat {
    #[default]
    Schem,
    Litematic,
}
impl BlueprintFormat {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Schem => "schem",
            Self::Litematic => "litematic",
        }
    }
    fn from_name(name: &str) -> Result<Self, String> {
        match std::path::Path::new(name)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("schem") => Ok(Self::Schem),
            Some("litematic") => Ok(Self::Litematic),
            _ => Err("不支持的蓝图文件类型".into()),
        }
    }
}

#[tauri::command]
pub fn get_plugin_icon(id: String, bundled: Option<bool>) -> Result<Option<String>, String> {
    use base64::Engine;
    let icon = if bundled.unwrap_or(false) {
        bundled_plugins::icon(&id)
    } else {
        registry()?.icon(&id).map_err(|e| e.to_string())
    }?;
    Ok(icon.map(|bytes| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }))
}
#[tauri::command]
pub fn list_plugins() -> Result<Vec<InstalledPlugin>, String> {
    registry()?.list().map_err(|error| error.to_string())
}
#[tauri::command]
pub async fn preview_plugin_package(
    window: WebviewWindow,
) -> Result<Option<PluginManifest>, String> {
    let revision = begin_plugin_preview()?;
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("选择插件包目录")
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    prepare_directory_package(folder.path().to_owned(), revision).map(Some)
}
pub(super) fn prepare_directory_package(
    path: PathBuf,
    revision: u64,
) -> Result<PluginManifest, String> {
    let (manifest, _) =
        plugin_signatures::read_package(&path).map_err(|error| error.to_string())?;
    let roots = plugin_signatures::trusted_roots()?;
    let signature = cobble_plugin_host::package_signature_status(&path, Some(&roots))
        .map_err(|error| error.code)?;
    PACKAGE
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "PLUGIN_PREVIEW_UNAVAILABLE")?
        .approve(
            revision,
            PendingPackage {
                path,
                manifest,
                signature,
                _staging: None,
            },
        )
}
#[tauri::command]
pub fn install_plugin(
    id: String,
    reviewed_manifest: PluginManifest,
) -> Result<Vec<InstalledPlugin>, String> {
    apply_approved_package(id, false, reviewed_manifest)
}
#[tauri::command]
pub fn update_plugin(
    id: String,
    reviewed_manifest: PluginManifest,
) -> Result<Vec<InstalledPlugin>, String> {
    apply_approved_package(id, true, reviewed_manifest)
}
fn apply_approved_package(
    id: String,
    update: bool,
    reviewed_manifest: PluginManifest,
) -> Result<Vec<InstalledPlugin>, String> {
    let (revision, package) = {
        let approval = PACKAGE
            .get_or_init(Default::default)
            .lock()
            .map_err(|error| error.to_string())?;
        (
            approval.revision,
            approval.pending.clone().ok_or("请重新选择插件包")?,
        )
    };
    let path = &package.path;
    let approved = &package.manifest;
    let (current, _) = plugin_signatures::read_package(path).map_err(|error| error.to_string())?;
    let roots = plugin_signatures::trusted_roots()?;
    if cobble_plugin_host::package_signature_status(path, Some(&roots))
        .map_err(|error| error.code)?
        != package.signature
    {
        return Err("PLUGIN_REVIEW_MISMATCH".into());
    }
    if id != approved.id || approved != &current || approved != &reviewed_manifest {
        return Err("PLUGIN_REVIEW_MISMATCH".into());
    }
    let registry = registry()?;
    let mut viewer_lifecycle = viewer_lifecycle::lock()?;
    if update {
        viewer_lifecycle.revoke(&id);
        close_plugin_resources(&id);
        registry.update_directory_approved(path, &reviewed_manifest)
    } else {
        registry.install_directory(path)
    }
    .map_err(|error| error.to_string())?;
    if let Ok(mut pending) = PACKAGE.get_or_init(Default::default).lock()
        && pending.revision == revision
    {
        pending.pending = None;
    }
    tracing::info!(event = "plugin_package_applied", plugin_id = %id, version = %approved.version, update);
    list_plugins()
}
fn close_plugin_resources(id: &str) {
    blueprint_provider::revoke_plugin(id);
    plugin_views::revoke_plugin(id);
    pack_inspection_pages::revoke_plugin(id);
    plugin_reports::cancel_for_plugin(id);
    tool_forms::cancel_for_plugin(id);
    if let Ok(mut resources) = resources().lock() {
        resources.retain(|_, resource| {
            if resource.plugin_id == id {
                resource.cancel.store(true, Ordering::Relaxed);
                false
            } else {
                true
            }
        });
    }
}
#[tauri::command]
pub fn set_plugin_enabled(id: String, enabled: bool) -> Result<Vec<InstalledPlugin>, String> {
    let mut viewer_lifecycle = viewer_lifecycle::lock()?;
    if !enabled {
        viewer_lifecycle.revoke(&id);
    }
    if !enabled {
        close_plugin_resources(&id);
    }
    registry()?
        .set_enabled(&id, enabled)
        .map_err(|error| error.to_string())?;
    tracing::info!(event = "plugin_enabled_changed", plugin_id = %id, enabled);
    list_plugins()
}
#[tauri::command]
pub fn uninstall_plugin(id: String) -> Result<Vec<InstalledPlugin>, String> {
    let mut viewer_lifecycle = viewer_lifecycle::lock()?;
    viewer_lifecycle.revoke(&id);
    close_plugin_resources(&id);
    registry()?
        .uninstall(&id)
        .map_err(|error| error.to_string())?;
    tracing::info!(event = "plugin_uninstalled", plugin_id = %id);
    list_plugins()
}
fn read_file(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err("蓝图文件超过支持大小".into());
    }
    Ok(bytes)
}
fn open(
    bytes: Vec<u8>,
    name: String,
    plugin_id: Option<String>,
    format: BlueprintFormat,
) -> Result<OpenViewer, String> {
    open_with_cancel(
        bytes,
        name,
        plugin_id,
        format,
        None,
        Arc::new(AtomicBool::new(false)),
    )
}
fn open_with_cancel(
    bytes: Vec<u8>,
    name: String,
    plugin_id: Option<String>,
    format: BlueprintFormat,
    source: Option<BlueprintSource>,
    cancel: Arc<AtomicBool>,
) -> Result<OpenViewer, String> {
    let registry = registry()?;
    let host = super::runtime_resources::plugin_host()?;
    open_in_registry(
        &registry,
        &host,
        ViewerInput {
            bytes,
            name,
            plugin_id,
            format,
            source,
            cancel,
        },
        None,
    )
}
pub(super) struct ViewerInput {
    pub bytes: Vec<u8>,
    pub name: String,
    pub plugin_id: Option<String>,
    pub format: BlueprintFormat,
    pub source: Option<BlueprintSource>,
    pub cancel: Arc<AtomicBool>,
}
fn scene_version_matches_format(format: BlueprintFormat, version: u32) -> bool {
    match format {
        BlueprintFormat::Schem => matches!(version, 1..=3),
        BlueprintFormat::Litematic => matches!(version, 4..=7),
    }
}
pub(super) fn open_in_registry(
    registry: &PluginRegistry,
    host: &std::path::Path,
    input: ViewerInput,
    expected: Option<&PluginManifest>,
) -> Result<OpenViewer, String> {
    let ViewerInput {
        bytes,
        name,
        plugin_id,
        format,
        source,
        cancel,
    } = input;
    let plugin = {
        let mut lifecycle = viewer_lifecycle::lock()?;
        let plugin = blueprint_viewers::select_in(registry, None, format, plugin_id.as_deref())?;
        if expected.is_some_and(|expected| expected != &plugin.manifest) {
            return Err("Selected Viewer changed during handoff".into());
        }
        lifecycle.track(&plugin.manifest.id, &cancel)?;
        plugin
    };
    let started = std::time::Instant::now();
    let session = registry
        .open_viewer(host, &plugin.manifest.id, &bytes, cancel.clone())
        .map_err(|error| {
            tracing::warn!(event="viewer_failed",plugin_id=%plugin.manifest.id,code=%error.code);
            error.to_string()
        })?;
    tracing::info!(event="viewer_opened",plugin_id=%plugin.manifest.id,milliseconds=started.elapsed().as_millis() as u64);
    let scene = session.summary().clone();
    if !scene_version_matches_format(format, scene.format_version) {
        return Err("蓝图格式与文件扩展名不匹配".into());
    }
    viewer_lifecycle::publish(
        &cancel,
        || {
            if !registry
                .resolve_enabled(&plugin.manifest.id)
                .is_ok_and(|current| current.manifest == plugin.manifest)
            {
                return Err("Viewer plugin disabled or changed".into());
            }
            Ok(())
        },
        || {
            let id = next_id();
            resources()
                .lock()
                .map_err(|error| error.to_string())?
                .insert(
                    id.clone(),
                    Arc::new(Resource {
                        session: Mutex::new(session),
                        cancel: cancel.clone(),
                        bytes,
                        name: name.clone(),
                        plugin_id: plugin.manifest.id.clone(),
                        viewer_manifest: plugin.manifest.clone(),
                        format,
                        source,
                    }),
                );
            Ok(OpenViewer {
                resource_id: id,
                name,
                scene,
                format,
            })
        },
    )
}
#[tauri::command]
pub async fn open_schematic(
    window: WebviewWindow,
    plugin_id: Option<String>,
) -> Result<Option<OpenViewer>, String> {
    let extensions = if let Some(id) = plugin_id.as_deref() {
        let selected = registry()?.resolve_enabled(id).map_err(|e| e.to_string())?;
        if !selected
            .manifest
            .supports_interface(&cobble_plugin_api::InterfaceRequirement::viewer())
        {
            return Err("Selected plugin does not provide a Viewer".into());
        }
        selected.manifest.file_extensions
    } else {
        vec!["schem".into(), "litematic".into()]
    };
    let extension_names = extensions.iter().map(String::as_str).collect::<Vec<_>>();
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("打开蓝图")
        .add_filter("Minecraft 蓝图", &extension_names)
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_owned();
    let name = file.file_name();
    let format = BlueprintFormat::from_name(&name)?;
    tauri::async_runtime::spawn_blocking(move || {
        open(read_file(&path)?, name, plugin_id, format).map(Some)
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
pub async fn read_viewer_chunk(resource_id: String, index: u32) -> Result<SceneChunk, String> {
    let resource = resource(&resource_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        resource
            .session
            .lock()
            .map_err(|error| error.to_string())?
            .read_chunk(index, resource.cancel.clone())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
pub fn close_viewer(resource_id: String) -> Result<(), String> {
    if let Some(resource) = resources()
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&resource_id)
    {
        resource.cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}
#[tauri::command]
pub fn save_blueprint(resource_id: String) -> Result<Blueprint, String> {
    let resource = resource(&resource_id)?;
    let directory = root("blueprints")?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let blueprint = Blueprint {
        id: next_id(),
        name: resource.name.clone(),
        plugin_id: resource.plugin_id.clone(),
        format: resource.format,
        favorite: false,
        has_thumbnail: false,
        source: resource.source.clone(),
    };
    std::fs::write(
        directory.join(format!("{}.{}", blueprint.id, blueprint.format.extension())),
        &resource.bytes,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(
        directory.join(format!("{}.json", blueprint.id)),
        serde_json::to_vec(&blueprint).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(blueprint)
}
#[tauri::command]
pub fn list_blueprints() -> Result<Vec<Blueprint>, String> {
    let directory = root("blueprints")?;
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    for file in std::fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        if file
            .path()
            .extension()
            .is_some_and(|extension| extension == "json")
            && let Ok(bytes) = std::fs::read(file.path())
            && let Ok(value) = serde_json::from_slice::<Blueprint>(&bytes)
        {
            values.push(value);
        }
    }
    values.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(values)
}
#[tauri::command]
pub async fn open_blueprint(id: String, plugin_id: Option<String>) -> Result<OpenViewer, String> {
    valid_id(&id)?;
    let directory = root("blueprints")?;
    let blueprint: Blueprint = serde_json::from_slice(
        &std::fs::read(directory.join(format!("{id}.json"))).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if blueprint.id != id {
        return Err("蓝图标识不匹配".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        open_with_cancel(
            read_file(&directory.join(format!("{id}.{}", blueprint.format.extension())))?,
            blueprint.name,
            plugin_id.or_else(|| (!blueprint.plugin_id.is_empty()).then_some(blueprint.plugin_id)),
            blueprint.format,
            blueprint.source,
            Arc::new(AtomicBool::new(false)),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod package_tests {
    use super::*;
    const COMPONENT: &[u8] = b"\0asm\x0d\0\x01\0";
    fn manifest() -> PluginManifest {
        PluginManifest {
            frontend: None,
            permissions: None,
            description: None,
            tags: Vec::new(),
            icon: None,
            id: "synthetic-viewer".into(),
            name: "Synthetic Viewer".into(),
            version: "0.1.0".into(),
            api_version: cobble_plugin_api::API_VERSION.into(),
            publisher: "Original test fixture".into(),
            capabilities: vec!["viewer".into()],
            tool_form: None,
            composer_permissions: Default::default(),
            interfaces: Default::default(),
            file_extensions: vec!["schem".into()],
            report_protocols: vec![],
            contributions: None,
            provider_origins: vec![],
            component_sha256: cobble_plugin_host::digest(COMPONENT),
        }
    }
    #[test]
    fn viewer_version_guard_keeps_sponge_and_litematic_versions_separate() {
        for version in 0..=8 {
            assert_eq!(
                scene_version_matches_format(BlueprintFormat::Schem, version),
                (1..=3).contains(&version)
            );
            assert_eq!(
                scene_version_matches_format(BlueprintFormat::Litematic, version),
                (4..=7).contains(&version)
            );
        }
        assert!(!scene_version_matches_format(
            BlueprintFormat::Schem,
            u32::MAX
        ));
        assert!(!scene_version_matches_format(
            BlueprintFormat::Litematic,
            u32::MAX
        ));
    }
    #[test]
    fn pending_file_completion_consumes_only_its_exact_generation() {
        let mut pending = Some(PendingBlueprint {
            queue_id: "new-selection".into(),
            path: PathBuf::from("private-directory/one.schem"),
        });
        clear_pending(&mut pending, "old-selection");
        assert_eq!(pending.as_ref().unwrap().queue_id, "new-selection");
        clear_pending(&mut pending, "new-selection");
        assert!(pending.is_none());
    }
    #[test]
    fn inline_artwork_is_materialized_before_catalog_approval() {
        let icon = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../examples/modpack-inspector/icon.png"
        ));
        let mut manifest = manifest();
        manifest.icon = Some(cobble_plugin_api::PluginIcon {
            file: "icon.png".into(),
            sha256: cobble_plugin_host::digest(icon),
        });
        assert!(staged_package_with_icon(manifest.clone(), COMPONENT, None).is_err());
        assert!(
            staged_package_with_icon(manifest.clone(), COMPONENT, Some(b"broken PNG")).is_err()
        );
        let staged = staged_package_with_icon(manifest.clone(), COMPONENT, Some(icon)).unwrap();
        assert_eq!(std::fs::read(staged.path.join("icon.png")).unwrap(), icon);
        assert_eq!(
            cobble_plugin_host::read_package(&staged.path).unwrap().0,
            manifest
        );
    }
    #[test]
    fn hash_mismatch_cannot_replace_approved_package() {
        let mut state = PackageApproval::default();
        let revision = state.begin();
        state
            .approve(revision, staged_package(manifest(), COMPONENT).unwrap())
            .unwrap();
        assert!(staged_package(manifest(), b"different bytes").is_err());
        assert_eq!(state.pending.as_ref().unwrap().manifest, manifest());
        assert!(state.pending.as_ref().unwrap().path.exists());
    }
    #[test]
    fn replacing_and_canceling_preview_releases_owned_temp_directory() {
        let mut state = PackageApproval::default();
        let revision = state.begin();
        let first = staged_package(manifest(), COMPONENT).unwrap();
        let first_path = first.path.clone();
        state.approve(revision, first).unwrap();
        let replacement = state.begin();
        assert!(!first_path.exists());
        let late = staged_package(manifest(), COMPONENT).unwrap();
        let late_path = late.path.clone();
        assert!(state.approve(revision, late).is_err());
        assert!(!late_path.exists());
        let second = staged_package(manifest(), COMPONENT).unwrap();
        let second_path = second.path.clone();
        state.approve(replacement, second).unwrap();
        let in_flight_install = state.pending.clone();
        state.begin();
        assert!(second_path.exists());
        drop(in_flight_install);
        assert!(!second_path.exists());
    }
}
