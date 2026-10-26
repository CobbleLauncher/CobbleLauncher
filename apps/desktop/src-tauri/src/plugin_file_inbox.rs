// SPDX-License-Identifier: MIT
//! Native-selected, immutable Inbox handles and explicit main-window handoffs.
//! No renderer/guest request accepts a path or file bytes.
use super::plugin_views::ViewAuthorization;
use cobble_plugin_api::{PermissionNode, PluginManifest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Webview};
#[path = "plugin_file_inbox_links.rs"]
mod links;
pub use links::LinkRequest;
const MAX_FILES: usize = 8;
const MAX_BYTES: u64 = cobble_content::MAX_PACK_ARCHIVE_BYTES;
const MAX_TOTAL: u64 = 1024 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(600);
const HANDOFF_TTL: Duration = Duration::from_secs(300);
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Handler {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub requires_game: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameChoice {
    pub id: String,
    pub name: String,
    pub version: String,
    pub compatibility: String,
    pub reason: Option<String>,
    pub available: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub resource_id: String,
    pub name: String,
    pub bytes: u64,
    pub kind: String,
    pub evidence: Vec<String>,
    pub handlers: Vec<Handler>,
    pub games: Vec<GameChoice>,
    pub expires_at: u64,
    pub declared_minecraft: Option<String>,
    pub declared_loader: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default)]
struct Classification {
    kind: String,
    evidence: Vec<String>,
    minecraft: Option<String>,
    loader: Option<String>,
    imports: Vec<String>,
    error: Option<String>,
}
struct Snapshot {
    _directory: tempfile::TempDir,
    path: PathBuf,
    name: String,
    bytes: u64,
    sha256: String,
    classification: Classification,
}
struct Resource {
    id: String,
    manifest: PluginManifest,
    session: Option<String>,
    scope: Option<String>,
    cancel: Option<Arc<AtomicBool>>,
    file: Arc<Snapshot>,
    expires: Instant,
    expires_at: u64,
}
#[derive(Clone)]
struct Handoff {
    id: String,
    resource_id: String,
    manifest: PluginManifest,
    file: Arc<Snapshot>,
    handler: Handler,
    game_id: Option<String>,
    locale: String,
    expires: Instant,
}
#[derive(Default)]
struct Store {
    resources: VecDeque<Resource>,
    handoffs: VecDeque<Handoff>,
    reserved: u64,
    reserved_files: usize,
}
static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
fn store() -> &'static Mutex<Store> {
    STORE.get_or_init(|| Mutex::new(Store::default()))
}
fn at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn token() -> Result<String, String> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    Ok(bytes.iter().map(|v| format!("{v:02x}")).collect())
}
fn prune(state: &mut Store) {
    let now = Instant::now();
    state.resources.retain(|r| r.expires > now);
    state.handoffs.retain(|h| h.expires > now);
}
fn retained_bytes(state: &Store) -> u64 {
    let mut ids = HashSet::new();
    state
        .resources
        .iter()
        .map(|r| (&r.id, &r.file))
        .chain(state.handoffs.iter().map(|h| (&h.resource_id, &h.file)))
        .filter_map(|(id, f)| ids.insert(id).then_some(f.bytes))
        .sum()
}
struct Reservation(u64, usize);
impl Drop for Reservation {
    fn drop(&mut self) {
        if let Ok(mut state) = store().lock() {
            state.reserved = state.reserved.saturating_sub(self.0);
            state.reserved_files = state.reserved_files.saturating_sub(self.1);
        }
    }
}
fn reserve(bytes: u64, count: usize) -> Result<Reservation, String> {
    let mut state = store().lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    prune(&mut state);
    if state.resources.len() + state.reserved_files + count > MAX_FILES
        || state.reserved + retained_bytes(&state) + bytes > MAX_TOTAL
    {
        return Err("FILE_INBOX_LIMIT".into());
    }
    state.reserved += bytes;
    state.reserved_files += count;
    Ok(Reservation(bytes, count))
}
pub fn shutdown() {
    if let Ok(mut state) = store().lock() {
        state.resources.clear();
        state.handoffs.clear();
    }
}
pub fn permitted(manifest: &PluginManifest) -> Result<(), String> {
    if manifest
        .permissions
        .as_deref()
        .unwrap_or_default()
        .contains(&PermissionNode::FileInboxImportRequest {})
    {
        Ok(())
    } else {
        Err("PLUGIN_PERMISSION_DENIED".into())
    }
}
fn current(auth: &ViewAuthorization) -> Result<(), String> {
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    let selected = super::registry()?
        .resolve_enabled(&auth.manifest.id)
        .map_err(|_| "PLUGIN_VIEW_REVOKED")?;
    if selected.manifest != auth.manifest {
        return Err("PLUGIN_VIEW_STALE".into());
    }
    permitted(&auth.manifest)
}
fn regular(path: &Path) -> Result<fs::Metadata, String> {
    let m = fs::symlink_metadata(path).map_err(|_| "FILE_INBOX_SOURCE_UNAVAILABLE")?;
    if !m.is_file() || m.file_type().is_symlink() {
        return Err("FILE_INBOX_REGULAR_FILE_REQUIRED".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err("FILE_INBOX_REGULAR_FILE_REQUIRED".into());
        }
    }
    if m.len() > MAX_BYTES {
        return Err("FILE_INBOX_FILE_TOO_LARGE".into());
    }
    Ok(m)
}
fn safe_name(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("FILE_INBOX_NAME_INVALID")?;
    cobble_store::SafeRelativePath::parse(name).map_err(|_| "FILE_INBOX_NAME_INVALID")?;
    if name.len() > 240 || name.contains(['/', '\\']) {
        return Err("FILE_INBOX_NAME_INVALID".into());
    }
    Ok(name.into())
}
fn snapshot(path: &Path, cancel: Option<&AtomicBool>) -> Result<Snapshot, String> {
    let before = regular(path)?;
    let name = safe_name(path)?;
    let directory = tempfile::Builder::new()
        .prefix("cobble-file-inbox-")
        .tempdir()
        .map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    let parent = if name.to_ascii_lowercase().ends_with(".jar") {
        let p = directory.path().join("mods");
        fs::create_dir(&p).map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
        p
    } else {
        directory.path().to_owned()
    };
    let destination = parent.join(&name);
    let mut input = File::open(path).map_err(|_| "FILE_INBOX_SOURCE_UNAVAILABLE")?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut copied = 0;
    loop {
        if cancel.is_some_and(|c| c.load(Ordering::Acquire)) {
            return Err("PLUGIN_ACTION_CANCELLED".into());
        }
        let amount = input
            .read(&mut buffer)
            .map_err(|_| "FILE_INBOX_SOURCE_UNAVAILABLE")?;
        if amount == 0 {
            break;
        }
        copied += amount as u64;
        if copied > before.len() {
            return Err("FILE_INBOX_SOURCE_CHANGED".into());
        }
        hash.update(&buffer[..amount]);
        output
            .write_all(&buffer[..amount])
            .map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    }
    output.sync_all().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    drop(output);
    let after = regular(path)?;
    if copied != before.len()
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return Err("FILE_INBOX_SOURCE_CHANGED".into());
    }
    let classification = classify(&destination, directory.path(), &name);
    Ok(Snapshot {
        _directory: directory,
        path: destination,
        name,
        bytes: copied,
        sha256: format!("{:x}", hash.finalize()),
        classification,
    })
}
fn classify(path: &Path, base: &Path, name: &str) -> Classification {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut c = Classification {
        kind: "unknown".into(),
        ..Classification::default()
    };
    if extension == "jar" {
        c.kind = "mod".into();
        c.imports.push("mod".into());
        c.evidence.push("jar_file".into());
        if let Ok(mods) = cobble_content::list_mods(base) {
            c.loader = mods
                .first()
                .and_then(|m| m.loader.clone())
                .map(|v| v.to_ascii_lowercase());
        }
        if c.loader.is_some() {
            c.evidence.push("declared_loader".into());
        }
        return c;
    }
    if ["schem", "litematic"].contains(&extension.as_str()) {
        c.kind = "blueprint".into();
        c.evidence.push("extension_only".into());
        return c;
    }
    if ["log", "txt"].contains(&extension.as_str()) {
        c.kind = "log".into();
        c.evidence.push("extension_only".into());
        return c;
    }
    if !["zip", "mrpack"].contains(&extension.as_str()) {
        c.evidence.push("extension_only".into());
        return c;
    }
    let markers = archive_markers(path);
    let Ok(markers) = markers else {
        c.error = Some("FILE_INBOX_ARCHIVE_INVALID".into());
        return c;
    };
    if markers.contains("modrinth.index.json") {
        match cobble_content::read_mrpack(path) {
            Ok(pack) => {
                c.imports.push("mrpack".into());
                c.minecraft = Some(pack.minecraft_version);
                c.loader = pack.loader_kind;
                c.evidence.push("mrpack_manifest".into());
            }
            Err(_) => c.error = Some("FILE_INBOX_FORMAT_INVALID".into()),
        }
    }
    if markers.contains("manifest.json")
        && let Ok(pack) = cobble_content::read_curseforge_pack(path)
    {
        c.imports.push("curseforge-pack".into());
        c.minecraft = Some(pack.minecraft_version);
        c.loader = pack.loader.map(|l| l.kind);
        c.evidence.push("curseforge_manifest".into());
    }
    if markers.contains("pack.mcmeta") {
        let metadata = File::open(path)
            .ok()
            .and_then(|f| cobble_content::read_pack_metadata_zip(f).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        if metadata.as_ref().is_some_and(|v| v["pack"].is_object()) {
            c.evidence.push("root_pack_metadata".into());
            if markers.contains("assets/") {
                c.imports.push("resource-pack".into());
                c.evidence.push("assets".into());
            }
        } else {
            c.error = Some("FILE_INBOX_METADATA_INVALID".into());
        }
    }
    if markers.contains("shaders/") {
        c.imports.push("shader-pack".into());
        c.evidence.push("shaders".into());
    }
    c.kind = match c.imports.as_slice() {
        [] => "unknown".into(),
        [one] => one.clone(),
        _ => {
            c.evidence.push("multiple_formats".into());
            "ambiguous".into()
        }
    };
    if c.kind == "ambiguous" {
        c.minecraft = None;
        c.loader = None;
    }
    if c.imports.is_empty() && c.error.is_none() {
        c.evidence.push("archive_unrecognized".into());
    }
    let _ = name;
    c
}
fn archive_markers(path: &Path) -> Result<HashSet<String>, String> {
    let mut archive =
        zip::ZipArchive::new(File::open(path).map_err(|_| "FILE_INBOX_ARCHIVE_INVALID")?)
            .map_err(|_| "FILE_INBOX_ARCHIVE_INVALID")?;
    if archive.len() > 50_000 {
        return Err("FILE_INBOX_ARCHIVE_INVALID".into());
    }
    archive
        .validate_header_consistency()
        .map_err(|_| "FILE_INBOX_ARCHIVE_INVALID")?;
    let mut found = HashSet::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|_| "FILE_INBOX_ARCHIVE_INVALID")?;
        let name = entry.name().map_err(|_| "FILE_INBOX_ARCHIVE_INVALID")?;
        let clean = name.trim_end_matches('/');
        if !clean.is_empty() {
            cobble_store::SafeRelativePath::parse(clean)
                .map_err(|_| "FILE_INBOX_ARCHIVE_INVALID")?;
        }
        for root in ["modrinth.index.json", "manifest.json", "pack.mcmeta"] {
            if name == root {
                found.insert(root.into());
            }
        }
        for root in ["assets/", "shaders/", "data/"] {
            if name.starts_with(root) {
                found.insert(root.into());
            }
        }
    }
    Ok(found)
}
fn handlers(file: &Snapshot, locale: &str) -> Result<Vec<Handler>, String> {
    let zh = locale == "zh-CN";
    let mut result = Vec::new();
    for kind in &file.classification.imports {
        let label = match (kind.as_str(), zh) {
            ("mrpack", false) => "Import modpack",
            ("mrpack", true) => "导入整合包",
            ("curseforge-pack", false) => "Import CurseForge pack",
            ("curseforge-pack", true) => "导入 CurseForge 整合包",
            ("mod", false) => "Import Mod",
            ("mod", true) => "导入 Mod",
            ("resource-pack", false) => "Import resource pack",
            ("resource-pack", true) => "导入资源包",
            ("shader-pack", false) => "Import shader pack",
            ("shader-pack", true) => "导入光影包",
            _ => continue,
        };
        result.push(Handler {
            id: format!("builtin:{kind}"),
            label: label.into(),
            kind: "import".into(),
            requires_game: !["mrpack", "curseforge-pack"].contains(&kind.as_str()),
        });
    }
    let ext = file
        .path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    for plugin in super::registry()?
        .list()
        .map_err(|_| "FILE_INBOX_UNAVAILABLE")?
    {
        if !plugin.enabled
            || plugin.problem.is_some()
            || plugin.manifest.id == "file-inbox"
            || !plugin.manifest.file_extensions.iter().any(|e| e == &ext)
        {
            continue;
        }
        let (prefix, kind) = if file.classification.kind == "blueprint"
            && file.bytes <= cobble_plugin_api::MAX_FILE_BYTES as u64
            && plugin
                .manifest
                .supports_interface(&cobble_plugin_api::InterfaceRequirement::viewer())
        {
            ("viewer", "viewer")
        } else if (file.bytes <= cobble_plugin_api::MAX_FILE_BYTES as u64 || ext == "zip")
            && super::plugin_reports::inbox::supports(&plugin.manifest, &file.name)
            && (!["zip", "mrpack"].contains(&ext.as_str())
                || file
                    .classification
                    .evidence
                    .iter()
                    .any(|e| e == "root_pack_metadata"))
        {
            ("report", "report")
        } else {
            continue;
        };
        result.push(Handler {
            id: format!("{prefix}:{}", plugin.manifest.id),
            label: plugin.manifest.name,
            kind: kind.into(),
            requires_game: false,
        });
    }
    Ok(result)
}
fn choices(
    app: &AppHandle,
    file: &Snapshot,
    bound: Option<&str>,
) -> Result<Vec<GameChoice>, String> {
    Ok(crate::games::file_inbox_games::targets(app, bound)?
        .into_iter()
        .map(|g| {
            let (version, loader) = (&file.classification.minecraft, &file.classification.loader);
            let (compatibility, reason) = if version.as_ref().is_some_and(|v| v != &g.version) {
                ("mismatch", "version_mismatch")
            } else if version.is_some()
                && loader
                    .as_ref()
                    .is_some_and(|l| g.loader.as_ref() != Some(l))
            {
                ("mismatch", "loader_mismatch")
            } else if version.is_some() {
                ("declared-match", "declared_version_match")
            } else {
                ("unknown", "metadata_missing")
            };
            GameChoice {
                id: g.id,
                name: g.name,
                version: g.version,
                available: g.available,
                compatibility: compatibility.into(),
                reason: Some(if !g.available { "game_running" } else { reason }.into()),
            }
        })
        .collect())
}
fn item(app: &AppHandle, r: &Resource, locale: &str) -> Result<Item, String> {
    Ok(Item {
        resource_id: r.id.clone(),
        name: r.file.name.clone(),
        bytes: r.file.bytes,
        kind: r.file.classification.kind.clone(),
        evidence: r.file.classification.evidence.clone(),
        handlers: handlers(&r.file, locale)?,
        games: choices(app, &r.file, r.scope.as_deref())?,
        expires_at: r.expires_at,
        declared_minecraft: r.file.classification.minecraft.clone(),
        declared_loader: r.file.classification.loader.clone(),
        error: r.file.classification.error.clone(),
    })
}
fn scope_allowed(stored: Option<&str>, bound: Option<&str>) -> bool {
    match (stored, bound) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}
fn capture(
    paths: Vec<PathBuf>,
    manifest: PluginManifest,
    session: Option<String>,
    scope: Option<String>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    if paths.len() > MAX_FILES {
        return Err("FILE_INBOX_TOO_MANY_FILES".into());
    }
    let expected = paths
        .iter()
        .map(|p| regular(p).map(|m| m.len()))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum();
    let _reservation = reserve(expected, paths.len())?;
    let mut resources = VecDeque::new();
    for path in paths {
        let file = Arc::new(snapshot(&path, cancel.as_deref())?);
        resources.push_back(Resource {
            id: token()?,
            manifest: manifest.clone(),
            session: session.clone(),
            scope: scope.clone(),
            cancel: cancel.clone(),
            file,
            expires: Instant::now() + TTL,
            expires_at: at() + TTL.as_secs(),
        });
    }
    let mut state = store().lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    prune(&mut state);
    state.resources.extend(resources);
    Ok(())
}
pub async fn capture_drop(app: AppHandle, paths: Vec<PathBuf>) -> Result<bool, String> {
    let Ok(plugin) = super::registry()?.resolve_enabled("file-inbox") else {
        return Ok(false);
    };
    if permitted(&plugin.manifest).is_err() {
        return Ok(false);
    }
    let count = paths.len();
    tauri::async_runtime::spawn_blocking(move || capture(paths, plugin.manifest, None, None, None))
        .await
        .map_err(|_| "FILE_INBOX_UNAVAILABLE")??;
    app.emit_to(
        "main",
        "plugin-file-inbox-drop",
        json!({"plugin_id":"file-inbox","count":count}),
    )
    .map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    Ok(true)
}
pub async fn pick(app: AppHandle, auth: &ViewAuthorization) -> Result<Value, String> {
    current(auth)?;
    let Some(files) = rfd::AsyncFileDialog::new()
        .set_title(if auth.locale == "zh-CN" {
            "选择投递文件"
        } else {
            "Choose files"
        })
        .pick_files()
        .await
    else {
        return pending(app, auth).await;
    };
    let paths = files.iter().map(|f| f.path().to_owned()).collect();
    let manifest = auth.manifest.clone();
    let session = Some(auth.session_id.clone());
    let scope = auth.instance_id.clone();
    let cancel = Some(auth.cancel.clone());
    tauri::async_runtime::spawn_blocking(move || capture(paths, manifest, session, scope, cancel))
        .await
        .map_err(|_| "FILE_INBOX_UNAVAILABLE")??;
    pending(app, auth).await
}
pub async fn pending(app: AppHandle, auth: &ViewAuthorization) -> Result<Value, String> {
    current(auth)?;
    let records = {
        let mut state = store().lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
        prune(&mut state);
        let mut records = Vec::new();
        for resource in &mut state.resources {
            if resource.manifest != auth.manifest
                || !scope_allowed(resource.scope.as_deref(), auth.instance_id.as_deref())
            {
                continue;
            }
            if resource.session.as_deref() == Some(&auth.session_id)
                || resource.session.is_none()
                || resource
                    .cancel
                    .as_ref()
                    .is_some_and(|c| c.load(Ordering::Acquire))
            {
                resource.session = Some(auth.session_id.clone());
                resource.cancel = Some(auth.cancel.clone());
                if resource.scope.is_none() {
                    resource.scope = auth.instance_id.clone();
                }
                records.push((
                    resource.id.clone(),
                    resource.file.clone(),
                    resource.scope.clone(),
                    resource.expires_at,
                ));
            }
        }
        records
    };
    let mut result = Vec::new();
    for (id, file, scope, expires_at) in records {
        let r = Resource {
            id,
            manifest: auth.manifest.clone(),
            session: Some(auth.session_id.clone()),
            scope,
            cancel: None,
            file,
            expires: Instant::now() + TTL,
            expires_at,
        };
        result.push(item(&app, &r, &auth.locale)?);
    }
    serde_json::to_value(result).map_err(|_| "FILE_INBOX_UNAVAILABLE".into())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenRequest {
    pub resource_id: String,
    pub handler_id: String,
    pub game_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleaseRequest {
    pub resource_ids: Vec<String>,
}
pub async fn release(
    _app: AppHandle,
    auth: &ViewAuthorization,
    args: ReleaseRequest,
) -> Result<Value, String> {
    current(auth)?;
    if args.resource_ids.len() > 32 {
        return Err("FILE_INBOX_RESOURCE_INVALID".into());
    }
    let mut state = store().lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    state.resources.retain(|r| {
        !(r.manifest == auth.manifest
            && r.session.as_deref() == Some(&auth.session_id)
            && args.resource_ids.contains(&r.id))
    });
    Ok(Value::Null)
}
pub async fn open(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: OpenRequest,
) -> Result<Value, String> {
    current(auth)?;
    let (file, scope) = {
        let mut state = store().lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
        prune(&mut state);
        let resource = state
            .resources
            .iter()
            .find(|r| {
                r.id == args.resource_id
                    && r.manifest == auth.manifest
                    && r.session.as_deref() == Some(&auth.session_id)
            })
            .ok_or("FILE_INBOX_RESOURCE_EXPIRED")?;
        (resource.file.clone(), resource.scope.clone())
    };
    let handler = handlers(&file, &auth.locale)?
        .into_iter()
        .find(|h| h.id == args.handler_id)
        .ok_or("FILE_INBOX_HANDLER_UNAVAILABLE")?;
    if let Some(id) = &args.game_id {
        if auth.instance_id.as_deref().is_some_and(|bound| bound != id)
            || scope.as_deref().is_some_and(|bound| bound != id)
        {
            return Err("PLUGIN_GAME_SCOPE".into());
        }
        crate::games::ensure_registered_game(&app, id).map_err(|_| "PLUGIN_GAME_SCOPE")?;
    }
    if handler.requires_game && args.game_id.is_none() {
        return Err("FILE_INBOX_GAME_REQUIRED".into());
    }
    let handoff = Handoff {
        id: token()?,
        resource_id: args.resource_id,
        manifest: auth.manifest.clone(),
        file,
        handler,
        game_id: args.game_id,
        locale: auth.locale.clone(),
        expires: Instant::now() + HANDOFF_TTL,
    };
    let event = json!({"handoff_id":handoff.id,"plugin_id":auth.manifest.id});
    {
        let mut state = store().lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
        prune(&mut state);
        if state
            .handoffs
            .iter()
            .any(|v| v.resource_id == handoff.resource_id)
        {
            return Err("FILE_INBOX_ACTION_PENDING".into());
        }
        if state.handoffs.len() >= 16 {
            return Err("FILE_INBOX_LIMIT".into());
        }
        state.handoffs.push_back(handoff);
    }
    app.emit_to("main", "plugin-file-inbox-open", event)
        .map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    Ok(json!({"requested":true}))
}
pub async fn open_link(
    app: AppHandle,
    auth: &ViewAuthorization,
    args: LinkRequest,
) -> Result<Value, String> {
    current(auth)?;
    let link = links::parse(&args.url)?;
    let game_id = args.game_id.or_else(|| auth.instance_id.clone());
    if let Some(id) = &game_id {
        if auth.instance_id.as_deref().is_some_and(|bound| bound != id) {
            return Err("PLUGIN_GAME_SCOPE".into());
        }
        crate::games::ensure_registered_game(&app, id).map_err(|_| "PLUGIN_GAME_SCOPE")?;
    }
    if link.kind != "modpack" && game_id.is_none() {
        return Err("FILE_INBOX_GAME_REQUIRED".into());
    }
    app.emit_to(
        "main",
        "plugin-file-inbox-market",
        json!({
            "provider":link.provider,"kind":link.kind,"project":link.project,
            "version":link.version,"gameId":game_id
        }),
    )
    .map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    Ok(json!({"requested":true}))
}
fn main_only(view: &Webview) -> Result<(), String> {
    if view.label() == "main" {
        Ok(())
    } else {
        Err("PLUGIN_VIEW_CALLER_DENIED".into())
    }
}
fn handoff(id: &str) -> Result<Handoff, String> {
    let mut state = store().lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
    prune(&mut state);
    state
        .handoffs
        .iter()
        .find(|h| h.id == id)
        .cloned()
        .ok_or_else(|| "FILE_INBOX_HANDOFF_EXPIRED".into())
}
fn validate_handoff(h: &Handoff) -> Result<(), String> {
    let plugin = super::registry()?
        .resolve_enabled(&h.manifest.id)
        .map_err(|_| "PLUGIN_VIEW_REVOKED")?;
    if plugin.manifest != h.manifest {
        return Err("PLUGIN_VIEW_STALE".into());
    }
    permitted(&plugin.manifest)?;
    if !handlers(&h.file, &h.locale)?
        .iter()
        .any(|v| v.id == h.handler.id)
    {
        return Err("FILE_INBOX_HANDLER_UNAVAILABLE".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn file_inbox_preview_handoff(
    app: AppHandle,
    webview: Webview,
    handoff_id: String,
) -> Result<Value, String> {
    main_only(&webview)?;
    let h = handoff(&handoff_id)?;
    validate_handoff(&h)?;
    let game = if let Some(id) = &h.game_id {
        crate::games::file_inbox_games::targets(&app, Some(id))?
            .into_iter()
            .next()
            .map(|g| json!({"id":g.id,"name":g.name,"version":g.version,"available":g.available}))
    } else {
        None
    };
    Ok(
        json!({"handoffId":h.id,"name":h.file.name,"bytes":h.file.bytes,"handler":h.handler,"game":game,"warning":if h.handler.id=="builtin:mod"{Some("executable_mod")}else{None}}),
    )
}
fn verify_snapshot(file: &Snapshot) -> Result<(), String> {
    let mut source = File::open(&file.path).map_err(|_| "FILE_INBOX_SOURCE_UNAVAILABLE")?;
    if source
        .metadata()
        .map_err(|_| "FILE_INBOX_SOURCE_UNAVAILABLE")?
        .len()
        != file.bytes
    {
        return Err("FILE_INBOX_SOURCE_CHANGED".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut total = 0u64;
    loop {
        let count = source
            .read(&mut buffer)
            .map_err(|_| "FILE_INBOX_SOURCE_UNAVAILABLE")?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > file.bytes {
            return Err("FILE_INBOX_SOURCE_CHANGED".into());
        }
        hash.update(&buffer[..count]);
    }
    if total != file.bytes || format!("{:x}", hash.finalize()) != file.sha256 {
        return Err("FILE_INBOX_SOURCE_CHANGED".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn file_inbox_apply_handoff(
    app: AppHandle,
    webview: Webview,
    handoff_id: String,
    confirmed: bool,
) -> Result<Value, String> {
    main_only(&webview)?;
    let h = {
        let mut state = store().lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
        prune(&mut state);
        let index = state
            .handoffs
            .iter()
            .position(|h| h.id == handoff_id)
            .ok_or("FILE_INBOX_HANDOFF_EXPIRED")?;
        state
            .handoffs
            .remove(index)
            .ok_or("FILE_INBOX_HANDOFF_EXPIRED")?
    };
    if !confirmed {
        return Err("PLUGIN_ACTION_CANCELLED".into());
    }
    validate_handoff(&h)?;
    let copy = h.file.clone();
    tauri::async_runtime::spawn_blocking(move || verify_snapshot(&copy))
        .await
        .map_err(|_| "FILE_INBOX_UNAVAILABLE")??;
    let result = if let Some(kind) = h.handler.id.strip_prefix("builtin:") {
        let app = app.clone();
        let file = h.file.clone();
        let kind = kind.to_owned();
        let game = h.game_id.clone();
        tauri::async_runtime::spawn_blocking(move||match kind.as_str(){"mrpack"=>Ok(json!({"kind":"mrpack","preview":crate::games::file_inbox_games::mrpack(&app,&file.path)?})),"curseforge-pack"=>Ok(json!({"kind":"curseforge-pack","preview":crate::games::file_inbox_games::curseforge(&app,&file.path)?})),_=>crate::games::file_inbox_games::import(&app,game.as_deref().ok_or("FILE_INBOX_GAME_REQUIRED")?,&file.path,&kind)}).await.map_err(|_|"FILE_INBOX_UNAVAILABLE")?
    } else if let Some(id) = h.handler.id.strip_prefix("viewer:") {
        let bytes = fs::read(&h.file.path).map_err(|_| "FILE_INBOX_SOURCE_UNAVAILABLE")?;
        let name = h.file.name.clone();
        let id = id.to_owned();
        tauri::async_runtime::spawn_blocking(move || {
            let format = super::BlueprintFormat::from_name(&name)
                .map_err(|_| "FILE_INBOX_FORMAT_INVALID")?;
            super::open(bytes, name, Some(id), format)
                .map(|viewer| json!({"kind":"viewer","viewer":viewer}))
                .map_err(|_| "FILE_INBOX_VIEWER_FAILED".into())
        })
        .await
        .map_err(|_| "FILE_INBOX_VIEWER_FAILED")?
    } else if let Some(id) = h.handler.id.strip_prefix("report:") {
        super::plugin_reports::inbox::analyze(
            app.clone(),
            h.file.path.clone(),
            id.to_owned(),
            h.locale.clone(),
            h.game_id.clone(),
        )
        .await
        .map(|report| json!({"kind":"report","report":report,"pluginId":id}))
    } else {
        Err("FILE_INBOX_HANDLER_UNAVAILABLE".into())
    };
    if result.is_ok() {
        let mut state = store().lock().map_err(|_| "FILE_INBOX_UNAVAILABLE")?;
        state.resources.retain(|r| r.id != h.resource_id);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("selected.zip");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        (dir, path)
    }
    #[test]
    fn file_inbox_ambiguous_archive_keeps_explicit_handlers() {
        let (dir, path) = archive(&[
            (
                "pack.mcmeta",
                br#"{"pack":{"pack_format":34,"description":"Fixture"}}"#,
            ),
            ("assets/example/a.txt", b"a"),
            ("shaders/a.fsh", b"shader"),
        ]);
        let c = classify(&path, dir.path(), "selected.zip");
        assert_eq!(c.kind, "ambiguous");
        assert_eq!(c.imports, vec!["resource-pack", "shader-pack"]);
        assert!(c.minecraft.is_none());
        assert!(c.loader.is_none());
    }
    #[test]
    fn file_inbox_unknown_zip_does_not_guess_pack_type() {
        let (dir, path) = archive(&[
            ("manifest.json", br#"{"application":"other"}"#),
            ("readme.txt", b"data"),
        ]);
        let c = classify(&path, dir.path(), "selected.zip");
        assert_eq!(c.kind, "unknown");
        assert!(c.imports.is_empty());
    }
    #[test]
    fn file_inbox_snapshot_is_immutable_and_verifies_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("latest.log");
        fs::write(&path, b"original").unwrap();
        let snap = snapshot(&path, None).unwrap();
        fs::write(&path, b"changed").unwrap();
        assert_eq!(fs::read(&snap.path).unwrap(), b"original");
        verify_snapshot(&snap).unwrap();
        fs::write(&snap.path, b"tampered").unwrap();
        assert_eq!(
            verify_snapshot(&snap).unwrap_err(),
            "FILE_INBOX_SOURCE_CHANGED"
        );
        let cancelled = AtomicBool::new(true);
        assert!(snapshot(&path, Some(&cancelled)).is_err());
    }
    #[test]
    fn file_inbox_requests_reject_paths_and_extra_fields() {
        assert!(
            serde_json::from_value::<OpenRequest>(
                json!({"resourceId":"a","handlerId":"builtin:mod","path":"C:/private"})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<ReleaseRequest>(json!({"resourceIds":[],"path":"C:/private"}))
                .is_err()
        );
    }
}
