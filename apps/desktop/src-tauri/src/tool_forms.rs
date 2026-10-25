// SPDX-License-Identifier: MIT
//! Explicit, transient form computation. No files, settings or game context grants.
use cobble_plugin_api::{PluginManifest, ToolFormOutput, ToolFormRequest};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ToolFormError {
    Input { code: String, message: String },
    Host(String),
}
impl From<String> for ToolFormError {
    fn from(value: String) -> Self {
        Self::Host(value)
    }
}
impl From<&str> for ToolFormError {
    fn from(value: &str) -> Self {
        Self::Host(value.into())
    }
}
impl ToolFormError {
    fn plugin(error: cobble_plugin_api::PluginError) -> Self {
        if error.code == "PLUGIN_TOOL_INPUT"
            && !error.message.trim().is_empty()
            && error.message.len() <= 512
            && !error.message.chars().any(char::is_control)
        {
            Self::Input {
                code: error.code,
                message: error.message,
            }
        } else {
            Self::Host(error.code)
        }
    }
}

#[derive(Default)]
struct Jobs(HashMap<String, Arc<AtomicBool>>);
impl Jobs {
    fn begin(&mut self, id: &str) -> Result<Arc<AtomicBool>, String> {
        if self.0.contains_key(id) || self.0.len() >= 4 {
            return Err("PLUGIN_TOOL_FORM_BUSY".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.0.insert(id.into(), cancel.clone());
        Ok(cancel)
    }
    fn cancel(&self, id: &str) {
        if let Some(cancel) = self.0.get(id) {
            cancel.store(true, Ordering::SeqCst);
        }
    }
    fn finish<T>(
        &mut self,
        id: &str,
        cancel: &Arc<AtomicBool>,
        result: Result<T, ToolFormError>,
    ) -> Result<Option<T>, ToolFormError> {
        if !self
            .0
            .get(id)
            .is_some_and(|active| Arc::ptr_eq(active, cancel))
        {
            return Ok(None);
        }
        self.0.remove(id);
        if cancel.load(Ordering::SeqCst) {
            Ok(None)
        } else {
            result.map(Some)
        }
    }
}
static JOBS: OnceLock<Mutex<Jobs>> = OnceLock::new();
fn jobs() -> &'static Mutex<Jobs> {
    JOBS.get_or_init(Default::default)
}
struct ActiveJob {
    id: String,
    cancel: Arc<AtomicBool>,
}
impl Drop for ActiveJob {
    fn drop(&mut self) {
        if let Ok(mut jobs) = jobs().lock() {
            let _ = jobs.finish::<()>(
                &self.id,
                &self.cancel,
                Err("PLUGIN_TOOL_FORM_WORKER_FAILED".into()),
            );
        }
    }
}
pub(super) fn cancel_for_plugin(id: &str) {
    if let Ok(jobs) = jobs().lock() {
        jobs.cancel(id);
    }
}
#[tauri::command]
pub fn cancel_plugin_tool_form(plugin_id: String) -> Result<(), String> {
    super::valid_id(&plugin_id)?;
    jobs()
        .lock()
        .map_err(|_| "PLUGIN_TOOL_FORM_STATE_UNAVAILABLE")?
        .cancel(&plugin_id);
    Ok(())
}
fn current(manifest: &PluginManifest) -> Result<(), String> {
    let current = super::registry()?
        .resolve_enabled(&manifest.id)
        .map_err(|error| error.to_string())?;
    if current.manifest != *manifest {
        return Err("PLUGIN_CHANGED".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn run_plugin_tool_form(
    plugin_id: String,
    input: serde_json::Value,
    locale: String,
) -> Result<Option<ToolFormOutput>, ToolFormError> {
    super::valid_id(&plugin_id)?;
    let request = ToolFormRequest { locale, input };
    request.validate_envelope().map_err(|error| error.code)?;
    let cancel = jobs()
        .lock()
        .map_err(|_| "PLUGIN_TOOL_FORM_STATE_UNAVAILABLE")?
        .begin(&plugin_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _active = ActiveJob {
            id: plugin_id.clone(),
            cancel: cancel.clone(),
        };
        let result = (|| {
            let plugin = super::registry()?
                .resolve_enabled(&plugin_id)
                .map_err(|error| error.to_string())?;
            request
                .validate(&plugin.manifest)
                .map_err(|error| error.code)?;
            let component = super::plugin_reports::package(&plugin)?;
            current(&plugin.manifest)?;
            let host = std::env::current_exe()
                .map_err(|_| "PLUGIN_HOST_UNAVAILABLE")?
                .with_file_name(if cfg!(windows) {
                    "cobble-plugin-host.exe"
                } else {
                    "cobble-plugin-host"
                });
            let output = cobble_plugin_host::run_tool_form(
                &host,
                &component,
                &plugin.manifest,
                &request,
                cancel.clone(),
            )
            .map_err(ToolFormError::plugin)?;
            current(&plugin.manifest)?;
            Ok::<_, ToolFormError>(output)
        })();
        let mut state = jobs()
            .lock()
            .map_err(|_| "PLUGIN_TOOL_FORM_STATE_UNAVAILABLE")?;
        state.finish(&plugin_id, &cancel, result)
    })
    .await
    .map_err(|_| "PLUGIN_TOOL_FORM_WORKER_FAILED".to_owned())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_bounded_guest_validation_messages_are_exposed() {
        let guest = ToolFormError::plugin(cobble_plugin_api::PluginError::new(
            "PLUGIN_TOOL_INPUT",
            "Choose a valid target.",
        ));
        assert_eq!(
            serde_json::to_value(guest).unwrap(),
            serde_json::json!({"code":"PLUGIN_TOOL_INPUT","message":"Choose a valid target."})
        );
        for (code, message) in [
            ("PLUGIN_ENGINE", "private internal detail"),
            ("PLUGIN_TOOL_INPUT", "line\nbreak"),
        ] {
            assert_eq!(
                serde_json::to_value(ToolFormError::plugin(cobble_plugin_api::PluginError::new(
                    code, message
                )))
                .unwrap(),
                serde_json::json!(code)
            );
        }
    }
    #[test]
    fn cancellation_is_scoped_and_suppresses_late_output() {
        let mut jobs = Jobs::default();
        let first = jobs.begin("first").unwrap();
        let other = jobs.begin("other").unwrap();
        assert!(jobs.begin("first").is_err());
        jobs.cancel("first");
        assert!(!other.load(Ordering::SeqCst));
        assert_eq!(jobs.finish("first", &first, Ok(42)).unwrap(), None);
        assert_eq!(jobs.finish("other", &other, Ok(7)).unwrap(), Some(7));
    }
    #[test]
    fn old_completion_cannot_remove_a_new_run() {
        let mut jobs = Jobs::default();
        let old = jobs.begin("tool").unwrap();
        assert_eq!(jobs.finish("tool", &old, Ok(1)).unwrap(), Some(1));
        let current = jobs.begin("tool").unwrap();
        assert_eq!(jobs.finish("tool", &old, Ok(2)).unwrap(), None);
        assert!(jobs.begin("tool").is_err());
        assert_eq!(jobs.finish("tool", &current, Ok(3)).unwrap(), Some(3));
    }
}
