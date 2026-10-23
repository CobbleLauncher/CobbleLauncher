// SPDX-License-Identifier: MIT
//! Window-owned immutable inspection models and bounded per-request Wasm reports.
use cobble_modpack_inspector::{
    InspectionDetail, InspectionDetailQuery, InspectionEnvelope, InspectionPage, InspectionQuery,
    InspectionRequest, PackSnapshot, PreparedInspection,
};
use cobble_plugin_api::{PluginManifest, Report};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};
use tauri::{AppHandle, WebviewWindow};

const PROTOCOL: &str = "cobble.mrpack-page@1";
const MAX_WINDOWS: usize = 4;
static NEXT: AtomicU64 = AtomicU64::new(1);
static JOBS: AtomicUsize = AtomicUsize::new(0);
static STORE: OnceLock<Mutex<Store>> = OnceLock::new();

struct Session {
    id: String,
    manifest: PluginManifest,
    component: Vec<u8>,
    model: PreparedInspection,
    locale: String,
    instance_id: Option<String>,
}
struct WindowState {
    epoch: u64,
    plugin_id: String,
    instance_id: Option<String>,
    cancel: Arc<AtomicBool>,
    session: Option<Arc<Session>>,
}
#[derive(Default)]
struct Store {
    windows: HashMap<String, WindowState>,
}
impl Store {
    fn begin(
        &mut self,
        window: &str,
        plugin_id: &str,
        instance_id: Option<&str>,
    ) -> Result<u64, String> {
        if !self.windows.contains_key(window) && self.windows.len() >= MAX_WINDOWS {
            return Err("打开的整合包分析过多，请先关闭一个".into());
        }
        self.close(window, None);
        let epoch = NEXT.fetch_add(1, Ordering::Relaxed);
        self.windows.insert(
            window.into(),
            WindowState {
                epoch,
                plugin_id: plugin_id.into(),
                instance_id: instance_id.map(str::to_owned),
                cancel: Arc::new(AtomicBool::new(false)),
                session: None,
            },
        );
        Ok(epoch)
    }
    fn close(&mut self, window: &str, id: Option<&str>) {
        let matches = self.windows.get(window).is_some_and(|state| {
            id.is_none_or(|id| {
                state
                    .session
                    .as_ref()
                    .is_some_and(|session| session.id == id)
            })
        });
        if matches && let Some(state) = self.windows.remove(window) {
            state.cancel.store(true, Ordering::Release);
        }
    }
    fn close_scoped(
        &mut self,
        window: &str,
        id: Option<&str>,
        plugin_id: Option<&str>,
        instance_id: Option<&str>,
    ) {
        if id.is_none()
            && plugin_id.is_some_and(|plugin_id| {
                self.windows.get(window).is_some_and(|state| {
                    state.plugin_id != plugin_id || state.instance_id.as_deref() != instance_id
                })
            })
        {
            return;
        }
        self.close(window, id);
    }
    fn current(&self, window: &str, epoch: u64) -> bool {
        self.windows
            .get(window)
            .is_some_and(|state| state.epoch == epoch && !state.cancel.load(Ordering::Acquire))
    }
}
fn store() -> &'static Mutex<Store> {
    STORE.get_or_init(Default::default)
}
fn lock() -> Result<std::sync::MutexGuard<'static, Store>, String> {
    store().lock().map_err(|_| "整合包分析状态不可用".into())
}
fn close_epoch(window: &str, epoch: u64) {
    if let Ok(mut store) = lock()
        && store.current(window, epoch)
    {
        store.close(window, None);
    }
}

struct Job;
impl Job {
    fn acquire() -> Result<Self, String> {
        JOBS.try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            (count < MAX_WINDOWS).then_some(count + 1)
        })
        .map(|_| Self)
        .map_err(|_| "分析正在结束，请稍后重试".into())
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        JOBS.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(crate) fn revoke_window(window: &str) {
    if let Ok(mut store) = lock() {
        store.close(window, None);
    }
}
pub(super) fn revoke_plugin(plugin_id: &str) {
    if let Ok(mut store) = lock() {
        store.windows.retain(|_, state| {
            if state.plugin_id == plugin_id {
                state.cancel.store(true, Ordering::Release);
                false
            } else {
                true
            }
        });
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    session_id: String,
    summary: serde_json::Value,
    prepare_duration_ms: f64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageReply {
    session_id: String,
    revision: u64,
    page: InspectionPage,
    report: Report,
    guest_duration_ms: f64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailReply {
    session_id: String,
    revision: u64,
    detail: InspectionDetail,
}

#[tauri::command]
pub async fn begin_pack_inspection(
    app: AppHandle,
    window: WebviewWindow,
    plugin_id: String,
    compare: bool,
    locale: Option<String>,
    instance_id: Option<String>,
) -> Result<Option<Started>, String> {
    if let Some(id) = &instance_id {
        crate::games::ensure_registered_game(&app, id)?;
    }
    let locale = locale.unwrap_or_else(|| "en".into());
    super::extension_contributions::validate_locale(&locale)?;
    let plugin = super::plugin_reports::selected_plugin(&plugin_id)?;
    if !plugin
        .manifest
        .report_protocols
        .iter()
        .any(|protocol| protocol == PROTOCOL)
    {
        return Err("此插件不支持分页整合包分析".into());
    }
    let label = window.label().to_owned();
    let epoch = lock()?.begin(&label, &plugin_id, instance_id.as_deref())?;
    let job = match Job::acquire() {
        Ok(job) => job,
        Err(error) => {
            close_epoch(&label, epoch);
            return Err(error);
        }
    };
    let prepared = super::pack_inspection::prepare_pack_report_for_extensions(
        &window,
        compare,
        &plugin.manifest.file_extensions,
    )
    .await;
    if !lock()?.current(&label, epoch) {
        return Ok(None);
    }
    let Some((bytes, media, input_duration)) = (match prepared {
        Ok(value) => value,
        Err(error) => {
            close_epoch(&label, epoch);
            return Err(error);
        }
    }) else {
        close_epoch(&label, epoch);
        return Ok(None);
    };
    if !lock()?.current(&label, epoch) {
        return Ok(None);
    }
    if let Some(id) = &instance_id
        && let Err(error) = crate::games::ensure_registered_game(&app, id)
    {
        close_epoch(&label, epoch);
        return Err(error);
    }
    let started = Instant::now();
    let session = tauri::async_runtime::spawn_blocking(move || {
        let _job = job;
        super::plugin_reports::verify_current(&plugin.manifest)?;
        let component = super::plugin_reports::package(&plugin)?;
        let request = if media == "application/json" {
            InspectionRequest {
                current: PackSnapshot {
                    index: serde_json::from_slice(&bytes).map_err(|_| "整合包索引无效")?,
                    overrides: vec![],
                },
                baseline: None,
            }
        } else {
            serde_json::from_slice(&bytes).map_err(|_| "整合包元数据无效")?
        };
        let model = PreparedInspection::new(request).map_err(|_| "无法解析整合包元数据")?;
        super::plugin_reports::verify_current(&plugin.manifest)?;
        Ok::<_, String>(Arc::new(Session {
            id: format!("inspection-{epoch}"),
            manifest: plugin.manifest,
            component,
            model,
            locale,
            instance_id,
        }))
    })
    .await
    .map_err(|_| "整合包分析准备失败".to_owned())
    .and_then(|result| result);
    if !lock()?.current(&label, epoch) {
        return Ok(None);
    }
    let session = match session {
        Ok(session) => session,
        Err(error) => {
            close_epoch(&label, epoch);
            return Err(error);
        }
    };
    let summary = serde_json::to_value(session.model.summary()).map_err(|_| "整合包摘要无效")?;
    let mut store = lock()?;
    if !store.current(&label, epoch) {
        return Ok(None);
    }
    store.windows.get_mut(&label).ok_or("分析已关闭")?.session = Some(session.clone());
    Ok(Some(Started {
        session_id: session.id.clone(),
        summary,
        prepare_duration_ms: (input_duration + started.elapsed()).as_secs_f64() * 1000.0,
    }))
}

fn operation(
    window: &str,
    session_id: &str,
) -> Result<(Arc<Session>, u64, Arc<AtomicBool>), String> {
    let session = lock()?
        .windows
        .get(window)
        .and_then(|state| state.session.clone())
        .filter(|session| session.id == session_id)
        .ok_or("整合包分析已关闭，请重新选择文件")?;
    if let Err(error) = super::plugin_reports::verify_current(&session.manifest) {
        lock()?.close(window, Some(session_id));
        return Err(error);
    }
    let mut store = lock()?;
    let state = store
        .windows
        .get_mut(window)
        .filter(|state| {
            state
                .session
                .as_ref()
                .is_some_and(|session| session.id == session_id)
        })
        .ok_or("整合包分析已关闭")?;
    state.cancel.store(true, Ordering::Release);
    state.cancel = Arc::new(AtomicBool::new(false));
    state.epoch = NEXT.fetch_add(1, Ordering::Relaxed);
    Ok((session, state.epoch, state.cancel.clone()))
}

#[tauri::command]
pub async fn query_pack_inspection(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    query: InspectionQuery,
    revision: u64,
    locale: Option<String>,
) -> Result<Option<PageReply>, String> {
    let job = Job::acquire()?;
    let label = window.label().to_owned();
    let (session, epoch, cancel) = operation(&label, &session_id)?;
    if let Some(id) = &session.instance_id {
        crate::games::ensure_registered_game(&app, id)?;
    }
    let locale = locale.unwrap_or_else(|| session.locale.clone());
    super::extension_contributions::validate_locale(&locale)?;
    let show_dependencies = super::extension_settings::effective_settings(
        &session.manifest,
        session.instance_id.as_deref(),
    )?
    .get("show_dependencies")
    .and_then(serde_json::Value::as_bool)
    .unwrap_or(true);
    let mut query = query;
    query.show_dependencies = show_dependencies;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _job = job;
        let page = session
            .model
            .query(&query)
            .map_err(|error| error.to_string())?;
        let bytes = serde_json::to_vec(&InspectionEnvelope::page(page.clone()))
            .map_err(|_| "无法准备分析页")?;
        let host = std::env::current_exe()
            .map_err(|_| "无法找到分析宿主")?
            .with_file_name(if cfg!(windows) {
                "cobble-plugin-host.exe"
            } else {
                "cobble-plugin-host"
            });
        let started = Instant::now();
        let result =
            if session.manifest.api_version == cobble_plugin_api::REPORT_CONTEXT_API_VERSION {
                let context = super::extension_settings::report_context(
                    &session.manifest,
                    session.instance_id.as_deref(),
                    &locale,
                )?;
                cobble_plugin_host::run_report_with_context(
                    &host,
                    &session.component,
                    &bytes,
                    "application/vnd.cobble.mrpack-inspection+json",
                    cancel.clone(),
                    &context,
                )
            } else {
                cobble_plugin_host::run_report(
                    &host,
                    &session.component,
                    &bytes,
                    "application/vnd.cobble.mrpack-inspection+json",
                    cancel.clone(),
                )
            };
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        let report = result.map_err(|_| "插件分页分析失败")?;
        super::plugin_reports::verify_current(&session.manifest)?;
        Ok::<_, String>(Some(PageReply {
            session_id: session.id.clone(),
            revision,
            page,
            report,
            guest_duration_ms: started.elapsed().as_secs_f64() * 1000.0,
        }))
    })
    .await
    .map_err(|_| "分析宿主意外退出".to_owned())?;
    if !lock()?.current(&label, epoch) {
        return Ok(None);
    }
    result
}

#[tauri::command]
pub async fn detail_pack_inspection(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    query: InspectionDetailQuery,
    revision: u64,
) -> Result<Option<DetailReply>, String> {
    let label = window.label().to_owned();
    let (session, epoch, _) = operation(&label, &session_id)?;
    if let Some(id) = &session.instance_id {
        crate::games::ensure_registered_game(&app, id)?;
    }
    let detail = session
        .model
        .detail(&query)
        .map_err(|error| error.to_string())?;
    super::plugin_reports::verify_current(&session.manifest)?;
    if !lock()?.current(&label, epoch) {
        return Ok(None);
    }
    Ok(Some(DetailReply {
        session_id,
        revision,
        detail,
    }))
}

#[tauri::command]
pub fn close_pack_inspection(
    window: WebviewWindow,
    session_id: Option<String>,
    plugin_id: Option<String>,
    instance_id: Option<String>,
) -> Result<(), String> {
    lock()?.close_scoped(
        window.label(),
        session_id.as_deref(),
        plugin_id.as_deref(),
        instance_id.as_deref(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_and_window_close_cancel_pending_generations() {
        let mut store = Store::default();
        let first = store.begin("owned", "pack", None).unwrap();
        let cancel = store.windows["owned"].cancel.clone();
        let second = store.begin("owned", "pack", None).unwrap();
        assert!(cancel.load(Ordering::Acquire));
        assert!(!store.current("owned", first));
        assert!(store.current("owned", second));
        store.close("other", None);
        assert!(store.current("owned", second));
        store.close("owned", None);
        assert!(!store.current("owned", second));
    }
    #[test]
    fn pending_windows_are_bounded_and_replacement_remains_available() {
        let mut store = Store::default();
        for index in 0..MAX_WINDOWS {
            store
                .begin(&format!("window-{index}"), "pack", None)
                .unwrap();
        }
        assert!(store.begin("extra", "pack", None).is_err());
        assert!(store.begin("window-0", "replacement", None).is_ok());
    }
    #[test]
    fn closing_old_plugin_pending_picker_preserves_new_plugin() {
        let mut store = Store::default();
        store.begin("owned", "plugin-a", None).unwrap();
        let new = store.begin("owned", "plugin-b", None).unwrap();
        store.close_scoped("owned", None, Some("plugin-a"), None);
        assert!(store.current("owned", new));
        store.close_scoped("owned", None, Some("plugin-b"), None);
        assert!(!store.current("owned", new));
    }
    #[test]
    fn stale_same_plugin_pending_close_preserves_new_instance_context() {
        let mut store = Store::default();
        store.begin("owned", "pack", Some("first")).unwrap();
        let current = store.begin("owned", "pack", Some("second")).unwrap();
        store.close_scoped("owned", None, Some("pack"), Some("first"));
        assert!(store.current("owned", current));
        store.close_scoped("owned", None, Some("pack"), Some("second"));
        assert!(!store.current("owned", current));
    }
}
