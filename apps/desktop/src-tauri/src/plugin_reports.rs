// SPDX-License-Identifier: MIT
//! Native, explicitly selected immutable report input. No paths cross the IPC boundary.
#[path = "plugin_report_activity.rs"]
mod activity;
#[path = "config_diagnosis.rs"]
pub mod config_diagnosis;
#[path = "folder_report.rs"]
pub mod folder_report;
#[path = "plugin_report_inbox.rs"]
pub mod inbox;
#[path = "managed_config_report.rs"]
pub mod managed_config_report;
#[path = "managed_diagnostic_report.rs"]
pub mod managed_diagnostic_report;
#[path = "managed_inspection_report.rs"]
pub mod managed_inspection_report;
#[path = "managed_selected_report.rs"]
pub mod managed_selected_report;
use cobble_plugin_api::{InstalledPlugin, MAX_FILE_BYTES, PluginManifest, Report};
use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{AppHandle, WebviewWindow};

struct Analysis {
    plugin_id: String,
    cancel: Arc<AtomicBool>,
}
#[derive(Default)]
struct AnalysisState {
    active: Option<Analysis>,
}
impl AnalysisState {
    fn begin(&mut self, plugin_id: String) -> Result<Arc<AtomicBool>, String> {
        if self.active.is_some() {
            return Err("已有分析正在进行".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.active = Some(Analysis {
            plugin_id,
            cancel: cancel.clone(),
        });
        Ok(cancel)
    }
    fn cancel(&self, plugin_id: Option<&str>) {
        if let Some(active) = &self.active
            && plugin_id.is_none_or(|id| id == active.plugin_id)
        {
            active.cancel.store(true, Ordering::SeqCst);
        }
    }
    fn finish<T>(
        &mut self,
        cancel: &Arc<AtomicBool>,
        result: Result<Option<T>, String>,
    ) -> Result<Option<T>, String> {
        if !self
            .active
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(&active.cancel, cancel))
        {
            return Ok(None);
        }
        self.active = None;
        if cancel.load(Ordering::SeqCst) {
            Ok(None)
        } else {
            result
        }
    }
}
static ANALYSIS: OnceLock<Mutex<AnalysisState>> = OnceLock::new();
fn state() -> &'static Mutex<AnalysisState> {
    ANALYSIS.get_or_init(Default::default)
}

pub(super) fn cancel_for_plugin(plugin_id: &str) {
    crate::games::screenshot_sheet::revoke_plugin(plugin_id);
    if let Ok(state) = state().lock() {
        state.cancel(Some(plugin_id));
    }
}

#[tauri::command]
pub fn cancel_plugin_report() -> Result<(), String> {
    state().lock().map_err(|_| "分析状态不可用")?.cancel(None);
    Ok(())
}

pub(super) fn selected_plugin(plugin_id: &str) -> Result<InstalledPlugin, String> {
    super::valid_id(plugin_id)?;
    let plugin = super::registry()?
        .resolve_enabled(plugin_id)
        .map_err(|error| error.to_string())?;
    if plugin.manifest.capabilities != ["report"]
        || (!plugin.manifest.is_inspection_report()
            && !plugin.manifest.file_extensions.iter().any(|extension| {
                report_media_type(extension).is_some()
                    || extension == "mrpack"
                    || (extension == "zip" && pack_metadata_protocol(&plugin.manifest))
            }))
    {
        return Err("请先启用支持此文件类型的分析插件".into());
    }
    Ok(plugin)
}

pub(super) fn package(plugin: &InstalledPlugin) -> Result<Vec<u8>, String> {
    let directory = super::root("plugins")?.join(&plugin.manifest.id);
    let metadata = std::fs::symlink_metadata(&directory).map_err(|_| "无法读取插件包")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("插件目录无效".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("插件目录无效".into());
        }
    }
    let (manifest, component) = super::registry()?
        .read_installed_package(&plugin.manifest.id)
        .map_err(|_| "插件包无效")?;
    if manifest != plugin.manifest {
        return Err("插件已变化，请重新分析".into());
    }
    Ok(component)
}

pub(super) fn verify_current(manifest: &PluginManifest) -> Result<(), String> {
    if selected_plugin(&manifest.id)?.manifest != *manifest {
        return Err("插件已变化，请重新分析".into());
    }
    Ok(())
}

fn report_media_type(extension: &str) -> Option<&'static str> {
    match extension {
        "log" | "txt" => Some("text/plain"),
        "json" | "mcmeta" => Some("application/json"),
        "toml" => Some("application/toml"),
        "properties" => Some("text/x-java-properties"),
        "yaml" | "yml" => Some("application/yaml"),
        _ => None,
    }
}

fn pack_metadata_protocol(manifest: &PluginManifest) -> bool {
    manifest
        .report_protocols
        .iter()
        .any(|protocol| protocol == cobble_plugin_api::PACK_METADATA_PROTOCOL)
}
fn selected_snapshot(
    path: &Path,
    manifest: &PluginManifest,
) -> Result<(Vec<u8>, &'static str), String> {
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("zip"))
    {
        if !pack_metadata_protocol(manifest)
            || !manifest.file_extensions.iter().any(|value| value == "zip")
        {
            return Err("插件未获准读取资源包元数据".into());
        }
        let file = std::fs::File::open(path).map_err(|_| "无法读取所选资源包")?;
        return cobble_content::read_pack_metadata_zip(file)
            .map(|bytes| (bytes, "application/json"))
            .map_err(|_| "无法读取资源包根目录 pack.mcmeta，文件可能缺失、损坏或超出限制".into());
    }
    snapshot(path, &manifest.file_extensions)
}
fn snapshot(path: &Path, extensions: &[String]) -> Result<(Vec<u8>, &'static str), String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !extensions.contains(&extension) {
        return Err("插件不支持此文件类型".into());
    }
    let media_type = report_media_type(&extension).ok_or("不支持此文件类型")?;
    let config_text = matches!(extension.as_str(), "toml" | "properties" | "yaml" | "yml");
    let maximum = if media_type == "text/plain" || config_text {
        cobble_diagnostics::MAX_INPUT_BYTES
    } else {
        MAX_FILE_BYTES
    };
    if !std::fs::metadata(path)
        .map_err(|_| "无法读取所选文件")?
        .is_file()
    {
        return Err("请选择普通文件".into());
    }
    let file = std::fs::File::open(path).map_err(|_| "无法读取所选文件")?;
    if !file.metadata().map_err(|_| "无法读取所选文件")?.is_file() {
        return Err("请选择普通文件".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取所选文件")?;
    if bytes.len() > maximum {
        return Err("文件超过分析大小限制".into());
    }
    if config_text {
        std::str::from_utf8(&bytes).map_err(|_| "CONFIG_NOT_UTF8")?;
    }
    if media_type == "text/plain" {
        let text = std::str::from_utf8(&bytes).map_err(|_| "日志必须为 UTF-8 文本")?;
        bytes = cobble_diagnostics::redact_log(text).into_bytes();
        if bytes.len() > maximum {
            return Err("脱敏后的日志超过分析大小限制".into());
        }
    }
    Ok((bytes, media_type))
}

#[tauri::command]
pub async fn analyze_plugin_report(
    app: AppHandle,
    window: WebviewWindow,
    plugin_id: String,
    compare: Option<bool>,
    locale: Option<String>,
    instance_id: Option<String>,
) -> Result<Option<Report>, String> {
    if let Some(id) = &instance_id {
        crate::games::ensure_registered_game(&app, id)?;
    }
    let locale = locale.unwrap_or_else(|| "en".into());
    super::extension_contributions::validate_locale(&locale)?;
    super::valid_id(&plugin_id)?;
    let started = std::time::Instant::now();
    let cancel = state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .begin(plugin_id.clone())?;
    let result = async {
        let id = plugin_id.clone();
        let (plugin, component) = tauri::async_runtime::spawn_blocking(move || {
            let plugin = selected_plugin(&id)?;
            let component = package(&plugin)?;
            Ok::<_, String>((plugin, component))
        })
        .await
        .map_err(|_| "无法准备分析插件")??;
        if cancel.load(Ordering::SeqCst) {
            return Ok(None);
        }
        if plugin
            .manifest
            .file_extensions
            .iter()
            .any(|extension| extension == "mrpack")
        {
            let Some((bytes, media_type)) =
                super::pack_inspection::prepare_pack_report(&window, compare.unwrap_or(false))
                    .await?
            else {
                return Ok(None);
            };
            if let Some(id) = &instance_id {
                crate::games::ensure_registered_game(&app, id)?;
            }
            return execute_report(
                plugin,
                component,
                bytes,
                media_type,
                cancel.clone(),
                locale.clone(),
                instance_id.clone(),
            )
            .await;
        }
        let extensions: Vec<_> = plugin
            .manifest
            .file_extensions
            .iter()
            .filter(|extension| {
                report_media_type(extension).is_some()
                    || (extension.as_str() == "zip" && pack_metadata_protocol(&plugin.manifest))
            })
            .cloned()
            .collect();
        let Some(file) = rfd::AsyncFileDialog::new()
            .set_parent(&window)
            .set_title("选择分析文件")
            .add_filter("分析文件", &extensions)
            .pick_file()
            .await
        else {
            return Ok(None);
        };
        if cancel.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let path = file.path().to_owned();
        let manifest = plugin.manifest.clone();
        let (bytes, media_type) =
            tauri::async_runtime::spawn_blocking(move || selected_snapshot(&path, &manifest))
                .await
                .map_err(|_| "无法读取分析文件")??;
        if let Some(id) = &instance_id {
            crate::games::ensure_registered_game(&app, id)?;
        }
        execute_report(
            plugin,
            component,
            bytes,
            media_type,
            cancel.clone(),
            locale.clone(),
            instance_id.clone(),
        )
        .await
    }
    .await;
    let result = state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .finish(&cancel, result);
    let outcome = match &result {
        Ok(Some(_)) => "completed",
        Ok(None) => "cancelled",
        Err(_) => "failed",
    };
    tracing::info!(event = "plugin_report_finished", %plugin_id, outcome, elapsed_ms = started.elapsed().as_millis() as u64);
    result
}

async fn execute_report(
    plugin: InstalledPlugin,
    component: Vec<u8>,
    bytes: Vec<u8>,
    media_type: &'static str,
    cancel: Arc<AtomicBool>,
    locale: String,
    instance_id: Option<String>,
) -> Result<Option<Report>, String> {
    let mut activity = activity::Activity::new(
        &plugin.manifest.id,
        &plugin.manifest.api_version,
        cancel.clone(),
    );
    tauri::async_runtime::spawn_blocking(move || {
        verify_current(&plugin.manifest).inspect_err(|_| activity.failed("PLUGIN_CHANGED"))?;
        if cancel.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let host = std::env::current_exe()
            .map_err(|_| {
                activity.failed("PLUGIN_HOST_UNAVAILABLE");
                "无法找到分析宿主"
            })?
            .with_file_name(if cfg!(windows) {
                "cobble-plugin-host.exe"
            } else {
                "cobble-plugin-host"
            });
        let result = if plugin.manifest.api_version == cobble_plugin_api::REPORT_CONTEXT_API_VERSION
        {
            let context = super::extension_settings::report_context(
                &plugin.manifest,
                instance_id.as_deref(),
                &locale,
            )
            .inspect_err(|_| activity.failed("PLUGIN_INVALID_CONTEXT"))?;
            cobble_plugin_host::run_report_with_context(
                &host, &component, &bytes, media_type, cancel, &context,
            )
        } else {
            cobble_plugin_host::run_report(&host, &component, &bytes, media_type, cancel)
        };
        let report = result.map_err(|error| {
            activity.failed(&error.code);
            "插件分析失败，请检查文件或重试"
        })?;
        verify_current(&plugin.manifest).inspect_err(|_| activity.failed("PLUGIN_CHANGED"))?;
        activity.completed();
        Ok(Some(report))
    })
    .await
    .map_err(|_| "分析宿主意外退出")?
}

/// Snapshot the newest bounded portion; evidence remains relative to that immutable snapshot.
fn game_log_snapshot(directory: &Path) -> Result<(Vec<u8>, &'static str, bool), String> {
    game_log_snapshot_for_launch(directory, false)
}
fn game_log_snapshot_for_launch(
    directory: &Path,
    current_only: bool,
) -> Result<(Vec<u8>, &'static str, bool), String> {
    let root = directory.canonicalize().map_err(|_| "无法读取游戏目录")?;
    let mut candidates = ["logs/latest.log", ".cobble-logs/launcher.log"]
        .into_iter()
        .filter(|name| !current_only || *name == ".cobble-logs/launcher.log")
        .filter_map(|name| {
            let path = directory.join(name);
            let metadata = std::fs::metadata(&path).ok()?;
            (metadata.is_file() && metadata.len() > 0)
                .then(|| (metadata.modified().ok(), path, name))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
    let (_, path, name) = candidates.first().ok_or("还没有游戏运行日志")?;
    if !path
        .canonicalize()
        .map_err(|_| "无法读取游戏日志")?
        .starts_with(&root)
    {
        return Err("游戏日志超出游戏目录".into());
    }
    let mut file = std::fs::File::open(path).map_err(|_| "无法读取游戏日志")?;
    let length = file.metadata().map_err(|_| "无法读取游戏日志")?.len();
    let maximum = cobble_diagnostics::MAX_INPUT_BYTES as u64;
    let truncated = length > maximum;
    if truncated {
        file.seek(SeekFrom::Start(length - maximum))
            .map_err(|_| "无法读取游戏日志")?;
    }
    let mut bytes = Vec::new();
    file.take(maximum)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取游戏日志")?;
    if truncated {
        let start = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |index| index + 1);
        bytes.drain(..start);
    }
    if bytes.is_empty() {
        return Err("日志末尾没有可分析的完整行".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "日志必须为 UTF-8 文本")?;
    let bytes = cobble_diagnostics::redact_log(text).into_bytes();
    if bytes.len() > maximum as usize {
        return Err("脱敏后的日志超过分析大小限制".into());
    }
    Ok((bytes, name, truncated))
}

#[tauri::command]
pub async fn analyze_game_report(
    app: AppHandle,
    game_id: String,
    locale: Option<String>,
    current_launch: Option<bool>,
) -> Result<Option<Report>, String> {
    let locale = locale.unwrap_or_else(|| "en".into());
    super::extension_contributions::validate_locale(&locale)?;
    let cancel = state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .begin("crash-doctor".into())?;
    let result = async {
        let directory = crate::games::game_directory(&app, &game_id)?;
        let current_only = current_launch.unwrap_or(false);
        if current_only {
            crate::games::launch_recovery::validate_current_failure(&app, &game_id)?;
        }
        let snapshot_app = app.clone();
        let snapshot_game_id = game_id.clone();
        let (plugin, component, bytes, source, truncated) =
            tauri::async_runtime::spawn_blocking(move || {
                let plugin = selected_plugin("crash-doctor")?;
                let component = package(&plugin)?;
                let (bytes, source, truncated) = if current_only {
                    crate::games::launch_recovery::with_current_failure(
                        &snapshot_app,
                        &snapshot_game_id,
                        |directory| game_log_snapshot_for_launch(directory, true),
                    )?
                } else {
                    game_log_snapshot(&directory)?
                };
                Ok::<_, String>((plugin, component, bytes, source, truncated))
            })
            .await
            .map_err(|_| "无法读取游戏日志")??;
        let mut report = execute_report(
            plugin,
            component,
            bytes,
            "text/plain",
            cancel.clone(),
            locale.clone(),
            Some(game_id.clone()),
        )
        .await?;
        if let Some(report) = &mut report {
            report.findings.push(cobble_plugin_api::report::Finding {
                code: "log_excerpt".into(),
                severity: cobble_plugin_api::report::Severity::Info,
                title: source.into(),
                message: if truncated {
                    "已分析最近 1 MiB 日志中的完整行；证据行号从截取内容的第 1 行起算。"
                } else {
                    "证据行号对应原始日志。"
                }
                .into(),
                evidence: vec![],
                suggestions: vec![],
            });
        }
        Ok(report)
    }
    .await;
    state()
        .lock()
        .map_err(|_| "分析状态不可用")?
        .finish(&cancel, result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_failure_never_falls_back_to_previous_game_log() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("logs")).unwrap();
        std::fs::write(directory.path().join("logs/latest.log"), "old failure\n").unwrap();
        assert!(game_log_snapshot_for_launch(directory.path(), true).is_err());
        std::fs::create_dir(directory.path().join(".cobble-logs")).unwrap();
        std::fs::write(
            directory.path().join(".cobble-logs/launcher.log"),
            "current failure\n",
        )
        .unwrap();
        let (bytes, name, _) = game_log_snapshot_for_launch(directory.path(), true).unwrap();
        assert_eq!(bytes, b"current failure\n");
        assert_eq!(name, ".cobble-logs/launcher.log");
    }

    #[test]
    fn resource_pack_zip_requires_explicit_protocol_before_opening_input() {
        let mut manifest: PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/pack-metadata/plugin.json"
        ))
        .unwrap();
        manifest.report_protocols.clear();
        assert!(
            selected_snapshot(Path::new("missing-input.zip"), &manifest)
                .unwrap_err()
                .contains("插件未获准")
        );
        manifest
            .report_protocols
            .push(cobble_plugin_api::PACK_METADATA_PROTOCOL.into());
        manifest
            .file_extensions
            .retain(|extension| extension != "zip");
        assert!(
            selected_snapshot(Path::new("missing-input.zip"), &manifest)
                .unwrap_err()
                .contains("插件未获准")
        );
    }
    #[test]
    fn game_snapshot_uses_recent_log_and_preserves_bounded_relative_lines() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("logs")).unwrap();
        let mut content = vec![b'x'; cobble_diagnostics::MAX_INPUT_BYTES + 20];
        content.extend_from_slice(b"\njava.lang.OutOfMemoryError: Java heap space\n");
        std::fs::write(directory.path().join("logs/latest.log"), content).unwrap();
        let (bytes, source, truncated) = game_log_snapshot(directory.path()).unwrap();
        assert!(truncated);
        assert_eq!(source, "logs/latest.log");
        assert_eq!(bytes, b"java.lang.OutOfMemoryError: Java heap space\n");
    }

    #[test]
    fn game_snapshot_falls_back_to_launcher_log_and_redacts() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join(".cobble-logs")).unwrap();
        let text = "safe\n--accessToken secret-value\n";
        std::fs::write(directory.path().join(".cobble-logs/launcher.log"), text).unwrap();
        let (bytes, source, truncated) = game_log_snapshot(directory.path()).unwrap();
        assert!(!truncated);
        assert_eq!(source, ".cobble-logs/launcher.log");
        assert_eq!(bytes, cobble_diagnostics::redact_log(text).as_bytes());
        assert_eq!(String::from_utf8(bytes).unwrap().lines().count(), 2);
    }

    #[test]
    fn cancellation_revokes_late_result_and_keeps_exclusive_ownership() {
        let mut state = AnalysisState::default();
        let cancel = state.begin("doctor".into()).unwrap();
        state.cancel(Some("other"));
        assert!(!cancel.load(Ordering::SeqCst));
        state.cancel(Some("doctor"));
        assert!(state.begin("second".into()).is_err());
        assert_eq!(state.finish(&cancel, Ok(Some(42))).unwrap(), None);
        assert!(state.begin("second".into()).is_ok());
    }

    #[test]
    fn stale_completion_cannot_clear_current_analysis() {
        let mut state = AnalysisState::default();
        let first = state.begin("doctor".into()).unwrap();
        assert_eq!(state.finish(&first, Ok(Some(1))).unwrap(), Some(1));
        let second = state.begin("doctor".into()).unwrap();
        assert_eq!(state.finish(&first, Ok(Some(2))).unwrap(), None);
        state.cancel(None);
        assert!(second.load(Ordering::SeqCst));
    }

    #[test]
    fn snapshots_are_bounded_utf8_and_extension_authorized() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.LOG");
        std::fs::write(&path, b"safe\n").unwrap();
        let extensions = vec!["log".into()];
        assert_eq!(snapshot(&path, &extensions).unwrap().1, "text/plain");
        assert!(snapshot(&path, &["json".into()]).is_err());
        std::fs::write(&path, [0xff]).unwrap();
        assert!(snapshot(&path, &extensions).is_err());
        std::fs::write(&path, vec![b'x'; cobble_diagnostics::MAX_INPUT_BYTES + 1]).unwrap();
        assert!(snapshot(&path, &extensions).is_err());
        let missing =
            snapshot(&directory.path().join("private-token.log"), &extensions).unwrap_err();
        assert!(!missing.contains("private-token"));
    }

    #[test]
    fn json_snapshot_is_immutable_and_has_no_text_media_type() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("index.json");
        std::fs::write(&path, b"{\"files\":[]}").unwrap();
        let (bytes, media_type) = snapshot(&path, &["json".into()]).unwrap();
        std::fs::write(&path, b"changed").unwrap();
        assert_eq!(bytes, b"{\"files\":[]}");
        assert_eq!(media_type, "application/json");
    }

    #[test]
    fn config_picker_snapshots_are_bounded_utf8_and_preserve_format_and_bytes() {
        let directory = tempfile::tempdir().unwrap();
        for (extension, media) in [
            ("toml", "application/toml"),
            ("properties", "text/x-java-properties"),
            ("yaml", "application/yaml"),
            ("yml", "application/yaml"),
        ] {
            let path = directory.path().join(format!("selected.{extension}"));
            let source = b"enabled = true\n";
            std::fs::write(&path, source).unwrap();
            let (bytes, actual_media) = snapshot(&path, &[extension.into()]).unwrap();
            assert_eq!(bytes, source);
            assert_eq!(actual_media, media);
            assert!(snapshot(&path, &["json".into()]).is_err());
            std::fs::write(&path, [255]).unwrap();
            assert!(snapshot(&path, &[extension.into()]).is_err());
            std::fs::write(&path, vec![b'x'; cobble_diagnostics::MAX_INPUT_BYTES + 1]).unwrap();
            assert!(snapshot(&path, &[extension.into()]).is_err());
        }
    }
    #[test]
    fn mcmeta_snapshot_requires_declared_extension_and_preserves_json_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pack.mcmeta");
        let source = br#"{"pack":{"pack_format":34,"description":"Original"}}"#;
        std::fs::write(&path, source).unwrap();
        let (bytes, media) = snapshot(&path, &["mcmeta".into()]).unwrap();
        assert_eq!(bytes, source);
        assert_eq!(media, "application/json");
        assert!(snapshot(&path, &["json".into()]).is_err());
    }
}
