// SPDX-License-Identifier: MIT
//! Session-owned language snapshots and existing report-Wasm execution. No input writes.
use super::plugin_views::ViewAuthorization;
use cobble_plugin_api::{PermissionNode, PluginManifest, Report};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::AppHandle;
#[path = "lang_package_input.rs"]
mod input;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub id: String,
    pub namespace: Option<String>,
    pub locale: String,
    pub name: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionInfo {
    pub selection_id: String,
    pub source_name: String,
    pub kind: String,
    pub pack_format: Option<i32>,
    pub files: Vec<FileInfo>,
}
#[derive(Clone, Serialize)]
struct Document {
    id: String,
    name: String,
    locale: String,
    text: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckRequest {
    pub selection_id: String,
    pub namespace: Option<String>,
    pub baseline_locale: Option<String>,
    pub target_locale: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentRequest {
    pub selection_id: String,
    pub file_id: String,
}
#[derive(Clone)]
struct Job {
    cancel: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}
struct Slot {
    plugin: String,
    created: Instant,
    source: input::Source,
    job: Option<Job>,
}
static SLOTS: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
fn slots() -> &'static Mutex<HashMap<String, Slot>> {
    SLOTS.get_or_init(Default::default)
}
pub fn permitted(manifest: &PluginManifest) -> Result<(), String> {
    let allowed=manifest.permissions.as_deref().unwrap_or_default().iter().any(|node|matches!(node,PermissionNode::SelectedFileRead{extensions} if !extensions.is_empty() && extensions.len()==manifest.file_extensions.len() && extensions.iter().all(|v|matches!(v.as_str(),"json"|"zip") && manifest.file_extensions.contains(v))));
    if manifest.api_version == "0.2.0"
        && manifest.capabilities == ["report"]
        && manifest.report_protocols == [cobble_plugin_api::LANG_PACKAGE_PROTOCOL]
        && allowed
    {
        Ok(())
    } else {
        Err("PLUGIN_PERMISSION_DENIED".into())
    }
}
fn active(auth: &ViewAuthorization) -> Result<(), String> {
    if auth.cancel.load(Ordering::Acquire) {
        Err("PLUGIN_VIEW_CLOSED".into())
    } else {
        permitted(&auth.manifest)
    }
}
fn valid_slot<'a>(
    map: &'a mut HashMap<String, Slot>,
    auth: &ViewAuthorization,
    id: &str,
) -> Result<&'a mut Slot, String> {
    let slot = map
        .get_mut(&auth.session_id)
        .ok_or("LANG_PACKAGE_SELECTION_EXPIRED")?;
    if slot.plugin != auth.manifest.id
        || slot.source.info.selection_id != id
        || slot.created.elapsed() > Duration::from_secs(15 * 60)
    {
        return Err("LANG_PACKAGE_SELECTION_EXPIRED".into());
    }
    Ok(slot)
}
pub fn revoke_session(session: &str) {
    if let Ok(mut map) = slots().lock()
        && let Some(slot) = map.remove(session)
        && let Some(job) = slot.job
    {
        job.cancel.store(true, Ordering::Release);
    }
}
pub async fn pick(
    _app: AppHandle,
    auth: &ViewAuthorization,
) -> Result<Option<SelectionInfo>, String> {
    active(auth)?;
    let extensions: Vec<_> = auth
        .manifest
        .file_extensions
        .iter()
        .map(String::as_str)
        .collect();
    let Some(files) = rfd::AsyncFileDialog::new()
        .set_title(if auth.locale == "zh-CN" {
            "选择资源包 ZIP 或语言 JSON"
        } else {
            "Choose resource pack ZIP or language JSON"
        })
        .add_filter("Language JSON / resource pack ZIP", &extensions)
        .pick_files()
        .await
    else {
        return Ok(None);
    };
    active(auth)?;
    let paths: Vec<_> = files.iter().map(|f| f.path().to_owned()).collect();
    if paths.iter().any(|path| {
        !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                auth.manifest
                    .file_extensions
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(ext))
            })
    }) {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    let mut source = tauri::async_runtime::spawn_blocking(move || input::read(paths))
        .await
        .map_err(|_| "LANG_PACKAGE_IO")??;
    active(auth)?;
    let mut nonce = [0u8; 32];
    getrandom::fill(&mut nonce).map_err(|_| "LANG_PACKAGE_IO")?;
    source.info.selection_id = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let info = source.info.clone();
    let mut map = slots().lock().map_err(|_| "LANG_PACKAGE_IO")?;
    map.retain(|_, slot| slot.created.elapsed() < Duration::from_secs(15 * 60));
    if map.len() >= 8 && !map.contains_key(&auth.session_id) {
        return Err("LANG_PACKAGE_BUSY".into());
    }
    if let Some(old) = map.insert(
        auth.session_id.clone(),
        Slot {
            plugin: auth.manifest.id.clone(),
            created: Instant::now(),
            source,
            job: None,
        },
    ) && let Some(job) = old.job
    {
        job.cancel.store(true, Ordering::Release);
    }
    Ok(Some(info))
}
pub async fn document(
    auth: &ViewAuthorization,
    args: DocumentRequest,
) -> Result<serde_json::Value, String> {
    active(auth)?;
    let mut map = slots().lock().map_err(|_| "LANG_PACKAGE_IO")?;
    let slot = valid_slot(&mut map, auth, &args.selection_id)?;
    let file = slot
        .source
        .files
        .iter()
        .find(|f| f.id == args.file_id)
        .ok_or("LANG_PACKAGE_FILE_NOT_FOUND")?;
    Ok(serde_json::json!({"name":file.name,"text":file.text}))
}
struct Running {
    session: String,
    job: Job,
}
impl Drop for Running {
    fn drop(&mut self) {
        super::plugin_views::clear_job(&self.session, &self.job.cancel);
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
        if let Ok(mut map) = slots().lock()
            && let Some(slot) = map.get_mut(&self.session)
            && slot
                .job
                .as_ref()
                .is_some_and(|job| Arc::ptr_eq(&job.cancel, &self.job.cancel))
        {
            slot.job = None;
        }
        self.job.finished.store(true, Ordering::Release);
    }
}
pub async fn check(
    _app: AppHandle,
    auth: &ViewAuthorization,
    args: CheckRequest,
) -> Result<Option<Report>, String> {
    active(auth)?;
    let (files, source_name, pack_format, job) = {
        let mut map = slots().lock().map_err(|_| "LANG_PACKAGE_IO")?;
        let slot = valid_slot(&mut map, auth, &args.selection_id)?;
        if slot
            .job
            .as_ref()
            .is_some_and(|job| !job.finished.load(Ordering::Acquire))
        {
            return Err("LANG_PACKAGE_BUSY".into());
        }
        let ids: Vec<_> = slot
            .source
            .info
            .files
            .iter()
            .filter(|f| {
                f.namespace == args.namespace
                    && (f.locale == args.target_locale
                        || args.baseline_locale.as_ref() == Some(&f.locale))
            })
            .map(|f| f.id.as_str())
            .collect();
        let files: Vec<_> = slot
            .source
            .files
            .iter()
            .filter(|f| ids.contains(&f.id.as_str()))
            .cloned()
            .collect();
        if !files.iter().any(|f| f.locale == args.target_locale)
            || args
                .baseline_locale
                .as_ref()
                .is_some_and(|locale| !files.iter().any(|f| &f.locale == locale))
        {
            return Err("LANG_PACKAGE_SELECTION_INVALID".into());
        }
        if ACTIVE
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < 2).then_some(count + 1)
            })
            .is_err()
        {
            return Err("LANG_PACKAGE_BUSY".into());
        }
        let job = Job {
            cancel: Arc::new(AtomicBool::new(false)),
            finished: Arc::new(AtomicBool::new(false)),
        };
        slot.job = Some(job.clone());
        (
            files,
            slot.source.info.source_name.clone(),
            slot.source.info.pack_format,
            job,
        )
    };
    let running = Running {
        session: auth.session_id.clone(),
        job: job.clone(),
    };
    super::plugin_views::register_job(&auth.session_id, job.cancel.clone())?;
    let bytes=serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"protocol":cobble_plugin_api::LANG_PACKAGE_PROTOCOL,"sourceName":source_name,"packFormat":pack_format,"namespace":args.namespace,"baselineLocale":args.baseline_locale,"targetLocale":args.target_locale,"files":files})).map_err(|_|"LANG_PACKAGE_IO")?;
    if bytes.len() > cobble_plugin_api::MAX_FILE_BYTES {
        return Err("LANG_PACKAGE_LIMIT".into());
    }
    let manifest = auth.manifest.clone();
    let locale = if auth.locale == "zh-CN" {
        "zh-CN"
    } else {
        "en"
    }
    .to_owned();
    let final_cancel = job.cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _running = running;
        if job.cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        let plugin = super::plugin_reports::selected_plugin(&manifest.id)?;
        if plugin.manifest != manifest {
            return Err("PLUGIN_VIEW_STALE".into());
        }
        let component = super::plugin_reports::package(&plugin)?;
        let host = std::env::current_exe()
            .map_err(|_| "LANG_PACKAGE_IO")?
            .with_file_name(if cfg!(windows) {
                "cobble-plugin-host.exe"
            } else {
                "cobble-plugin-host"
            });
        let context =
            cobble_plugin_host::ReportHostContext::new(&manifest, &locale, BTreeMap::new())
                .map_err(|_| "LANG_PACKAGE_IO")?;
        let report = cobble_plugin_host::run_report_with_context(
            &host,
            &component,
            &bytes,
            "application/json",
            job.cancel.clone(),
            &context,
        )
        .map_err(|error| {
            if error.code == "CANCEL_REQUESTED" {
                "LANG_PACKAGE_CANCELLED".to_owned()
            } else {
                error.code
            }
        })?;
        if job.cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        super::plugin_reports::verify_current(&manifest)?;
        Ok(Some(report))
    })
    .await
    .map_err(|_| "LANG_PACKAGE_IO")?;
    active(auth)?;
    if final_cancel.load(Ordering::Acquire) {
        return Ok(None);
    }
    result
}
pub async fn cancel(auth: &ViewAuthorization) -> Result<(), String> {
    let job = slots()
        .lock()
        .map_err(|_| "LANG_PACKAGE_IO")?
        .remove(&auth.session_id)
        .and_then(|slot| slot.job);
    if let Some(job) = job {
        job.cancel.store(true, Ordering::Release);
        let start = Instant::now();
        while !job.finished.load(Ordering::Acquire) {
            if start.elapsed() > Duration::from_secs(11) {
                return Err("LANG_PACKAGE_CANCEL_TIMEOUT".into());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn auth(session: &str) -> ViewAuthorization {
        ViewAuthorization {
            manifest: serde_json::from_str(include_str!(
                "../../../../examples/lang-doctor/plugin.json"
            ))
            .unwrap(),
            instance_id: None,
            keybinding_target_id: None,
            handoff_id: None,
            config_restore_target: None,
            locale: "en".into(),
            theme: "light".into(),
            appearance: None,
            cancel: Arc::new(AtomicBool::new(false)),
            session_id: session.into(),
        }
    }
    #[test]
    fn language_selection_is_bound_to_issuing_plugin_session_token_and_ttl() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("en_us.json");
        std::fs::write(&path, "{}").unwrap();
        let mut source = input::read(vec![path]).unwrap();
        source.info.selection_id = "a".repeat(64);
        let owner = auth("first");
        let mut map = HashMap::new();
        map.insert(
            "first".into(),
            Slot {
                plugin: owner.manifest.id.clone(),
                created: Instant::now(),
                source,
                job: None,
            },
        );
        assert!(valid_slot(&mut map, &owner, &"a".repeat(64)).is_ok());
        assert!(valid_slot(&mut map, &auth("second"), &"a".repeat(64)).is_err());
        let mut other = auth("first");
        other.manifest.id = "other-plugin".into();
        assert!(valid_slot(&mut map, &other, &"a".repeat(64)).is_err());
        assert!(valid_slot(&mut map, &owner, &"b".repeat(64)).is_err());
        map.get_mut("first").unwrap().created = Instant::now() - Duration::from_secs(901);
        assert!(valid_slot(&mut map, &owner, &"a".repeat(64)).is_err());
    }
    #[test]
    fn language_picker_requires_exact_declared_formats_and_language_protocol() {
        let owner = auth("test");
        permitted(&owner.manifest).unwrap();
        let mut manifest = owner.manifest.clone();
        manifest.permissions = Some(vec![PermissionNode::UiWebview {}]);
        assert!(permitted(&manifest).is_err());
        manifest = owner.manifest;
        manifest.report_protocols = vec!["cobble.pack-metadata@1".into()];
        assert!(permitted(&manifest).is_err());
    }
}
