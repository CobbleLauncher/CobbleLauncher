// SPDX-License-Identifier: MIT
//! Installed Wasm Provider -> restricted public HTTPS -> managed Viewer resource.
use cobble_net::{Broker, BrokerConfig, BufferedResponse};
use cobble_plugin_api::{InstalledPlugin, MAX_FILE_BYTES, PluginManifest};
use cobble_plugin_host::provider::{
    self, GetResponse, ProviderOperation, ProviderOutput, ProviderRequest, ResourceKey, SearchQuery,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, WebviewWindow};

struct ActiveJob {
    request_id: String,
    provider_id: String,
    cancel: Arc<AtomicBool>,
}
type Jobs = HashMap<String, ActiveJob>;
static JOBS: OnceLock<Mutex<Jobs>> = OnceLock::new();
fn jobs() -> &'static Mutex<Jobs> {
    JOBS.get_or_init(Default::default)
}
struct Job {
    window: String,
    request_id: String,
    provider_id: String,
    cancel: Arc<AtomicBool>,
}
impl Job {
    fn begin(window: &str, request_id: String, provider_id: &str) -> Result<Self, String> {
        super::valid_id(provider_id)?;
        if request_id.is_empty()
            || request_id.len() > 80
            || !request_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("蓝图请求标识无效".into());
        }
        let mut all = jobs().lock().map_err(|_| "蓝图请求状态不可用")?;
        if !all.contains_key(window) && all.len() >= 8 {
            return Err("蓝图请求过多".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(previous) = all.insert(
            window.into(),
            ActiveJob {
                request_id: request_id.clone(),
                provider_id: provider_id.into(),
                cancel: cancel.clone(),
            },
        ) {
            previous.cancel.store(true, Ordering::Release);
        }
        Ok(Self {
            window: window.into(),
            request_id,
            provider_id: provider_id.into(),
            cancel,
        })
    }
    fn check(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Acquire) {
            Err("蓝图请求已取消".into())
        } else {
            Ok(())
        }
    }
    fn progress(&self, window: &WebviewWindow, phase: &'static str, completed: u32, total: u32) {
        let _ = window.emit(
            "blueprint-provider-progress",
            Progress {
                request_id: &self.request_id,
                phase,
                completed,
                total,
            },
        );
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Ok(mut all) = jobs().lock()
            && all
                .get(&self.window)
                .is_some_and(|job| Arc::ptr_eq(&job.cancel, &self.cancel))
        {
            all.remove(&self.window);
        }
    }
}
#[derive(Clone, Serialize)]
struct Progress<'a> {
    request_id: &'a str,
    phase: &'static str,
    completed: u32,
    total: u32,
}
pub fn revoke_window(label: &str) {
    if let Ok(mut all) = jobs().lock()
        && let Some(job) = all.remove(label)
    {
        job.cancel.store(true, Ordering::Release);
    }
}
pub fn revoke_plugin(id: &str) {
    if let Ok(mut all) = jobs().lock() {
        all.retain(|_, job| {
            if job.provider_id == id {
                job.cancel.store(true, Ordering::Release);
                false
            } else {
                true
            }
        });
    }
}
#[tauri::command]
pub fn cancel_blueprint_provider(window: WebviewWindow, request_id: String) -> Result<(), String> {
    let all = jobs().lock().map_err(|_| "蓝图请求状态不可用")?;
    if let Some(job) = all.get(window.label())
        && job.request_id == request_id
    {
        job.cancel.store(true, Ordering::Release);
    }
    Ok(())
}
fn is_provider(item: &InstalledPlugin) -> bool {
    item.enabled
        && item.problem.is_none()
        && item.manifest.api_version == cobble_plugin_api::PROVIDER_API_VERSION
        && item.manifest.capabilities == ["blueprint-provider"]
        && !item.manifest.provider_origins.is_empty()
        && item
            .manifest
            .supports_interface(&cobble_plugin_api::InterfaceRequirement::blueprint_provider())
}
fn selected(provider_id: &str) -> Result<InstalledPlugin, String> {
    selected_in(&super::registry()?, provider_id)
}
fn selected_in(
    registry: &cobble_plugin_host::PluginRegistry,
    provider_id: &str,
) -> Result<InstalledPlugin, String> {
    super::valid_id(provider_id)?;
    let item = registry
        .resolve_enabled(provider_id)
        .map_err(|error| error.to_string())?;
    if !is_provider(&item) {
        return Err("请先安装并启用所选蓝图来源插件".into());
    }
    Ok(item)
}
fn verify(manifest: &PluginManifest, job: &Job) -> Result<(), String> {
    job.check()?;
    if selected(&job.provider_id)?.manifest != *manifest {
        return Err("蓝图插件已变化，请重试".into());
    }
    Ok(())
}
#[derive(Serialize)]
pub struct Availability {
    available: bool,
    provider_id: Option<String>,
    providers: Vec<ProviderChoice>,
    viewer_available: bool,
    interface: cobble_plugin_host::interfaces::InterfaceResolution,
}
#[derive(Serialize)]
pub struct ProviderChoice {
    id: String,
    name: String,
    origins: Vec<String>,
}
#[tauri::command]
pub fn get_blueprint_provider_availability() -> Availability {
    let requirement = cobble_plugin_api::InterfaceRequirement::blueprint_provider();
    let registry = super::registry().ok();
    let catalogs = crate::plugin_catalogs::store().ok();
    let interface = registry
        .as_ref()
        .and_then(|registry| {
            registry
                .resolve_interface(&requirement, catalogs.as_ref())
                .ok()
        })
        .unwrap_or_else(|| cobble_plugin_host::interfaces::InterfaceResolution {
            id: requirement.id.clone(),
            version: requirement.version.clone(),
            implementations: vec![],
            candidates: vec![],
        });
    let installed = registry
        .as_ref()
        .and_then(|registry| registry.list().ok())
        .unwrap_or_default();
    let ready = cobble_plugin_host::interfaces::ready_plugins(&installed);
    let providers: Vec<_> = ready
        .iter()
        .filter(|item| is_provider(item))
        .map(|item| ProviderChoice {
            id: item.manifest.id.clone(),
            name: item.manifest.name.clone(),
            origins: item.manifest.provider_origins.clone(),
        })
        .collect();
    let viewer_available = ready.iter().any(|item| {
        item.manifest
            .supports_interface(&cobble_plugin_api::InterfaceRequirement::viewer())
    });

    Availability {
        available: !providers.is_empty(),
        provider_id: (providers.len() == 1).then(|| providers[0].id.clone()),
        providers,
        viewer_available,
        interface,
    }
}
fn host() -> Result<PathBuf, String> {
    let path = crate::runtime_resources::plugin_host().map_err(|_| "无法找到插件运行程序")?;
    if !path.is_file() {
        return Err("插件运行程序不可用".into());
    }
    Ok(path)
}
async fn fetch(
    job: &Job,
    broker: &Broker,
    url: &str,
    maximum: u64,
    approved_origins: &[String],
) -> Result<BufferedResponse, String> {
    use super::plugin_access_log::{Outcome, Target, record};
    // All production callers have resolved and verified this installed provider before fetch.
    if provider::validate_url(url, approved_origins).is_err() {
        record(
            &job.provider_id,
            "network",
            "https.get",
            Target::Origin(url),
            Outcome::Denied,
        );
        return Err("蓝图地址未获授权".into());
    }
    let response = broker.get(url, None, maximum);
    tokio::pin!(response);
    loop {
        tokio::select! {
            result = &mut response => {
                let outcome = if job.check().is_err() { Outcome::Cancelled } else if result.is_ok() { Outcome::Completed } else { Outcome::Failed };
                record(&job.provider_id, "network", "https.get", Target::Origin(url), outcome);
                job.check()?;
                return result.map_err(|_| "无法读取蓝图服务，请稍后重试".to_owned());
            }
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if let Err(error) = job.check() {
                    record(&job.provider_id, "network", "https.get", Target::Origin(url), Outcome::Cancelled);
                    return Err(error);
                }
            },
        }
    }
}
fn unfiltered_retry(request: &mut ProviderRequest, error: &cobble_plugin_api::PluginError) -> bool {
    if error.code != "PROVIDER_UNSUPPORTED_GAME_VERSION"
        || request.operation != ProviderOperation::Search
    {
        return false;
    }
    let Some(search) = request
        .search
        .as_mut()
        .filter(|search| !search.java_release.is_empty())
    else {
        return false;
    };
    search.java_release.clear();
    true
}
fn diagnostic_code(code: &str) -> &str {
    if !code.is_empty()
        && code.len() <= 80
        && code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        code
    } else {
        "PLUGIN_INVALID_ERROR"
    }
}
async fn execute(
    window: &WebviewWindow,
    job: &Job,
    request: ProviderRequest,
    phase: &'static str,
) -> Result<(ProviderOutput, PluginManifest, Broker, bool), String> {
    request.validate().map_err(|_| "蓝图筛选条件无效")?;
    let plugin = selected(&job.provider_id)?;
    let component = super::plugin_reports::package(&plugin)?;
    let executable = host()?;
    let origins = plugin.manifest.provider_origins.clone();
    let mut worker_request = request.clone();
    let worker_cancel = job.cancel.clone();
    let worker_origins = origins.clone();
    let (mut session, plan, request, version_filter_applied) =
        tauri::async_runtime::spawn_blocking(move || {
            let mut session =
                provider::ProviderSession::open(&executable, &component, worker_cancel.clone())?;
            let first = session.plan(&worker_request, &worker_origins, worker_cancel.clone());
            let mut version_filter_applied = true;
            let plan = match first {
                Err(error) if unfiltered_retry(&mut worker_request, &error) => {
                    version_filter_applied = false;
                    // A rejected RPC closes its worker. Retry with a fresh owned
                    // instance, before any metadata HTTP request has occurred.
                    session = provider::ProviderSession::open(
                        &executable,
                        &component,
                        worker_cancel.clone(),
                    )?;
                    session.plan(&worker_request, &worker_origins, worker_cancel)?
                }
                value => value?,
            };
            Ok::<_, cobble_plugin_api::PluginError>((
                session,
                plan,
                worker_request,
                version_filter_applied,
            ))
        })
        .await
        .map_err(|_| {
            tracing::warn!(event = "blueprint_provider_worker_failed", phase = "plan");
            "蓝图插件运行失败"
        })?
        .map_err(|error| {
            let code = diagnostic_code(&error.code);
            tracing::warn!(event = "blueprint_provider_failed", phase = "plan", code);
            format!("蓝图插件无法生成请求（{code}）")
        })?;
    verify(&plugin.manifest, job)?;
    let broker = Broker::new(BrokerConfig {
        max_redirects: 0,
        ..BrokerConfig::default()
    })
    .map_err(|_| "蓝图网络服务不可用")?;
    let mut responses = Vec::with_capacity(plan.len());
    for (index, entry) in plan.iter().enumerate() {
        verify(&plugin.manifest, job)?;
        job.progress(window, phase, index as u32, plan.len() as u32);
        let response = fetch(
            job,
            &broker,
            &entry.url,
            u64::from(entry.maximum_bytes),
            &origins,
        )
        .await?;
        responses.push(GetResponse {
            status: response.metadata.status,
            body: response.body,
        });
    }
    let worker_cancel = job.cancel.clone();
    let output = tauri::async_runtime::spawn_blocking(move || {
        session.normalize(&request, &responses, &origins, worker_cancel)
    })
    .await
    .map_err(|_| {
        tracing::warn!(
            event = "blueprint_provider_worker_failed",
            phase = "normalize"
        );
        "蓝图插件运行失败"
    })?
    .map_err(|error| {
        let code = diagnostic_code(&error.code);
        tracing::warn!(
            event = "blueprint_provider_failed",
            phase = "normalize",
            code
        );
        format!("蓝图服务返回了不支持的数据（{code}）")
    })?;
    verify(&plugin.manifest, job)?;
    job.progress(window, phase, plan.len() as u32, plan.len() as u32);
    Ok((output, plugin.manifest, broker, version_filter_applied))
}
#[derive(Serialize)]
pub struct SearchOutput {
    #[serde(flatten)]
    output: ProviderOutput,
    #[serde(skip_serializing_if = "Option::is_none")]
    search_scope: Option<&'static str>,
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn search_online_blueprints(
    window: WebviewWindow,
    app: AppHandle,
    game_id: String,
    provider_id: String,
    query: String,
    format: Option<String>,
    mod_namespace: Option<String>,
    cursor: Option<String>,
    request_id: String,
) -> Result<SearchOutput, String> {
    let release = crate::games::registered_game_version(&app, &game_id)?;
    let job = Job::begin(window.label(), request_id, &provider_id)?;
    let request = ProviderRequest {
        operation: ProviderOperation::Search,
        search: Some(SearchQuery {
            java_release: release,
            text: query,
            format,
            mod_namespace,
            cursor,
            limit: 20,
        }),
        key: None,
    };
    execute(&window, &job, request, "search")
        .await
        .map(|(output, _, _, version_filter_applied)| SearchOutput {
            output,
            search_scope: (!version_filter_applied).then_some("all_versions"),
        })
}
#[tauri::command]
pub async fn online_blueprint_details(
    window: WebviewWindow,
    provider_id: String,
    id: String,
    version: u32,
    request_id: String,
) -> Result<ProviderOutput, String> {
    let job = Job::begin(window.label(), request_id, &provider_id)?;
    let request = ProviderRequest {
        operation: ProviderOperation::Detail,
        search: None,
        key: Some(ResourceKey { id, version }),
    };
    execute(&window, &job, request, "details")
        .await
        .map(|(output, _, _, _)| output)
}
fn authorize_file(
    file: &provider::FileProposal,
    id: &str,
    version: u32,
    manifest: &PluginManifest,
) -> Result<super::BlueprintFormat, String> {
    if file.id != id
        || file.version != version
        || file.declared_bytes == 0
        || file.declared_bytes > MAX_FILE_BYTES as u64
    {
        return Err("蓝图文件超过当前预览限制，或来源不匹配".into());
    }
    let format = match file.format.as_str() {
        "schem" => super::BlueprintFormat::Schem,
        "litematic" => super::BlueprintFormat::Litematic,
        _ => return Err("不支持此蓝图格式".into()),
    };
    provider::validate_url(&file.url, &manifest.provider_origins)
        .map_err(|_| "蓝图原始文件地址未获授权")?;
    Ok(format)
}
fn file_source(file: &provider::FileProposal, manifest: &PluginManifest) -> super::BlueprintSource {
    super::BlueprintSource {
        provider: manifest.id.clone(),
        id: file.id.clone(),
        version: file.version,
        author: file.author.clone(),
        attribution: file.attribution.clone(),
        license: file.license.clone(),
    }
}
fn persist_download(
    directory: &Path,
    name: String,
    format: super::BlueprintFormat,
    source: super::BlueprintSource,
    bytes: &[u8],
) -> Result<super::Blueprint, String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    for entry in std::fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
            && let Ok(contents) = std::fs::read(&path)
            && let Ok(existing) = serde_json::from_slice::<super::Blueprint>(&contents)
            && existing.source.as_ref().is_some_and(|saved| {
                saved.provider == source.provider
                    && saved.id == source.id
                    && saved.version == source.version
            })
            && directory
                .join(format!("{}.{}", existing.id, existing.format.extension()))
                .is_file()
        {
            return Ok(existing);
        }
    }
    let blueprint = super::Blueprint {
        id: super::next_id(),
        name,
        plugin_id: String::new(),
        format,
        source: Some(source),
        favorite: false,
        has_thumbnail: false,
    };
    let content_path = directory.join(format!("{}.{}", blueprint.id, format.extension()));
    let metadata_path = directory.join(format!("{}.json", blueprint.id));
    std::fs::write(&content_path, bytes).map_err(|error| error.to_string())?;
    if let Err(error) = std::fs::write(
        &metadata_path,
        serde_json::to_vec(&blueprint).map_err(|error| error.to_string())?,
    ) {
        let _ = std::fs::remove_file(content_path);
        return Err(error.to_string());
    }
    Ok(blueprint)
}
#[tauri::command]
pub async fn download_online_blueprint(
    window: WebviewWindow,
    provider_id: String,
    id: String,
    version: u32,
    request_id: String,
) -> Result<super::Blueprint, String> {
    let job = Job::begin(window.label(), request_id, &provider_id)?;
    let request = ProviderRequest {
        operation: ProviderOperation::Download,
        search: None,
        key: Some(ResourceKey {
            id: id.clone(),
            version,
        }),
    };
    let (output, manifest, broker, _) = execute(&window, &job, request, "details").await?;
    let file = output.download.ok_or("蓝图插件未提供原始文件")?;
    let format = authorize_file(&file, &id, version, &manifest)?;
    job.progress(&window, "download", 0, file.declared_bytes as u32);
    let response = fetch(
        &job,
        &broker,
        &file.url,
        file.declared_bytes,
        &manifest.provider_origins,
    )
    .await?;
    if response.metadata.status != 200
        || response.body.len() as u64 != file.declared_bytes
        || file.sha256.as_ref().is_some_and(|hash| {
            hash.to_ascii_lowercase() != cobble_plugin_host::digest(&response.body)
        })
    {
        return Err("蓝图下载不完整或校验失败".into());
    }
    verify(&manifest, &job)?;
    job.progress(
        &window,
        "download",
        file.declared_bytes as u32,
        file.declared_bytes as u32,
    );
    let directory = super::root("blueprints")?;
    let source = file_source(&file, &manifest);
    persist_download(&directory, file.title, format, source, &response.body)
}
#[tauri::command]
pub async fn open_online_blueprint(
    window: WebviewWindow,
    provider_id: String,
    id: String,
    version: u32,
    request_id: String,
    viewer_id: Option<String>,
) -> Result<super::OpenViewer, String> {
    let job = Job::begin(window.label(), request_id, &provider_id)?;
    let request = ProviderRequest {
        operation: ProviderOperation::Download,
        search: None,
        key: Some(ResourceKey {
            id: id.clone(),
            version,
        }),
    };
    let (output, manifest, broker, _) = execute(&window, &job, request, "details").await?;
    let file = output.download.ok_or("蓝图插件未提供原始文件")?;
    let format = authorize_file(&file, &id, version, &manifest)?;
    let selected_viewer = super::blueprint_viewers::select_in(
        &super::registry()?,
        Some(&provider_id),
        format,
        viewer_id.as_deref(),
    )?;
    tracing::info!(event="blueprint_handoff",provider_id=%provider_id,viewer_id=%selected_viewer.manifest.id,phase="selected",outcome="ready");
    job.progress(&window, "download", 0, file.declared_bytes as u32);
    let response = fetch(
        &job,
        &broker,
        &file.url,
        file.declared_bytes,
        &manifest.provider_origins,
    )
    .await?;
    if response.metadata.status != 200
        || response.body.len() as u64 != file.declared_bytes
        || file.sha256.as_ref().is_some_and(|hash| {
            hash.to_ascii_lowercase() != cobble_plugin_host::digest(&response.body)
        })
    {
        return Err("蓝图下载不完整或校验失败".into());
    }
    verify(&manifest, &job)?;
    job.progress(
        &window,
        "download",
        file.declared_bytes as u32,
        file.declared_bytes as u32,
    );
    let directory = super::root("blueprints")?;
    let source = file_source(&file, &manifest);
    persist_download(
        &directory,
        file.title.clone(),
        format,
        source.clone(),
        &response.body,
    )?;
    job.progress(&window, "viewer", 0, 1);
    let cancel = job.cancel.clone();
    let opened = tauri::async_runtime::spawn_blocking(move || {
        let registry = super::registry()?;
        let current = super::blueprint_viewers::select_in(
            &registry,
            Some(&provider_id),
            format,
            Some(&selected_viewer.manifest.id),
        )?;
        if current.manifest != selected_viewer.manifest {
            return Err("Selected Viewer changed during handoff".into());
        }
        super::open_in_registry(
            &registry,
            &host()?,
            super::ViewerInput {
                bytes: response.body,
                name: file.title,
                plugin_id: Some(selected_viewer.manifest.id.clone()),
                format,
                source: Some(source),
                cancel,
            },
            Some(&selected_viewer.manifest),
        )
    })
    .await
    .map_err(|_| "蓝图预览运行失败")??;
    if verify(&manifest, &job).is_err() {
        let _ = super::close_viewer(opened.resource_id.clone());
        return Err("蓝图请求已取消或插件已变化".into());
    }
    job.progress(&window, "viewer", 1, 1);
    Ok(opened)
}

#[cfg(test)]
mod tests {
    use super::*;
    const ORIGIN: &str = "https://www.mcschematic.top";
    const PROVIDER: &str = "mcs-provider";

    #[test]
    fn downloaded_blueprints_persist_and_reuse_the_same_provider_version() {
        let temporary = tempfile::tempdir().unwrap();
        let source = super::super::BlueprintSource {
            provider: PROVIDER.into(),
            id: "fixture-blueprint".into(),
            version: 3,
            author: "Fixture author".into(),
            attribution: "original".into(),
            license: Some("MIT".into()),
        };
        let first = persist_download(
            temporary.path(),
            "Fixture build".into(),
            super::super::BlueprintFormat::Schem,
            source.clone(),
            b"synthetic-schematic",
        )
        .unwrap();
        let second = persist_download(
            temporary.path(),
            "Updated display title".into(),
            super::super::BlueprintFormat::Schem,
            source,
            b"replacement-bytes",
        )
        .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(second.name, "Fixture build");
        assert_eq!(
            std::fs::read(temporary.path().join(format!("{}.schem", first.id))).unwrap(),
            b"synthetic-schematic"
        );
        assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 2);
    }
    #[test]
    fn retries_only_explicit_unsupported_search_once_and_retains_other_filters() {
        let mut request = ProviderRequest {
            operation: ProviderOperation::Search,
            search: Some(SearchQuery {
                java_release: "26.3".into(),
                text: "hut".into(),
                format: Some("schem".into()),
                mod_namespace: None,
                cursor: None,
                limit: 20,
            }),
            key: None,
        };
        let unrelated = cobble_plugin_api::PluginError::new("PLUGIN_TIMEOUT", "not logged");
        assert!(!unfiltered_retry(&mut request, &unrelated));
        let unsupported =
            cobble_plugin_api::PluginError::new("PROVIDER_UNSUPPORTED_GAME_VERSION", "not logged");
        assert!(unfiltered_retry(&mut request, &unsupported));
        assert!(!unfiltered_retry(&mut request, &unsupported));
        let search = request.search.as_ref().unwrap();
        assert!(search.java_release.is_empty());
        assert_eq!(search.text, "hut");
        assert_eq!(search.format.as_deref(), Some("schem"));
        request.operation = ProviderOperation::Detail;
        assert!(!unfiltered_retry(&mut request, &unsupported));
        assert_eq!(diagnostic_code("PLUGIN_TIMEOUT"), "PLUGIN_TIMEOUT");
        assert_eq!(diagnostic_code("private\nquery"), "PLUGIN_INVALID_ERROR");
    }
    #[test]
    fn replacement_cancels_only_the_owned_window_and_old_drop_cannot_remove_new_job() {
        let first = Job::begin("provider-test-a", "request-1".into(), "provider-a").unwrap();
        let other = Job::begin("provider-test-b", "request-1".into(), "provider-b").unwrap();
        let second = Job::begin("provider-test-a", "request-1".into(), "provider-a").unwrap();
        assert!(first.check().is_err());
        assert!(other.check().is_ok());
        drop(first);
        assert!(second.check().is_ok());
        revoke_plugin("provider-a");
        assert!(second.check().is_err());
        assert!(other.check().is_ok());
    }
    #[test]
    fn rejects_unbounded_or_path_like_request_identifiers() {
        assert!(Job::begin("provider-test-invalid", "../private".into(), PROVIDER).is_err());
        assert!(Job::begin("provider-test-invalid", "x".repeat(81), PROVIDER).is_err());
    }

    #[test]
    #[ignore = "Requires actual community Provider and Viewer packages plus plugin host; original local fixtures only"]
    fn actual_independent_provider_to_selected_viewer_preserves_provenance_and_lifecycle() {
        use cobble_plugin_host::PluginRegistry;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let executable = root.join("target/release/cobble-plugin-host.exe");
        let temporary = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(temporary.path().join("plugins"));
        let provider = registry
            .install_directory(&root.join("target/community-provider-package"))
            .unwrap();
        let viewer_source = temporary.path().join("viewer-package");
        std::fs::create_dir(&viewer_source).unwrap();
        let (mut viewer_manifest, viewer_component) =
            cobble_plugin_host::read_package(&root.join("target/schematic-viewer-package"))
                .unwrap();
        viewer_manifest.id = "independent-preview".into();
        viewer_manifest.name = "Independent preview fixture".into();
        viewer_manifest.interfaces.provides = vec![cobble_plugin_api::ProvidedInterface {
            id: cobble_plugin_api::VIEWER_INTERFACE.into(),
            version: "1.0.0".into(),
        }];
        std::fs::write(
            viewer_source.join("plugin.json"),
            serde_json::to_vec(&viewer_manifest).unwrap(),
        )
        .unwrap();
        std::fs::write(viewer_source.join("component.wasm"), viewer_component).unwrap();
        cobble_plugin_host::copy_package_assets(
            &root.join("target/schematic-viewer-package"),
            &viewer_source,
            &viewer_manifest,
        )
        .unwrap();
        registry.install_directory(&viewer_source).unwrap();
        let (_, component) =
            cobble_plugin_host::read_package(&root.join("target/community-provider-package"))
                .unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut session =
            provider::ProviderSession::open(&executable, &component, cancel.clone()).unwrap();
        let request = ProviderRequest {
            operation: ProviderOperation::Download,
            search: None,
            key: Some(ResourceKey {
                id: "community-hut-a".into(),
                version: 7,
            }),
        };
        let plan = session
            .plan(
                &request,
                &provider.manifest.provider_origins,
                cancel.clone(),
            )
            .unwrap();
        assert_eq!(plan[0].url, "https://community.example/catalog/hut");
        let output = session
            .normalize(
                &request,
                &[GetResponse {
                    status: 200,
                    body: b"community-original-fixture".to_vec(),
                }],
                &provider.manifest.provider_origins,
                cancel.clone(),
            )
            .unwrap();
        let file = output.download.unwrap();
        session.close();
        assert!(session.has_exited().unwrap());
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../crates/plugin-host/tests/fixtures/community-provider/src/hut.schem"
        ))
        .to_vec();
        assert_eq!(bytes.len() as u64, file.declared_bytes);
        let format = authorize_file(&file, "community-hut-a", 7, &provider.manifest).unwrap();
        let source = file_source(&file, &provider.manifest);
        let selected = super::super::blueprint_viewers::select_in(
            &registry,
            Some(&provider.manifest.id),
            format,
            None,
        )
        .unwrap();
        assert_eq!(selected.manifest.id, "independent-preview");
        let opened = super::super::open_in_registry(
            &registry,
            &executable,
            super::super::ViewerInput {
                bytes: bytes.clone(),
                name: file.title,
                plugin_id: Some(selected.manifest.id.clone()),
                format,
                source: Some(source),
                cancel: cancel.clone(),
            },
            Some(&selected.manifest),
        )
        .unwrap();
        assert_eq!(opened.scene.palette, ["minecraft:stone"]);
        let resource = super::super::resource_in(&registry, &opened.resource_id).unwrap();
        assert_eq!(resource.bytes, bytes);
        let provenance = resource.source.as_ref().unwrap();
        assert_eq!(provenance.provider, "community-provider");
        assert_eq!(provenance.id, "community-hut-a");
        assert_eq!(provenance.author, "Fixture author");
        assert_eq!(provenance.license.as_deref(), Some("MIT"));
        assert_eq!(
            resource
                .session
                .lock()
                .unwrap()
                .read_chunk(0, cancel.clone())
                .unwrap()
                .blocks
                .len(),
            1
        );
        registry.set_enabled("community-provider", false).unwrap();
        revoke_plugin("community-provider");
        assert!(super::super::resource_in(&registry, &opened.resource_id).is_ok());
        registry.set_enabled("independent-preview", false).unwrap();
        assert!(super::super::resource_in(&registry, &opened.resource_id).is_err());
        assert!(cancel.load(Ordering::Acquire));
        {
            let mut viewer = resource.session.lock().unwrap();
            viewer.close();
            assert!(viewer.has_exited().unwrap());
        }
        drop(resource);
        temporary.close().unwrap();
    }

    #[test]
    #[ignore = "Actual independent community Wasm fixture and release host must be built; no public network"]
    fn independent_community_provider_uses_its_own_grant_and_disable_isolation() {
        use cobble_plugin_host::PluginRegistry;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let executable = root.join("target/release/cobble-plugin-host.exe");
        let temporary = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(temporary.path().join("plugins"));
        registry
            .install_directory(&root.join("target/mcs-provider-package"))
            .unwrap();
        registry
            .install_directory(&root.join("target/community-provider-package"))
            .unwrap();
        let community = selected_in(&registry, "community-provider").unwrap();
        let mcs = selected_in(&registry, PROVIDER).unwrap();
        assert_eq!(
            registry
                .list()
                .unwrap()
                .iter()
                .filter(|plugin| is_provider(plugin))
                .count(),
            2
        );
        assert_eq!(
            community.manifest.provider_origins,
            ["https://community.example"]
        );
        assert_ne!(
            community.manifest.provider_origins,
            mcs.manifest.provider_origins
        );
        let (_, component) =
            cobble_plugin_host::read_package(&temporary.path().join("plugins/community-provider"))
                .unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut session =
            provider::ProviderSession::open(&executable, &component, cancel.clone()).unwrap();
        let origins = community.manifest.provider_origins.clone();
        let search = ProviderRequest {
            operation: ProviderOperation::Search,
            search: Some(SearchQuery {
                java_release: "1.21.1".into(),
                text: "hut".into(),
                format: None,
                mod_namespace: None,
                cursor: None,
                limit: 20,
            }),
            key: None,
        };
        let mut invoke_fixture = |request: &ProviderRequest| {
            let plan = session.plan(request, &origins, cancel.clone()).unwrap();
            assert_eq!(
                plan.len(),
                1,
                "The generic runtime must not require MCS's two-GET detail shape"
            );
            assert_eq!(plan[0].url, "https://community.example/catalog/hut");
            // A clearly synthetic HTTP response exercises actual guest normalization,
            // without sending network traffic to the reserved .example domain.
            session
                .normalize(
                    request,
                    &[GetResponse {
                        status: 200,
                        body: b"community-original-fixture".to_vec(),
                    }],
                    &origins,
                    cancel.clone(),
                )
                .unwrap()
        };
        let result = invoke_fixture(&search);
        let item = &result.items[0];
        assert_eq!(item.id, "community-hut-a");
        assert_eq!(item.version, 7);
        assert_eq!(item.author, "Fixture author");
        assert_eq!(item.license.as_deref(), Some("MIT"));
        let key = Some(ResourceKey {
            id: item.id.clone(),
            version: item.version,
        });
        let detail = invoke_fixture(&ProviderRequest {
            operation: ProviderOperation::Detail,
            search: None,
            key: key.clone(),
        });
        let detail = detail.detail.unwrap();
        assert_eq!(detail.id, item.id);
        assert_eq!(detail.license.as_deref(), Some("MIT"));
        let mut file = invoke_fixture(&ProviderRequest {
            operation: ProviderOperation::Download,
            search: None,
            key,
        })
        .download
        .unwrap();
        assert_eq!(
            authorize_file(&file, &item.id, item.version, &community.manifest).unwrap(),
            super::super::BlueprintFormat::Schem
        );
        assert_eq!(file.url, "https://community.example/files/hut.schem");
        assert_eq!(file.attribution, "Original synthetic fixture");
        let source = file_source(&file, &community.manifest);
        assert_eq!(source.provider, "community-provider");
        assert_eq!(source.id, "community-hut-a");
        assert_eq!(source.author, "Fixture author");
        assert_eq!(source.license.as_deref(), Some("MIT"));
        file.url = "https://www.mcschematic.top/unapproved.schem".into();
        assert!(
            authorize_file(&file, &item.id, item.version, &community.manifest).is_err(),
            "Another installed provider's origin must not extend the selected grant"
        );
        let malicious = ProviderRequest {
            search: Some(SearchQuery {
                text: "cross-origin".into(),
                ..search.search.unwrap()
            }),
            ..search
        };
        assert!(session.plan(&malicious, &origins, cancel.clone()).is_err());
        session.close();
        assert!(session.has_exited().unwrap());
        let community_job = Job::begin(
            "community-proof",
            "community-proof".into(),
            "community-provider",
        )
        .unwrap();
        let mcs_job = Job::begin("mcs-proof", "mcs-proof".into(), PROVIDER).unwrap();
        registry.set_enabled("community-provider", false).unwrap();
        revoke_plugin("community-provider");
        assert!(selected_in(&registry, "community-provider").is_err());
        assert!(community_job.check().is_err());
        assert!(selected_in(&registry, PROVIDER).is_ok());
        assert!(mcs_job.check().is_ok());
        drop((community_job, mcs_job));
        temporary.close().unwrap();
    }

    #[test]
    #[ignore = "Explicit public HTTPS + real installed Provider and Viewer component smoke; builds required"]
    fn public_provider_original_download_and_real_viewer() {
        use cobble_plugin_host::PluginRegistry;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let executable = root.join("target/release/cobble-plugin-host.exe");
        assert!(
            executable.is_file(),
            "Build the release plugin host before this explicit smoke"
        );
        let temporary = tempfile::tempdir().unwrap();
        let registry = PluginRegistry::new(temporary.path().join("plugins"));
        let installed = registry
            .install_directory(&root.join("target/mcs-provider-package"))
            .unwrap();
        registry
            .install_directory(&root.join("target/schematic-viewer-package"))
            .unwrap();
        assert!(installed.enabled && installed.problem.is_none());
        assert_eq!(installed.manifest.id, PROVIDER);
        assert_eq!(installed.manifest.provider_origins, [ORIGIN]);
        assert_eq!(
            selected_in(&registry, PROVIDER).unwrap().manifest,
            installed.manifest
        );
        let (_, component) =
            cobble_plugin_host::read_package(&temporary.path().join("plugins/mcs-provider"))
                .unwrap();
        let origins = vec![ORIGIN.to_owned()];
        let job = Job::begin(
            "explicit-public-provider-smoke",
            "live-smoke".into(),
            PROVIDER,
        )
        .unwrap();
        let broker = Broker::new(BrokerConfig {
            max_redirects: 0,
            ..BrokerConfig::default()
        })
        .unwrap();
        let invoke = |request: &ProviderRequest| {
            // Exactly the production Wasm planning/normalization and restricted GET primitives.
            let mut session =
                provider::ProviderSession::open(&executable, &component, job.cancel.clone())
                    .unwrap();
            let plan = session.plan(request, &origins, job.cancel.clone()).unwrap();
            let mut responses = Vec::new();
            for entry in plan {
                let response = tauri::async_runtime::block_on(fetch(
                    &job,
                    &broker,
                    &entry.url,
                    u64::from(entry.maximum_bytes),
                    &origins,
                ))
                .unwrap();
                assert_eq!(response.metadata.status, 200);
                responses.push(GetResponse {
                    status: response.metadata.status,
                    body: response.body,
                });
            }
            let output = session
                .normalize(request, &responses, &origins, job.cancel.clone())
                .unwrap();
            session.close();
            assert!(
                session.has_exited().unwrap(),
                "The exact owned Provider worker must exit"
            );
            output
        };
        let search = ProviderRequest {
            operation: ProviderOperation::Search,
            search: Some(SearchQuery {
                java_release: "1.21.1".into(),
                text: "盆栽".into(),
                format: Some("schem".into()),
                mod_namespace: None,
                cursor: None,
                limit: 2,
            }),
            key: None,
        };
        let page = invoke(&search);
        let item = page
            .items
            .first()
            .expect("Public smoke query needs one current result");
        let request = ProviderRequest {
            operation: ProviderOperation::Download,
            search: None,
            key: Some(ResourceKey {
                id: item.id.clone(),
                version: item.version,
            }),
        };
        let file = invoke(&request)
            .download
            .expect("Actual guest must propose original source bytes");
        assert_eq!(
            file.url,
            format!(
                "{ORIGIN}/api/v1/blueprints/{}/versions/{}/download",
                item.id, item.version
            )
        );
        assert_eq!(file.format, "schem");
        assert!(file.declared_bytes > 0 && file.declared_bytes <= MAX_FILE_BYTES as u64);
        let response = tauri::async_runtime::block_on(fetch(
            &job,
            &broker,
            &file.url,
            file.declared_bytes,
            &origins,
        ))
        .unwrap();
        assert_eq!(response.metadata.status, 200);
        assert_eq!(response.body.len() as u64, file.declared_bytes);
        let mut viewer = registry
            .open_viewer(
                &executable,
                "schematic-viewer",
                &response.body,
                job.cancel.clone(),
            )
            .unwrap();
        let chunks = viewer.summary().chunk_count;
        assert!(chunks > 0);
        assert!(
            !viewer
                .read_chunk(0, job.cancel.clone())
                .unwrap()
                .blocks
                .is_empty()
        );
        eprintln!(
            "Public Provider smoke: real Wasm plan+normalize; original format={}, bytes={}, Viewer chunks={}; anonymous, no artifact saved",
            file.format, file.declared_bytes, chunks
        );
        viewer.close();
        assert!(
            viewer.has_exited().unwrap(),
            "The exact owned Viewer worker must exit"
        );
        drop(viewer);
        registry.set_enabled(PROVIDER, false).unwrap();
        assert!(
            selected_in(&registry, PROVIDER).is_err(),
            "The production selection gate must reject a disabled provider"
        );
        assert!(
            !registry
                .list()
                .unwrap()
                .iter()
                .find(|plugin| plugin.manifest.id == PROVIDER)
                .unwrap()
                .enabled
        );
        drop(job);
        temporary.close().unwrap();
    }
}
