// SPDX-License-Identifier: MIT
//! Bounded, in-process sharing of authorized durable artifact downloads.
use crate::{
    ArtifactDownloader, DownloadError, DownloadRequest, DownloadResult, NetError,
    TransferCancellation, TransferError,
};
use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
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
    source: String,
    item: String,
    expected_bytes: u64,
    subscribers: usize,
    cancel: TransferCancellation,
    result: watch::Sender<Option<Outcome>>,
}
#[derive(Clone)]
pub struct DownloadManager {
    downloader: ArtifactDownloader,
    jobs: Arc<Mutex<HashMap<[u8; 32], Job>>>,
    transferred: Arc<AtomicU64>,
    admission: Arc<crate::adaptive::Admission>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveDownloadProgress {
    pub source: String,
    pub item: String,
    pub bytes_received: u64,
    pub bytes_total: u64,
}
#[derive(Clone, Debug, Default)]
pub struct DownloadMetrics {
    pub bytes_received: u64,
    pub active_downloads: usize,
    pub items: Vec<ActiveDownloadProgress>,
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
    /// Scheduling bound also respects the manager's sixteen unique-job ceiling.
    pub fn concurrency(&self) -> usize {
        self.downloader.concurrency().min(16)
    }
    pub fn new(downloader: ArtifactDownloader) -> Self {
        Self::with_adaptive(downloader, crate::download_tuning().adaptive)
    }
    /// Explicit mode for callers constructing a workflow with its own settings.
    pub fn with_adaptive(downloader: ArtifactDownloader, adaptive: bool) -> Self {
        let admission = crate::adaptive::Admission::new(downloader.concurrency().min(16), adaptive);
        Self {
            downloader,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            transferred: Arc::new(AtomicU64::new(0)),
            admission,
        }
    }
    pub fn current_concurrency(&self) -> usize {
        self.admission.limit()
    }
    pub fn metrics(&self) -> DownloadMetrics {
        let mut metrics = DownloadMetrics::default();
        if let Ok(jobs) = self.jobs.lock() {
            metrics.bytes_received = self.transferred.load(Ordering::Relaxed);
            for job in jobs.values() {
                let bytes = job.cancel.observed_bytes();
                metrics.bytes_received = metrics.bytes_received.saturating_add(bytes);
                let active = job.cancel.active_transfers();
                metrics.active_downloads += active;
                if active > 0 {
                    metrics.items.push(ActiveDownloadProgress {
                        source: job.source.clone(),
                        item: job.item.clone(),
                        bytes_received: bytes,
                        bytes_total: job.expected_bytes,
                    });
                }
            }
        }
        metrics
            .items
            .sort_by(|left, right| left.item.cmp(&right.item));
        metrics
    }
    /// Call after producers stop scheduling. Cancelled subscriptions can return
    /// before their blocking owners exit; job results are published only after
    /// `download_shared_before` has drained those owners.
    pub async fn wait_idle(&self) {
        loop {
            let receivers: Vec<_> = self
                .jobs
                .lock()
                .expect("download jobs mutex")
                .values()
                .map(|job| job.result.subscribe())
                .collect();
            if receivers.is_empty() {
                return;
            }
            for mut receive in receivers {
                loop {
                    if receive.borrow_and_update().is_some() {
                        break;
                    }
                    if receive.changed().await.is_err() {
                        break;
                    }
                }
            }
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
                let (source, item, expected_bytes) = request.progress_labels();
                jobs.insert(
                    key,
                    Job {
                        source,
                        item,
                        expected_bytes,
                        subscribers: 1,
                        cancel: job_cancel.clone(),
                        result,
                    },
                );
                let manager = self.clone();
                // One absolute budget includes queueing, retries and backoff.
                let deadline = Instant::now() + self.downloader.total_timeout();
                tokio::spawn(async move {
                    let started = Instant::now();
                    let outcome = manager.run(request, job_cancel, deadline).await;
                    if let Ok(mut jobs) = manager.jobs.lock()
                        && let Some(job) = jobs.remove(&key)
                    {
                        match &outcome {
                            Ok(_) => {
                                tracing::debug!(target: "cobble_net::download", source = %job.source, item = %job.item, elapsed_ms = started.elapsed().as_millis() as u64, bytes_received = job.cancel.observed_bytes(), outcome = "verified", "download finished")
                            }
                            Err(error) => {
                                tracing::warn!(target: "cobble_net::download", source = %job.source, item = %job.item, elapsed_ms = started.elapsed().as_millis() as u64, category = download_category(error), "download failed")
                            }
                        }
                        manager
                            .transferred
                            .fetch_add(job.cancel.observed_bytes(), Ordering::Relaxed);
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
            let lease = tokio::select! { biased;
                _ = cancel.cancelled() => return Err(Arc::new(DownloadError::Cancelled)),
                value = tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), self.admission.acquire()) => match value { Ok(lease) => lease, Err(_) => return Err(Arc::new(DownloadError::Deadline)) },
            };
            let before = cancel.observed_bytes();
            let outcome = self
                .downloader
                .download_shared_before(request.clone(), cancel.clone(), deadline)
                .await;
            let congestion = matches!(
                &outcome,
                Err(DownloadError::Deadline
                    | DownloadError::Transfer(
                        TransferError::Network(NetError::Timeout(_))
                            | TransferError::HttpStatus(429 | 503)
                            | TransferError::HttpRetry {
                                status: 429 | 503,
                                ..
                            }
                    ))
            );
            self.admission
                .observe(cancel.observed_bytes().saturating_sub(before), congestion);
            drop(lease);
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
fn download_category(error: &DownloadError) -> &'static str {
    match error {
        DownloadError::InvalidRequest => "invalid_request",
        DownloadError::Policy(_) => "policy",
        DownloadError::Transfer(_) => "transport",
        DownloadError::Staging(_) => "staging",
        DownloadError::Integrity(_) => "integrity",
        DownloadError::Cancelled => "cancelled",
        DownloadError::Deadline => "deadline",
        DownloadError::Worker => "worker",
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
