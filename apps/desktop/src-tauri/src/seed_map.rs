// SPDX-License-Identifier: MIT
//! Bounded, session-owned seed computation. No filesystem or network grants.
use crate::plugins::{plugin_views, plugin_views::ViewAuthorization};
use cobble_seed_map::{MapResult, SearchResult};
use serde::Deserialize;
use std::{
    sync::{Arc, Mutex, OnceLock, atomic::Ordering},
    time::Duration,
};

#[path = "seed_map_jobs.rs"]
mod jobs;
use jobs::{Job, Jobs, Owner};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateRequest {
    pub request_id: String,
    pub map: cobble_seed_map::MapRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchRequest {
    pub request_id: String,
    pub search: cobble_seed_map::SearchRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelRequest {
    pub request_id: String,
}

static JOBS: OnceLock<Mutex<Jobs>> = OnceLock::new();
fn jobs() -> &'static Mutex<Jobs> {
    JOBS.get_or_init(Default::default)
}

fn owner(auth: &ViewAuthorization) -> Owner {
    Owner {
        plugin: auth.manifest.id.clone(),
        session: auth.session_id.clone(),
    }
}

struct ActiveJob {
    owner: Owner,
    job: Arc<Job>,
}

impl ActiveJob {
    fn complete<T>(&self, result: Result<T, String>) -> Result<T, String> {
        plugin_views::clear_job(&self.owner.session, &self.job.cancel);
        jobs()
            .lock()
            .map_err(|_| "SEED_MAP_STATE_UNAVAILABLE")?
            .complete(&self.owner, &self.job, result)
    }
}

impl Drop for ActiveJob {
    fn drop(&mut self) {
        plugin_views::clear_job(&self.owner.session, &self.job.cancel);
        if let Ok(mut state) = jobs().lock() {
            state.finish(&self.owner, &self.job);
        } else {
            self.job.finished.store(true, Ordering::Release);
        }
    }
}

async fn run<T: Send + 'static>(
    auth: &ViewAuthorization,
    request_id: String,
    operation: impl FnOnce(&std::sync::atomic::AtomicBool) -> Result<T, cobble_seed_map::SeedMapError>
    + Send
    + 'static,
) -> Result<T, String> {
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    let owner = owner(auth);
    let job = jobs()
        .lock()
        .map_err(|_| "SEED_MAP_STATE_UNAVAILABLE")?
        .begin(owner.clone(), &request_id)?;
    let active = ActiveJob {
        owner,
        job: job.clone(),
    };
    plugin_views::register_job(&auth.session_id, job.cancel.clone())?;
    let output = tauri::async_runtime::spawn_blocking(move || {
        // Keep the slot and view job until native work actually exits, even when
        // the awaiting frontend future is dropped or the worker panics.
        if job.cancel.load(Ordering::Acquire) {
            return Err("SEED_MAP_CANCELLED".into());
        }
        let result = operation(&job.cancel).map_err(|error| error.code.to_owned());
        active.complete(result)
    })
    .await
    .map_err(|_| "SEED_MAP_WORKER_FAILED".to_owned())?;
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    output
}

pub async fn generate_for_frontend(
    auth: &ViewAuthorization,
    args: GenerateRequest,
) -> Result<MapResult, String> {
    run(auth, args.request_id, move |cancel| {
        cobble_seed_map::generate(&args.map, cancel)
    })
    .await
}

pub async fn search_for_frontend(
    auth: &ViewAuthorization,
    args: SearchRequest,
) -> Result<SearchResult, String> {
    run(auth, args.request_id, move |cancel| {
        cobble_seed_map::search(&args.search, cancel)
    })
    .await
}

/// The response means the matched worker has released its capacity. A stale ID
/// cannot cancel a replacement job, even within the same native view session.
pub async fn cancel_for_frontend(
    auth: &ViewAuthorization,
    args: CancelRequest,
) -> Result<bool, String> {
    jobs::validate_request_id(&args.request_id)?;
    if auth.cancel.load(Ordering::Acquire) {
        return Err("PLUGIN_VIEW_REVOKED".into());
    }
    let job = jobs()
        .lock()
        .map_err(|_| "SEED_MAP_STATE_UNAVAILABLE")?
        .cancel(&owner(auth), &args.request_id);
    let Some(job) = job else {
        return Ok(false);
    };
    while !job.finished.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(true)
}

pub fn capabilities_for_frontend() -> Result<cobble_seed_map::Capabilities, String> {
    Ok(cobble_seed_map::capabilities())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_native_job_rejects_real_engine_output_and_releases_its_slot() {
        let mut jobs = Jobs::default();
        let owner = Owner {
            plugin: "compass".into(),
            session: "view-fixture".into(),
        };
        let job = jobs.begin(owner.clone(), "map-1").unwrap();
        jobs.cancel(&owner, "map-1").unwrap();
        let request = cobble_seed_map::MapRequest {
            seed: "-9223372036854775808".into(),
            version: "1.21.1".into(),
            dimension: "overworld".into(),
            preset: "default".into(),
            y: 64,
            x: 0,
            z: 0,
            width: 16,
            height: 16,
            step: 4,
        };
        let result =
            cobble_seed_map::generate(&request, &job.cancel).map_err(|error| error.code.to_owned());
        assert_eq!(result.unwrap_err(), "SEED_MAP_CANCELLED");
        assert_eq!(
            jobs.complete(&owner, &job, Ok(42)),
            Err("SEED_MAP_CANCELLED".into())
        );
        assert!(jobs.begin(owner, "map-2").is_ok());
    }

    #[test]
    fn native_envelopes_reject_unexpected_fields() {
        assert!(
            serde_json::from_value::<CancelRequest>(serde_json::json!({
                "requestId": "map-1", "pluginId": "foreign"
            }))
            .is_err()
        );
        let request = serde_json::json!({
            "requestId": "map-1", "map": {
                "seed": "0", "version": "1.21.1", "dimension": "overworld",
                "preset": "default", "y": 64, "x": 0, "z": 0,
                "width": 16, "height": 16, "step": 4, "path": "arbitrary"
            }
        });
        assert!(serde_json::from_value::<GenerateRequest>(request).is_err());
    }
}
