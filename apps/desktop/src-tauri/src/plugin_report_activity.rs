// SPDX-License-Identifier: MIT
//! Report-host lifecycle metadata only. Never accept raw error messages or input labels.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
pub(super) struct Activity {
    plugin_id: String,
    interface: &'static str,
    started: Instant,
    cancel: Arc<AtomicBool>,
    outcome: &'static str,
    code: &'static str,
}
fn safe_id(value: &str) -> String {
    if !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        value.into()
    } else {
        "invalid".into()
    }
}
fn safe_code(value: &str) -> &'static str {
    match value {
        "CANCEL_REQUESTED" => "CANCEL_REQUESTED",
        "PLUGIN_CLOSED" => "PLUGIN_CLOSED",
        "PLUGIN_CRASH" => "PLUGIN_CRASH",
        "PLUGIN_ENGINE" => "PLUGIN_ENGINE",
        "PLUGIN_INVALID_COMPONENT" => "PLUGIN_INVALID_COMPONENT",
        "PLUGIN_INVALID_CONTEXT" => "PLUGIN_INVALID_CONTEXT",
        "PLUGIN_INVALID_PACKAGE" => "PLUGIN_INVALID_PACKAGE",
        "PLUGIN_INVALID_REPLY" => "PLUGIN_INVALID_REPLY",
        "PLUGIN_IO" => "PLUGIN_IO",
        "PLUGIN_LIMIT" => "PLUGIN_LIMIT",
        "PLUGIN_PERMISSION_DENIED" => "PLUGIN_PERMISSION_DENIED",
        "PLUGIN_PROTOCOL" => "PLUGIN_PROTOCOL",
        "PLUGIN_REPORT_FAILED" => "PLUGIN_REPORT_FAILED",
        "PLUGIN_TIMEOUT" => "PLUGIN_TIMEOUT",
        "PLUGIN_TRAP" => "PLUGIN_TRAP",
        "PLUGIN_UNSUPPORTED" => "PLUGIN_UNSUPPORTED",
        "PLUGIN_CHANGED" => "PLUGIN_CHANGED",
        "PLUGIN_HOST_UNAVAILABLE" => "PLUGIN_HOST_UNAVAILABLE",
        _ => "PLUGIN_REPORT_FAILED",
    }
}
impl Activity {
    pub(super) fn new(plugin_id: &str, api_version: &str, cancel: Arc<AtomicBool>) -> Self {
        let activity = Self {
            plugin_id: safe_id(plugin_id),
            interface: match api_version {
                "0.1.0" => "launcher:report@0.1.0",
                "0.2.0" => "launcher:report@0.2.0",
                _ => "launcher:report@unknown",
            },
            started: Instant::now(),
            cancel,
            outcome: "failed",
            code: "PLUGIN_HOST_INTERRUPTED",
        };
        tracing::info!(
            event = "plugin_report_host_started",
            plugin_id = activity.plugin_id.as_str(),
            interface = activity.interface,
            outcome = "started",
            code = "OK",
            duration_ms = 0_u64
        );
        activity
    }
    pub(super) fn failed(&mut self, code: &str) {
        self.outcome = if code == "CANCEL_REQUESTED" {
            "cancelled"
        } else {
            "failed"
        };
        self.code = safe_code(code);
    }
    pub(super) fn completed(&mut self) {
        self.outcome = "completed";
        self.code = "OK";
    }
    fn result(&self) -> (&'static str, &'static str) {
        if self.cancel.load(Ordering::Acquire) {
            ("cancelled", "CANCEL_REQUESTED")
        } else {
            (self.outcome, self.code)
        }
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        let (outcome, code) = self.result();
        tracing::info!(
            event = "plugin_report_host_finished",
            plugin_id = self.plugin_id.as_str(),
            interface = self.interface,
            outcome,
            code,
            duration_ms = self.started.elapsed().as_millis() as u64
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_fields_never_accept_raw_errors_or_paths() {
        assert_eq!(safe_id("config-doctor"), "config-doctor");
        assert_eq!(safe_id("C:/Users/private/config.toml"), "invalid");
        assert_eq!(safe_code("PLUGIN_TIMEOUT"), "PLUGIN_TIMEOUT");
        assert_eq!(
            safe_code("PLUGIN_IO: private-path or token"),
            "PLUGIN_REPORT_FAILED"
        );
        assert_eq!(safe_code("PRIVATESECRET"), "PLUGIN_REPORT_FAILED");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut activity = Activity::new("config-doctor", "0.2.0", cancel.clone());
        activity.completed();
        assert_eq!(activity.result(), ("completed", "OK"));
        cancel.store(true, Ordering::Release);
        assert_eq!(activity.result(), ("cancelled", "CANCEL_REQUESTED"));
    }
}
