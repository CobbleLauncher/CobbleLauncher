// SPDX-License-Identifier: MIT
//! Opt-in observation from the real WebView. These are application milestones,
//! not OS process-creation timings or a native startup percentile benchmark.
use serde::Serialize;
use std::{
    sync::OnceLock,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;

static STARTED: OnceLock<Instant> = OnceLock::new();
static HOME: OnceLock<HomeReadyMetrics> = OnceLock::new();

#[derive(Clone, Debug, Serialize)]
pub struct HomeReadyMetrics {
    pub native_main_to_home_report_ms: u64,
    pub renderer_home_ready_ms: f64,
    pub home_report_unix_ms: u64,
}

/// Call at native main entry, before logging or state initialization.
pub fn mark_started() {
    STARTED.get_or_init(Instant::now);
}

fn measurement(started: Instant, renderer_ready_ms: f64) -> Result<HomeReadyMetrics, String> {
    if !renderer_ready_ms.is_finite() || !(0.0..=3_600_000.0).contains(&renderer_ready_ms) {
        return Err("Invalid renderer readiness timing".into());
    }
    Ok(HomeReadyMetrics {
        native_main_to_home_report_ms: started.elapsed().as_millis() as u64,
        renderer_home_ready_ms: renderer_ready_ms,
        home_report_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis() as u64,
    })
}

/// Frontend invokes once after home summaries are rendered and two animation
/// frames have elapsed. Native duration ends at IPC receipt, not exact paint.
#[tauri::command]
pub fn report_home_ready(
    app: AppHandle,
    renderer_ready_ms: f64,
) -> Result<HomeReadyMetrics, String> {
    if let Some(metrics) = HOME.get() {
        return Ok(metrics.clone());
    }
    let started = STARTED
        .get()
        .copied()
        .ok_or("Native startup timer was not initialized")?;
    let metrics = measurement(started, renderer_ready_ms)?;
    if HOME.set(metrics.clone()).is_ok() {
        tracing::info!(
            event = "home_ready_report_received",
            native_main_to_home_report_ms = metrics.native_main_to_home_report_ms,
            renderer_home_ready_ms = metrics.renderer_home_ready_ms,
            home_report_unix_ms = metrics.home_report_unix_ms
        );
        // Session filesystem/process checks begin after the home frame reports.
        crate::games::sessions::watch_recovered(&app);
    }
    Ok(HOME.get().expect("home metrics initialized").clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timings_keep_native_and_renderer_scopes_separate_and_reject_invalid_values() {
        let started = Instant::now() - std::time::Duration::from_millis(30);
        let metrics = measurement(started, 12.5).unwrap();
        assert!(metrics.native_main_to_home_report_ms >= 30);
        assert_eq!(metrics.renderer_home_ready_ms, 12.5);
        assert!(metrics.home_report_unix_ms > 1_000_000_000_000);
        assert!(measurement(started, f64::NAN).is_err());
        assert!(measurement(started, f64::INFINITY).is_err());
        assert!(measurement(started, -1.0).is_err());
    }
}
