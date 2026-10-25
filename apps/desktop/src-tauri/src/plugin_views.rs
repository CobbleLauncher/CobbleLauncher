// SPDX-License-Identifier: MIT
//! Inline plugin UI: a narrowly authorized native child and an opaque-origin frame.
use cobble_plugin_api::PluginManifest;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tauri::{Manager, Webview, WebviewBuilder, WebviewUrl};

const SCHEME: &str = "cobble-plugin";
static NEXT_OPEN: AtomicU64 = AtomicU64::new(0);
#[cfg(debug_assertions)]
static ISOLATED_PROBE: AtomicBool = AtomicBool::new(false);
static VIEWS: OnceLock<Mutex<HashMap<String, Arc<ViewSession>>>> = OnceLock::new();
fn views() -> &'static Mutex<HashMap<String, Arc<ViewSession>>> {
    VIEWS.get_or_init(Default::default)
}

/// The response policy and joined native workflow are verified on WebView2 155.
/// Windows still checks the actual engine version before loading any assets.
/// Other engines retain the host-rendered fallback.
fn frontend_runtime_enabled() -> bool {
    cfg!(windows)
}

/// Mark the standalone security/workflow examples for bounded native diagnostics.
/// This helper accepts only a temporary probe registry and never broadens support.
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub fn enable_isolated_probe() -> Result<(), String> {
    let registry = super::root("plugins")?
        .canonicalize()
        .map_err(|_| "PROBE_REGISTRY_INVALID")?;
    let parent = registry.parent().ok_or("PROBE_REGISTRY_INVALID")?;
    let temporary = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| "PROBE_REGISTRY_INVALID")?;
    if !parent.starts_with(temporary)
        || !parent.ancestors().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("cobble-plugin-webview-probe-"))
        })
    {
        return Err("PROBE_REGISTRY_INVALID".into());
    }
    ISOLATED_PROBE.store(true, Ordering::Release);
    Ok(())
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
#[derive(Serialize)]
pub struct OpenView {
    pub view_id: String,
}
#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigRestoreTarget {
    pub config_id: String,
    pub backup_id: String,
}
impl ConfigRestoreTarget {
    fn valid(&self) -> bool {
        self.config_id.len() == 64
            && self.config_id.bytes().all(|b| b.is_ascii_hexdigit())
            && self.backup_id.split_once('-').is_some_and(|(stamp, hash)| {
                !stamp.is_empty()
                    && stamp.bytes().all(|b| b.is_ascii_digit())
                    && hash.len() == 64
                    && hash.bytes().all(|b| b.is_ascii_hexdigit())
            })
    }
}
struct ViewSession {
    app: tauri::AppHandle,
    owner: String,
    label: String,
    manifest: PluginManifest,
    instance_id: Option<String>,
    keybinding_target_id: Option<String>,
    handoff_id: Option<String>,
    config_restore_target: Option<ConfigRestoreTarget>,
    dirty: AtomicBool,
    locale: String,
    theme: String,
    appearance: Option<AppearanceContext>,
    cancel: Arc<AtomicBool>,
    job: Mutex<Option<Arc<AtomicBool>>>,
}
#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceContext {
    pub accent: String,
    pub density: String,
    pub scale: u16,
    #[serde(default)]
    pub text_size: Option<u16>,
    pub motion: String,
}
impl AppearanceContext {
    fn valid(&self) -> bool {
        ["green", "blue", "amber", "purple"].contains(&self.accent.as_str())
            && ["standard", "compact", "comfortable"].contains(&self.density.as_str())
            && (90..=125).contains(&self.scale)
            && self.scale.is_multiple_of(5)
            && self.text_size.is_none_or(|size| (13..=18).contains(&size))
            && ["system", "reduced"].contains(&self.motion.as_str())
    }
}
struct PendingView(Option<String>);
impl Drop for PendingView {
    fn drop(&mut self) {
        if let Some(label) = self.0.take() {
            revoke_window(&label);
        }
    }
}
#[derive(Clone)]
pub struct ViewAuthorization {
    pub manifest: PluginManifest,
    pub instance_id: Option<String>,
    pub keybinding_target_id: Option<String>,
    pub handoff_id: Option<String>,
    pub config_restore_target: Option<ConfigRestoreTarget>,
    pub locale: String,
    pub theme: String,
    pub appearance: Option<AppearanceContext>,
    pub cancel: Arc<AtomicBool>,
    pub session_id: String,
}
/// Test-only backend fixture session. It is not a rendered WebView authorization proof.
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub fn isolated_authorization(
    app: &tauri::AppHandle,
    plugin_id: &str,
    session_id: &str,
) -> Result<ViewAuthorization, String> {
    if !ISOLATED_PROBE.load(Ordering::Acquire)
        || !session_id.starts_with("maintenance-probe-")
        || session_id.len() > 96
        || !session_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err("PROBE_SESSION_INVALID".into());
    }
    let manifest = selected(plugin_id)?;
    let session = Arc::new(ViewSession {
        app: app.clone(),
        owner: "main".into(),
        label: session_id.into(),
        manifest: manifest.clone(),
        instance_id: None,
        keybinding_target_id: None,
        handoff_id: None,
        config_restore_target: None,
        dirty: AtomicBool::new(false),
        locale: "en-US".into(),
        theme: "light".into(),
        appearance: None,
        cancel: Arc::new(AtomicBool::new(false)),
        job: Mutex::new(None),
    });
    let mut sessions = views().lock().map_err(|_| "PROBE_SESSION_INVALID")?;
    if sessions.contains_key(session_id) {
        return Err("PROBE_SESSION_INVALID".into());
    }
    sessions.insert(session_id.into(), session.clone());
    Ok(ViewAuthorization {
        manifest,
        instance_id: None,
        keybinding_target_id: None,
        handoff_id: None,
        config_restore_target: None,
        locale: "en-US".into(),
        theme: "light".into(),
        appearance: None,
        cancel: session.cancel.clone(),
        session_id: session_id.into(),
    })
}
fn parent_only(webview: &Webview) -> Result<(), String> {
    if webview.label() != "main" {
        return Err("PLUGIN_VIEW_CALLER_DENIED".into());
    }
    Ok(())
}
fn selected(plugin_id: &str) -> Result<PluginManifest, String> {
    super::valid_id(plugin_id)?;
    let installed = super::registry()?
        .resolve_enabled(plugin_id)
        .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?;
    if installed.manifest.frontend.is_none() {
        return Err("PLUGIN_VIEW_UNAVAILABLE".into());
    }
    Ok(installed.manifest)
}
fn current(label: &str) -> Result<Arc<ViewSession>, String> {
    let session = views()
        .lock()
        .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?
        .get(label)
        .cloned()
        .ok_or("PLUGIN_VIEW_REVOKED")?;
    if session.cancel.load(Ordering::Acquire) || selected(&session.manifest.id)? != session.manifest
    {
        terminate(&session);
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    Ok(session)
}
pub fn authorize_view(webview: &Webview) -> Result<ViewAuthorization, String> {
    let session = current(webview.label())?;
    Ok(ViewAuthorization {
        manifest: session.manifest.clone(),
        instance_id: session.instance_id.clone(),
        keybinding_target_id: session.keybinding_target_id.clone(),
        handoff_id: session.handoff_id.clone(),
        config_restore_target: session.config_restore_target.clone(),
        locale: session.locale.clone(),
        theme: session.theme.clone(),
        appearance: session.appearance.clone(),
        cancel: session.cancel.clone(),
        session_id: session.label.clone(),
    })
}
pub fn register_job(label: &str, job: Arc<AtomicBool>) -> Result<(), String> {
    let session = current(label)?;
    let mut active = session.job.lock().map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?;
    if session.cancel.load(Ordering::Acquire) {
        job.store(true, Ordering::Release);
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    if let Some(previous) = active.replace(job) {
        previous.store(true, Ordering::Release);
    }
    Ok(())
}
pub fn clear_job(label: &str, job: &Arc<AtomicBool>) {
    let session = views()
        .lock()
        .ok()
        .and_then(|store| store.get(label).cloned());
    if let Some(session) = session
        && let Ok(mut active) = session.job.lock()
        && active
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, job))
    {
        *active = None;
    }
}
fn terminate(session: &ViewSession) {
    super::plugin_lang_package::revoke_session(&session.label);
    session.cancel.store(true, Ordering::Release);
    super::plugin_mesh_events::revoke_session(&session.label);
    if let Ok(mut job) = session.job.lock()
        && let Some(job) = job.take()
    {
        job.store(true, Ordering::Release);
    }
    if let Some(view) = session.app.get_webview(&session.label) {
        let _ = view.close();
    }
}
fn remove_matching(predicate: impl Fn(&ViewSession) -> bool) {
    let removed = if let Ok(mut store) = views().lock() {
        let keys: Vec<_> = store
            .iter()
            .filter(|(_, session)| predicate(session))
            .map(|(key, _)| key.clone())
            .collect();
        keys.into_iter()
            .filter_map(|key| store.remove(&key))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    for session in removed {
        terminate(&session);
    }
}
pub fn revoke_window(label: &str) {
    remove_matching(|session| session.owner == label || session.label == label);
}
pub fn revoke_plugin(plugin_id: &str) {
    remove_matching(|session| session.manifest.id == plugin_id);
}
fn validate_bounds(bounds: ViewBounds, width: f64, height: f64) -> Result<(), String> {
    if ![
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        width,
        height,
    ]
    .iter()
    .all(|number| number.is_finite())
        || bounds.x < 0.0
        || bounds.y < 0.0
        || bounds.width < 1.0
        || bounds.height < 1.0
        || bounds.x + bounds.width > width + 1.0
        || bounds.y + bounds.height > height + 1.0
    {
        return Err("PLUGIN_VIEW_BOUNDS_INVALID".into());
    }
    Ok(())
}
fn window_bounds(webview: &Webview, bounds: ViewBounds) -> Result<(), String> {
    let window = webview.window();
    let scale = window
        .scale_factor()
        .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?;
    let size = window
        .inner_size()
        .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?
        .to_logical::<f64>(scale);
    validate_bounds(bounds, size.width, size.height)
}
fn base_url(label: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("http://{SCHEME}.localhost/{label}/")
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!("{SCHEME}://localhost/{label}/")
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Preserve the named Tauri command arguments for existing clients.
pub async fn open_plugin_view(
    webview: Webview,
    plugin_id: String,
    instance_id: Option<String>,
    keybinding_target_id: Option<String>,
    handoff_id: Option<String>,
    config_restore_target: Option<ConfigRestoreTarget>,
    bounds: ViewBounds,
    locale: String,
    theme: String,
    appearance: Option<AppearanceContext>,
) -> Result<OpenView, String> {
    if !frontend_runtime_enabled() {
        return Err("PLUGIN_FRONTEND_ISOLATION_UNVERIFIED".into());
    }
    parent_only(&webview)?;
    window_bounds(&webview, bounds)?;
    if !["zh-CN", "en-US"].contains(&locale.as_str())
        || !["light", "dark"].contains(&theme.as_str())
        || appearance.as_ref().is_some_and(|value| !value.valid())
    {
        return Err("PLUGIN_VIEW_CONTEXT_INVALID".into());
    }
    if let Some(id) = &instance_id {
        crate::games::ensure_registered_game(webview.app_handle(), id)?;
    }
    if let Some(id) = &keybinding_target_id {
        crate::games::ensure_registered_game(webview.app_handle(), id)?;
    }
    if config_restore_target
        .as_ref()
        .is_some_and(|target| !target.valid() || instance_id.is_none())
    {
        return Err("PLUGIN_VIEW_CONTEXT_INVALID".into());
    }
    if handoff_id.is_some() {
        super::plugin_handoffs::authorize_view(
            &plugin_id,
            instance_id.as_deref(),
            handoff_id.as_deref(),
        )?;
    }
    let generation = NEXT_OPEN.fetch_add(1, Ordering::AcqRel) + 1;
    revoke_window(webview.label());
    let manifest = tauri::async_runtime::spawn_blocking(move || selected(&plugin_id))
        .await
        .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")??;
    if NEXT_OPEN.load(Ordering::Acquire) != generation {
        return Err("PLUGIN_VIEW_REPLACED".into());
    }
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?;
    let label = format!(
        "plugin-ui-{}",
        random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let base = base_url(&label);
    let entry = format!(
        "{base}{}",
        manifest
            .frontend
            .as_ref()
            .ok_or("PLUGIN_VIEW_UNAVAILABLE")?
            .entry
    );
    let shell = format!("{base}__shell");
    let session = Arc::new(ViewSession {
        app: webview.app_handle().clone(),
        owner: webview.label().into(),
        label: label.clone(),
        manifest,
        instance_id,
        keybinding_target_id,
        handoff_id,
        config_restore_target,
        dirty: AtomicBool::new(false),
        locale,
        theme,
        appearance,
        cancel: Arc::new(AtomicBool::new(false)),
        job: Mutex::new(None),
    });
    views()
        .lock()
        .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?
        .insert(label.clone(), session.clone());
    let mut pending = PendingView(Some(label.clone()));
    let allowed_shell = shell.clone();
    let allowed_entry = entry.clone();
    let builder = WebviewBuilder::new(
        &label,
        WebviewUrl::External(
            "about:blank"
                .parse()
                .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?,
        ),
    )
    .data_directory(super::root("plugin-webview-storage")?.join(&session.manifest.id))
    .initialization_script(SHELL_BOOTSTRAP)
    .initialization_script_for_all_frames(FRAME_BOOTSTRAP)
    .on_navigation(move |url| url.as_str() == allowed_shell || url.as_str() == allowed_entry)
    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
    .on_download(|_, _| false)
    .on_permission_request(|_, _| tauri::webview::PermissionResponse::Deny);
    let result = webview.window().add_child(
        builder,
        tauri::LogicalPosition::new(bounds.x, bounds.y),
        tauri::LogicalSize::new(bounds.width, bounds.height),
    );
    if result.is_err() || NEXT_OPEN.load(Ordering::Acquire) != generation {
        revoke_window(&label);
        return Err("PLUGIN_VIEW_UNAVAILABLE".into());
    }
    let child = result.map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?;
    #[cfg(windows)]
    {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        child
            .with_webview(move |native| {
                let version =
                    cobble_plugin_webview_windows::native::browser_version(native.environment());
                let _ = sender.send(version);
            })
            .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?;
        let version = tokio::time::timeout(std::time::Duration::from_secs(5), receiver)
            .await
            .map_err(|_| "PLUGIN_VIEW_RUNTIME_UNSUPPORTED")?
            .map_err(|_| "PLUGIN_VIEW_RUNTIME_UNSUPPORTED")?
            .map_err(|_| "PLUGIN_VIEW_RUNTIME_UNSUPPORTED")?;
        let major = version
            .split('.')
            .next()
            .and_then(|part| part.parse::<u32>().ok())
            .ok_or("PLUGIN_VIEW_RUNTIME_UNSUPPORTED")?;
        if major < 152 {
            revoke_window(&label);
            return Err("PLUGIN_VIEW_RUNTIME_UNSUPPORTED".into());
        }
        #[cfg(debug_assertions)]
        if ISOLATED_PROBE.load(Ordering::Acquire) {
            eprintln!("probe WebView2 runtime {version}");
        }
    }
    #[cfg(not(windows))]
    {
        revoke_window(&label);
        return Err("PLUGIN_VIEW_RUNTIME_UNSUPPORTED".into());
    }
    if NEXT_OPEN.load(Ordering::Acquire) != generation
        || child
            .navigate(shell.parse().map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?)
            .is_err()
    {
        revoke_window(&label);
        return Err("PLUGIN_VIEW_UNAVAILABLE".into());
    }
    pending.0 = None;
    Ok(OpenView { view_id: label })
}
#[tauri::command]
pub fn resize_plugin_view(
    webview: Webview,
    view_id: String,
    bounds: ViewBounds,
) -> Result<(), String> {
    parent_only(&webview)?;
    window_bounds(&webview, bounds)?;
    let session = current(&view_id)?;
    if session.owner != webview.label() {
        return Err("PLUGIN_VIEW_CALLER_DENIED".into());
    }
    webview
        .app_handle()
        .get_webview(&view_id)
        .ok_or("PLUGIN_VIEW_REVOKED")?
        .set_bounds(tauri::Rect {
            position: tauri::LogicalPosition::new(bounds.x, bounds.y).into(),
            size: tauri::LogicalSize::new(bounds.width, bounds.height).into(),
        })
        .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE".into())
}
#[tauri::command]
pub fn close_plugin_view(webview: Webview, view_id: String) -> Result<(), String> {
    parent_only(&webview)?;
    let owned = views()
        .lock()
        .map_err(|_| "PLUGIN_VIEW_UNAVAILABLE")?
        .get(&view_id)
        .is_some_and(|session| session.owner == webview.label());
    if owned {
        revoke_window(&view_id);
    }
    Ok(())
}

pub fn set_dirty(label: &str, dirty: bool) -> Result<(), String> {
    current(label)?.dirty.store(dirty, Ordering::Release);
    Ok(())
}
#[tauri::command]
pub fn plugin_view_is_dirty(webview: Webview, view_id: String) -> Result<bool, String> {
    parent_only(&webview)?;
    let session = current(&view_id)?;
    if session.owner != webview.label() {
        return Err("PLUGIN_VIEW_CALLER_DENIED".into());
    }
    Ok(session.dirty.load(Ordering::Acquire))
}

#[tauri::command]
pub fn set_plugin_view_visible(
    webview: Webview,
    view_id: String,
    visible: bool,
) -> Result<(), String> {
    parent_only(&webview)?;
    let session = current(&view_id)?;
    if session.owner != webview.label() {
        return Err("PLUGIN_VIEW_CALLER_DENIED".into());
    }
    let view = webview
        .app_handle()
        .get_webview(&view_id)
        .ok_or("PLUGIN_VIEW_REVOKED")?;
    if visible { view.show() } else { view.hide() }.map_err(|_| "PLUGIN_VIEW_UNAVAILABLE".into())
}

pub fn serve_asset(
    context: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let result = asset_response(
        context.webview_label(),
        request.uri().path(),
        request.uri().query(),
        request.method().as_str(),
    );
    match result {
        Ok(response) => response,
        Err(_) => tauri::http::Response::builder()
            .status(403)
            .header("Content-Type", "text/plain")
            .header("Connection-Allowlist", "(); webrtc=block")
            .header("Content-Security-Policy", "default-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'")
            .header("X-Content-Type-Options", "nosniff")
            .header("Cache-Control", "no-store")
            .body(b"Plugin view unavailable".to_vec())
            .expect("fixed response"),
    }
}
fn asset_response(
    label: &str,
    path: &str,
    query: Option<&str>,
    method: &str,
) -> Result<tauri::http::Response<Vec<u8>>, String> {
    if query.is_some() || method != "GET" {
        return Err("PLUGIN_VIEW_ASSET_DENIED".into());
    }
    let session = current(label)?;
    let asset = path
        .strip_prefix(&format!("/{label}/"))
        .ok_or("PLUGIN_VIEW_ASSET_DENIED")?;
    let base = base_url(label);
    let entry = format!(
        "{base}{}",
        session
            .manifest
            .frontend
            .as_ref()
            .ok_or("PLUGIN_VIEW_UNAVAILABLE")?
            .entry
    );
    let is_shell = asset == "__shell";
    let (bytes, media, csp) = if is_shell {
        let background = if session.theme == "dark" {
            "#181b17"
        } else {
            "#f7f8f4"
        };
        let html = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><style>html,body,iframe{{width:100%;height:100%;margin:0;border:0;overflow:hidden;background:{background}}}</style></head><body><iframe id=\"plugin-content\" sandbox=\"allow-scripts\" referrerpolicy=\"no-referrer\" title=\"Plugin\" src=\"{entry}\"></iframe></body></html>"
        );
        (
            html.into_bytes(),
            "text/html; charset=utf-8",
            format!(
                "default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; frame-src {entry}; object-src 'none'; base-uri 'none'; form-action 'none'; connect-src ipc: http://ipc.localhost"
            ),
        )
    } else {
        let directory = super::root("plugins")?.join(&session.manifest.id);
        let file = cobble_plugin_host::read_frontend_asset(&directory, &session.manifest, asset)
            .map_err(|_| "PLUGIN_VIEW_ASSET_DENIED")?;
        (
            file.bytes,
            file.media_type,
            format!(
                "default-src 'none'; script-src {base}; style-src {base} 'unsafe-inline'; img-src {base} data:; font-src {base} data:; connect-src 'none'; worker-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'"
            ),
        )
    };
    tauri::http::Response::builder().status(200)
        .header("Content-Type", media).header("Content-Security-Policy", csp)
        .header("Connection-Allowlist", if is_shell { "(response-origin \"http://ipc.localhost/frontend_call\"); webrtc=block" } else { "(response-origin); webrtc=block" })
        .header("Cache-Control", "no-store").header("X-Content-Type-Options", "nosniff")
        .header("Access-Control-Allow-Origin", "null").header("Referrer-Policy", "no-referrer")
        .header("Permissions-Policy", "camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), bluetooth=(), display-capture=()")
        .body(bytes).map_err(|_| "PLUGIN_VIEW_ASSET_DENIED".into())
}

const SHELL_BOOTSTRAP: &str = r#"
(() => {
 if (window !== window.top) return;
 const channel = 'cobble-plugin-frontend-v1';
 const allowed = new Set(['mesh.subscribe','mesh.receive','mesh.unsubscribe','mesh.send','mesh.peers','ai.status','ai.configure','ai.clear','ai.chat','ai.cancel','recovery.experimentPreview','recovery.experimentOpen','trialCopy.cleanup','trialCopy.preview','trialCopy.pickDestination','trialCopy.start','trialCopy.task','trialCopy.cancel','trialCopy.openGame','storageWorkspace.overview','storageWorkspace.pickDestination','storageWorkspace.cleanCache','storageWorkspace.startMove','storageWorkspace.task','storageWorkspace.cancel','storageWorkspace.releaseSource','storageWorkspace.openSource','configWorkspace.list','configWorkspace.read','configWorkspace.readBackup','configWorkspace.write','configWorkspace.restore','recovery.snapshot','recovery.openRequest','playManifest.preview','playManifest.export','fileInbox.pick','fileInbox.pending','fileInbox.open','fileInbox.release','langPackage.pick','langPackage.check','langPackage.document','langPackage.cancel','generatedBlueprint.save','generatedBlueprint.open','context','exports.text','clipboard.writeText','view.setDirty','handoffs.receivers','handoffs.send','handoffs.take','data.read','data.write','data.remove','worlds.metadata','seedMap.capabilities','seedMap.generate','seedMap.search','seedMap.cancel','configs.list','configs.read','report.pick','report.check','report.cancel','keybindings.preview','keybindings.apply','keybindings.restore','diagnostics.list','diagnostics.read','diagnostics.check','diagnostics.pick','inspection.snapshot','inspection.check','worldSnapshots.list','worldSnapshots.compare','worldSnapshots.openRestore']);
 window.addEventListener('message', async event => {
  const frame = document.getElementById('plugin-content');
  const data = event.data;
  if (!frame || event.source !== frame.contentWindow || !data || data.channel !== channel || typeof data.id !== 'string' || data.id.length > 80 || typeof data.token !== 'string' || !/^[a-f0-9]{32}$/.test(data.token) || !data.request || !allowed.has(data.request.method)) return;
  const reply = {channel,id:data.id,token:data.token};
  try {
   const requestLimit = ['exports.text','handoffs.send','configWorkspace.write'].includes(data.request.method) ? 7 * 1048576 : data.request.method === 'generatedBlueprint.save' ? 360448 : data.request.method === 'data.write' ? 69632 : 16384;
   if (JSON.stringify(data.request).length > requestLimit) throw new Error('PLUGIN_FRONTEND_REQUEST_TOO_LARGE');
   reply.value = await window.__TAURI_INTERNALS__.invoke('frontend_call', {request:data.request});
  } catch (error) { reply.error = typeof error === 'string' ? error.slice(0,1024) : 'PLUGIN_FRONTEND_REQUEST_FAILED'; }
  frame.contentWindow.postMessage(reply, '*');
 });
})();
"#;
const FRAME_BOOTSTRAP: &str = r#"
(() => {
 if (window === window.top) return;
 const channel = 'cobble-plugin-frontend-v1';
 const random = new Uint8Array(16); crypto.getRandomValues(random);
 const token = [...random].map(byte=>byte.toString(16).padStart(2,'0')).join('');
 const pending = new Map(); let next = 0;
 window.addEventListener('message', event => {
  const data=event.data;
  if (event.source!==window.parent || !data || data.channel!==channel || data.token!==token) return;
  const request=pending.get(data.id); if(!request)return;
  pending.delete(data.id);clearTimeout(request.timer);
  if(typeof data.error==='string')request.reject(new Error(data.error));else request.resolve(data.value);
 });
 Object.defineProperty(window,'__COBBLE_PLUGIN_BRIDGE__',{value:Object.freeze({request(method,params={}) {
  return new Promise((resolve,reject)=>{
   if(!['mesh.subscribe','mesh.receive','mesh.unsubscribe','mesh.send','mesh.peers','ai.status','ai.configure','ai.clear','ai.chat','ai.cancel','recovery.experimentPreview','recovery.experimentOpen','trialCopy.cleanup','trialCopy.preview','trialCopy.pickDestination','trialCopy.start','trialCopy.task','trialCopy.cancel','trialCopy.openGame','storageWorkspace.overview','storageWorkspace.pickDestination','storageWorkspace.cleanCache','storageWorkspace.startMove','storageWorkspace.task','storageWorkspace.cancel','storageWorkspace.releaseSource','storageWorkspace.openSource','configWorkspace.list','configWorkspace.read','configWorkspace.readBackup','configWorkspace.write','configWorkspace.restore','recovery.snapshot','recovery.openRequest','playManifest.preview','playManifest.export','fileInbox.pick','fileInbox.pending','fileInbox.open','fileInbox.release','langPackage.pick','langPackage.check','langPackage.document','langPackage.cancel','generatedBlueprint.save','generatedBlueprint.open','context','exports.text','clipboard.writeText','view.setDirty','handoffs.receivers','handoffs.send','handoffs.take','data.read','data.write','data.remove','worlds.metadata','seedMap.capabilities','seedMap.generate','seedMap.search','seedMap.cancel','configs.list','configs.read','report.pick','report.check','report.cancel','keybindings.preview','keybindings.apply','keybindings.restore','diagnostics.list','diagnostics.read','diagnostics.check','diagnostics.pick','inspection.snapshot','inspection.check','worldSnapshots.list','worldSnapshots.compare','worldSnapshots.openRestore'].includes(method)){reject(new Error('PLUGIN_FRONTEND_METHOD_DENIED'));return;}
   if(pending.size>=16){reject(new Error('PLUGIN_FRONTEND_BUSY'));return;}
   const id=String(++next);const timer=setTimeout(()=>{pending.delete(id);reject(new Error('PLUGIN_FRONTEND_TIMEOUT'));},60000);
   pending.set(id,{resolve,reject,timer});
   try{window.parent.postMessage({channel,token,id,request:{method,params}},'*');}catch(error){pending.delete(id);clearTimeout(timer);reject(error);}
  });
 }})});
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appearance_accepts_legacy_defaults_and_bounded_text_scaling() {
        let legacy = serde_json::json!({"accent":"green","density":"standard","scale":100,"motion":"system"});
        let parsed: AppearanceContext = serde_json::from_value(legacy.clone()).unwrap();
        assert!(parsed.valid());
        assert_eq!(parsed.text_size, None);
        for (density, scale, text_size) in [("compact", 90, 13), ("comfortable", 125, 18)] {
            let mut value = legacy.clone();
            value["density"] = density.into();
            value["scale"] = scale.into();
            value["textSize"] = text_size.into();
            assert!(
                serde_json::from_value::<AppearanceContext>(value)
                    .unwrap()
                    .valid()
            );
        }
        for (field, value) in [
            ("scale", 91),
            ("scale", 130),
            ("textSize", 12),
            ("textSize", 19),
        ] {
            let mut invalid = legacy.clone();
            invalid[field] = value.into();
            assert!(
                !serde_json::from_value::<AppearanceContext>(invalid)
                    .unwrap()
                    .valid()
            );
        }
    }
    #[test]
    fn bounds_are_finite_and_remain_inside_parent() {
        let good = ViewBounds {
            x: 20.0,
            y: 80.0,
            width: 620.0,
            height: 450.0,
        };
        assert!(validate_bounds(good, 680.0, 560.0).is_ok());
        for bad in [
            ViewBounds { x: -1.0, ..good },
            ViewBounds {
                width: f64::NAN,
                ..good
            },
            ViewBounds {
                height: 600.0,
                ..good
            },
            ViewBounds {
                height: 0.0,
                ..good
            },
        ] {
            assert!(validate_bounds(bad, 680.0, 560.0).is_err());
        }
    }
}
