// SPDX-License-Identifier: MIT
//! In-memory ownership and capacity for seed computation; contains no seed values.
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_ACTIVE: usize = 2;

#[derive(Clone, Hash, PartialEq, Eq)]
pub(super) struct Owner {
    pub plugin: String,
    pub session: String,
}

pub(super) struct Job {
    pub request_id: String,
    pub cancel: Arc<AtomicBool>,
    pub finished: AtomicBool,
}

#[derive(Default)]
pub(super) struct Jobs(HashMap<Owner, Arc<Job>>);

pub(super) fn validate_request_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err("SEED_MAP_REQUEST_INVALID".into());
    }
    Ok(())
}

impl Jobs {
    pub fn begin(&mut self, owner: Owner, request_id: &str) -> Result<Arc<Job>, String> {
        validate_request_id(request_id)?;
        if let Some(active) = self.0.get(&owner) {
            return Err(if active.request_id == request_id {
                "SEED_MAP_DUPLICATE_REQUEST"
            } else {
                "SEED_MAP_BUSY"
            }
            .into());
        }
        if self.0.len() >= MAX_ACTIVE {
            return Err("SEED_MAP_BUSY".into());
        }
        let job = Arc::new(Job {
            request_id: request_id.into(),
            cancel: Arc::new(AtomicBool::new(false)),
            finished: AtomicBool::new(false),
        });
        self.0.insert(owner, job.clone());
        Ok(job)
    }

    pub fn cancel(&self, owner: &Owner, request_id: &str) -> Option<Arc<Job>> {
        let job = self
            .0
            .get(owner)
            .filter(|job| job.request_id == request_id)?;
        job.cancel.store(true, Ordering::Release);
        Some(job.clone())
    }

    pub fn finish(&mut self, owner: &Owner, job: &Arc<Job>) {
        if self
            .0
            .get(owner)
            .is_some_and(|active| Arc::ptr_eq(active, job))
        {
            self.0.remove(owner);
        }
        job.finished.store(true, Ordering::Release);
    }

    pub fn complete<T>(
        &mut self,
        owner: &Owner,
        job: &Arc<Job>,
        result: Result<T, String>,
    ) -> Result<T, String> {
        let owned = self
            .0
            .get(owner)
            .is_some_and(|active| Arc::ptr_eq(active, job));
        self.finish(owner, job);
        if !owned || job.cancel.load(Ordering::Acquire) {
            Err("SEED_MAP_CANCELLED".into())
        } else {
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(plugin: &str, session: &str) -> Owner {
        Owner {
            plugin: plugin.into(),
            session: session.into(),
        }
    }

    #[test]
    fn cancelled_workers_retain_capacity_until_they_exit() {
        let mut jobs = Jobs::default();
        let first = owner("compass", "first");
        let second = owner("compass", "second");
        let third = owner("other", "third");
        let job = jobs.begin(first.clone(), "map-1").unwrap();
        jobs.begin(second, "map-2").unwrap();
        jobs.cancel(&first, "map-1").unwrap();
        assert!(job.cancel.load(Ordering::Acquire));
        assert!(!job.finished.load(Ordering::Acquire));
        assert_eq!(
            jobs.begin(first.clone(), "map-1").err().unwrap(),
            "SEED_MAP_DUPLICATE_REQUEST"
        );
        assert_eq!(
            jobs.begin(first.clone(), "map-3").err().unwrap(),
            "SEED_MAP_BUSY"
        );
        assert_eq!(
            jobs.begin(third.clone(), "map-3").err().unwrap(),
            "SEED_MAP_BUSY"
        );
        jobs.finish(&first, &job);
        assert!(job.finished.load(Ordering::Acquire));
        jobs.begin(third, "map-3").unwrap();
    }

    #[test]
    fn cancel_requires_native_plugin_session_and_matching_request() {
        let mut jobs = Jobs::default();
        let first = owner("compass", "first");
        let job = jobs.begin(first.clone(), "map-1").unwrap();
        for foreign in [owner("other", "first"), owner("compass", "second")] {
            assert!(jobs.cancel(&foreign, "map-1").is_none());
        }
        assert!(jobs.cancel(&first, "stale-id").is_none());
        assert!(!job.cancel.load(Ordering::Acquire));
        jobs.finish(&first, &job);
        let next = jobs.begin(first.clone(), "map-2").unwrap();
        jobs.finish(&first, &job);
        assert!(jobs.cancel(&first, "map-2").is_some());
        assert!(next.cancel.load(Ordering::Acquire));
    }

    #[test]
    fn request_ids_are_bounded_opaque_identifiers() {
        for invalid in ["", "seed value", "request/one", "你好", &"a".repeat(65)] {
            assert_eq!(
                validate_request_id(invalid).unwrap_err(),
                "SEED_MAP_REQUEST_INVALID"
            );
        }
        assert!(validate_request_id("map_1-ABC").is_ok());
    }

    #[test]
    fn cancel_suppresses_late_output_without_removing_new_work() {
        let mut jobs = Jobs::default();
        let owner = owner("compass", "first");
        let first = jobs.begin(owner.clone(), "map-1").unwrap();
        jobs.cancel(&owner, "map-1").unwrap();
        assert_eq!(
            jobs.complete(&owner, &first, Ok(42)),
            Err("SEED_MAP_CANCELLED".into())
        );
        let second = jobs.begin(owner.clone(), "map-2").unwrap();
        assert_eq!(
            jobs.complete(&owner, &first, Ok(99)),
            Err("SEED_MAP_CANCELLED".into())
        );
        assert!(!second.finished.load(Ordering::Acquire));
        assert_eq!(jobs.complete(&owner, &second, Ok(7)), Ok(7));
    }
}
