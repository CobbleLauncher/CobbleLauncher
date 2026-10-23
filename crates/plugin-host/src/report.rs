// SPDX-License-Identifier: MIT
//! One-shot report execution. The process and its pipes belong to this call only.
use cobble_plugin_api::{MAX_COMPONENT_BYTES, MAX_FILE_BYTES, PluginError, Report};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub(crate) const MAX_REPORT_BYTES: usize = 2 * 1024 * 1024;
pub fn check_input(component: &[u8], data: &[u8], media: &str) -> Result<(), PluginError> {
    if component.len() > MAX_COMPONENT_BYTES
        || data.len() > MAX_FILE_BYTES
        || (media == cobble_plugin_api::CONFIG_FOLDER_MEDIA_TYPE
            && data.len() > cobble_plugin_api::MAX_CONFIG_FOLDER_ENVELOPE_BYTES)
        || (matches!(
            media,
            "text/plain" | "application/toml" | "text/x-java-properties" | "application/yaml"
        ) && data.len() > 1024 * 1024)
    {
        return Err(PluginError::new(
            "PLUGIN_LIMIT",
            "Report input exceeds budget",
        ));
    }
    if !matches!(
        media,
        "text/plain"
            | "application/json"
            | "application/toml"
            | "text/x-java-properties"
            | "application/yaml"
            | "application/vnd.cobble.mrpack-inspection+json"
            | "application/vnd.cobble.config-folder+json"
    ) {
        return Err(PluginError::new(
            "PLUGIN_UNSUPPORTED",
            "Unsupported report input type",
        ));
    }
    Ok(())
}

pub fn run_report(
    host: &Path,
    component: &[u8],
    data: &[u8],
    media_type: &str,
    cancel: Arc<AtomicBool>,
) -> Result<Report, PluginError> {
    execute_report(host, component, data, media_type, cancel, None)
}

pub fn run_report_with_context(
    host: &Path,
    component: &[u8],
    data: &[u8],
    media_type: &str,
    cancel: Arc<AtomicBool>,
    context: &super::ReportHostContext,
) -> Result<Report, PluginError> {
    context.validate()?;
    execute_report(host, component, data, media_type, cancel, Some(context))
}
fn execute_report(
    host: &Path,
    component: &[u8],
    data: &[u8],
    media_type: &str,
    cancel: Arc<AtomicBool>,
    context: Option<&super::ReportHostContext>,
) -> Result<Report, PluginError> {
    check_input(component, data, media_type)?;
    let mut frames = vec![
        component.to_vec(),
        data.to_vec(),
        media_type.as_bytes().to_vec(),
    ];
    if let Some(context) = context {
        frames.push(
            serde_json::to_vec(context).map_err(|_| {
                PluginError::new("PLUGIN_INVALID_CONTEXT", "Invalid report context")
            })?,
        );
    }
    run_worker(host, "--report-stdio", frames, cancel, |report| {
        cobble_plugin_api::report::validate_report(&report, media_type, data)?;
        Ok(report)
    })
}

pub(crate) fn run_worker<T: serde::de::DeserializeOwned>(
    host: &Path,
    mode: &str,
    frames: Vec<Vec<u8>>,
    cancel: Arc<AtomicBool>,
    validate: impl FnOnce(T) -> Result<T, PluginError>,
) -> Result<T, PluginError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut worker = super::worker_session::WorkerSession::spawn(
        host,
        mode,
        frames,
        MAX_REPORT_BYTES + 1024,
        &cancel,
    )?;
    worker.finish_input();
    let result = worker.receive_validated(&cancel, validate);
    worker.close();
    if !worker.has_exited().unwrap_or(false) {
        return Err(PluginError::new(
            "PLUGIN_IO",
            "Could not reap plugin worker",
        ));
    }
    if cancel.load(Ordering::Acquire) {
        return Err(PluginError::new(
            "CANCEL_REQUESTED",
            "Plugin call cancelled",
        ));
    }
    if Instant::now() >= deadline {
        return Err(PluginError::new(
            "PLUGIN_TIMEOUT",
            "Plugin call exceeded 10 seconds",
        ));
    }
    result
}
