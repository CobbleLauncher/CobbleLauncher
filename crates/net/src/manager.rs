// SPDX-License-Identifier: MIT
//! Bounded, in-process sharing of authorized durable artifact downloads.
use crate::{
    ArtifactDownloader, DownloadError, DownloadRequest, DownloadResult, NetError,
    TransferCancellation, TransferError,
};
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::watch;

type Outcome = Result<Arc<DownloadResult>, Arc<DownloadError>>;
#[derive(Debug)]
pub enum ManagedDownloadError {
    Busy,
    Cancelled,
    Download(Arc<DownloadError>),
}
impl fmt::Display for ManagedDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => {
                f.write_str("download manager is busy; retry after current workers finish")
            }
            Self::Cancelled => f.write_str("download subscription cancelled"),
            Self::Download(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ManagedDownloadError {}
struct Job {
    subscribers: usize,
    cancel: TransferCancellation,
    result: watch::Sender<Option<Outcome>>,
}
#[derive(Clone)]
pub struct DownloadManager {
    downloader: ArtifactDownloader,
    jobs: Arc<Mutex<HashMap<[u8; 32], Job>>>,
}
struct Subscription {
    manager: DownloadManager,
    key: [u8; 32],
    // A finished job may have been replaced before this subscriber returns.
    result: watch::Sender<Option<Outcome>>,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        if let Ok(mut jobs) = self.manager.jobs.lock()
            && let Some(job) = jobs.get_mut(&self.key)
            && job.result.same_channel(&self.result)
        {
            job.subscribers -= 1;
            if job.subscribers == 0 {
                job.cancel.cancel();
            }
        }
    }
}
impl DownloadManager {
    pub fn new(downloader: ArtifactDownloader) -> Self {
        Self {
            downloader,
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    /// Shares matching jobs only within this manager. Closing jobs remain Busy
    /// until their blocking workers release staging locks. Dropping this future
    /// releases its subscription, just like explicitly cancelling it.
    pub async fn download(
        &self,
        request: DownloadRequest,
        cancel: TransferCancellation,
    ) -> Result<Arc<DownloadResult>, ManagedDownloadError> {
        self.downloader
            .validate_request(&request)
            .map_err(|error| ManagedDownloadError::Download(Arc::new(error)))?;
        if cancel.is_cancelled() {
            return Err(ManagedDownloadError::Cancelled);
        }
        let key = request.key();
        let (mut receive, _subscription) = {
            let mut jobs = self.jobs.lock().map_err(|_| ManagedDownloadError::Busy)?;
            if let Some(job) = jobs.get_mut(&key) {
                if job.subscribers == 0 || job.subscribers >= 64 {
                    return Err(ManagedDownloadError::Busy);
                }
                job.subscribers += 1;
            } else {
                if jobs.len() >= 16 {
                    return Err(ManagedDownloadError::Busy);
                }
                let (result, _) = watch::channel(None);
                let job_cancel = TransferCancellation::default();
                jobs.insert(
                    key,
                    Job {
                        subscribers: 1,
                        cancel: job_cancel.clone(),
                        result,
                    },
                );
                let manager = self.clone();
                // One absolute budget includes queueing, retries and backoff.
                let deadline = Instant::now() + self.downloader.total_timeout();
                tokio::spawn(async move {
                    let outcome = manager.run(request, job_cancel, deadline).await;
                    if let Ok(mut jobs) = manager.jobs.lock()
                        && let Some(job) = jobs.remove(&key)
                    {
                        job.result.send_replace(Some(outcome));
                    }
                });
            }
            let job = jobs.get(&key).expect("inserted job");
            (
                job.result.subscribe(),
                Subscription {
                    manager: self.clone(),
                    key,
                    result: job.result.clone(),
                },
            )
        };
        loop {
            if let Some(outcome) = receive.borrow_and_update().clone() {
                return outcome.map_err(ManagedDownloadError::Download);
            }
            tokio::select! { biased;
                _ = cancel.cancelled() => return Err(ManagedDownloadError::Cancelled),
                changed = receive.changed() => if changed.is_err() {
                    return Err(ManagedDownloadError::Download(Arc::new(DownloadError::Worker)));
                },
            }
        }
    }
    async fn run(
        &self,
        request: DownloadRequest,
        cancel: TransferCancellation,
        deadline: Instant,
    ) -> Outcome {
        for attempt in 0..3 {
            let outcome = self
                .downloader
                .download_shared_before(request.clone(), cancel.clone(), deadline)
                .await;
            let error = match outcome {
                Ok(result) => return Ok(Arc::new(result)),
                Err(error) => error,
            };
            let Some(delay) = retry_delay(&error, attempt) else {
                return Err(Arc::new(error));
            };
            if attempt == 2 {
                return Err(Arc::new(error));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if delay >= remaining {
                return Err(Arc::new(DownloadError::Deadline));
            }
            tokio::select! { biased;
                _ = cancel.cancelled() => return Err(Arc::new(DownloadError::Cancelled)),
                _ = tokio::time::sleep(delay) => {},
            }
        }
        unreachable!("three attempts always return")
    }
}
fn retry_delay(error: &DownloadError, attempt: u32) -> Option<Duration> {
    let backoff = Duration::from_millis(100 * (1 << attempt));
    match error {
        DownloadError::Transfer(TransferError::HttpRetry {
            retry_after_seconds,
            ..
        }) => Some(backoff.max(Duration::from_secs(retry_after_seconds.unwrap_or(0)))),
        DownloadError::Transfer(
            TransferError::HttpStatus(500..=599)
            | TransferError::Network(
                NetError::Resolution
                | NetError::Connect
                | NetError::Protocol
                | NetError::Timeout(_),
            ),
        ) => Some(backoff),
        _ => None,
    }
}
