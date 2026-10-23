// SPDX-License-Identifier: MIT
use crate::BackupError;
use rustic_core::{Progress, ProgressBars, ProgressType, RusticProgress};
use serde::Serialize;
use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupPhase {
    Scan,
    Snapshot,
    Verify,
    Restore,
    Publish,
    Complete,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupProgress {
    pub phase: BackupPhase,
    /// Logical processed/read source bytes for the current phase, including reused bytes.
    pub source_bytes: u64,
    pub total_bytes: Option<u64>,
}

/// Shared cancellation flag is compatible with Core's existing operation cancellation.
/// Callbacks are synchronous, thread-safe and must return promptly. No paths are emitted.
#[derive(Clone)]
pub struct BackupControl {
    cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    progress: Arc<dyn Fn(BackupProgress) + Send + Sync>,
    state: Arc<Mutex<State>>,
    read: Arc<AtomicU64>,
    processed: Arc<AtomicU64>,
}
struct State {
    phase: BackupPhase,
    total: Option<u64>,
    last: Option<Instant>,
}
impl fmt::Debug for BackupControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BackupControl")
    }
}
impl Default for BackupControl {
    fn default() -> Self {
        Self::new(Arc::new(AtomicBool::new(false)), |_| {})
    }
}
impl BackupControl {
    pub fn new(
        cancelled: Arc<AtomicBool>,
        progress: impl Fn(BackupProgress) + Send + Sync + 'static,
    ) -> Self {
        Self::with_cancellation(move || cancelled.load(Ordering::Relaxed), progress)
    }
    pub fn with_cancellation(
        cancelled: impl Fn() -> bool + Send + Sync + 'static,
        progress: impl Fn(BackupProgress) + Send + Sync + 'static,
    ) -> Self {
        Self {
            cancelled: Arc::new(cancelled),
            progress: Arc::new(progress),
            state: Arc::new(Mutex::new(State {
                phase: BackupPhase::Scan,
                total: None,
                last: None,
            })),
            read: Arc::new(AtomicU64::new(0)),
            processed: Arc::new(AtomicU64::new(0)),
        }
    }
    pub(crate) fn is_cancelled(&self) -> bool {
        (self.cancelled)()
    }
    pub(crate) fn complete(&self, total: u64) {
        self.read.store(total, Ordering::Relaxed);
        self.processed.store(total, Ordering::Relaxed);
        {
            let mut state = self.state.lock().expect("backup progress lock");
            state.phase = BackupPhase::Complete;
            state.total = Some(total);
            state.last = None;
        }
        self.emit(true);
    }
    pub(crate) fn check(&self) -> Result<(), BackupError> {
        if self.is_cancelled() {
            Err(BackupError::Cancelled)
        } else {
            Ok(())
        }
    }
    pub(crate) fn error(&self, operation: &'static str) -> BackupError {
        if self.is_cancelled() {
            BackupError::Cancelled
        } else {
            BackupError::Engine(operation)
        }
    }
    pub(crate) fn phase(&self, phase: BackupPhase, total: Option<u64>) {
        self.read.store(0, Ordering::Relaxed);
        self.processed.store(0, Ordering::Relaxed);
        {
            let mut state = self.state.lock().expect("backup progress lock");
            state.phase = phase;
            state.total = total;
            state.last = None;
        }
        self.emit(true);
    }
    pub(crate) fn read_bytes(&self, bytes: u64) {
        self.read.fetch_add(bytes, Ordering::Relaxed);
        self.emit(false);
    }
    fn emit(&self, force: bool) {
        let event = {
            let mut state = self.state.lock().expect("backup progress lock");
            if !force
                && state
                    .last
                    .is_some_and(|time| time.elapsed() < Duration::from_millis(100))
            {
                return;
            }
            state.last = Some(Instant::now());
            let bytes = self
                .read
                .load(Ordering::Relaxed)
                .max(self.processed.load(Ordering::Relaxed));
            BackupProgress {
                phase: state.phase,
                source_bytes: state.total.map_or(bytes, |total| bytes.min(total)),
                total_bytes: state.total,
            }
        };
        (self.progress)(event);
    }
}
impl ProgressBars for BackupControl {
    fn progress(&self, kind: ProgressType, prefix: &str) -> Progress {
        if matches!(kind, ProgressType::Bytes)
            && matches!(prefix, "backing up..." | "restoring file contents...")
        {
            Progress::new(EngineProgress(self.clone()))
        } else {
            Progress::hidden()
        }
    }
}
#[derive(Debug)]
struct EngineProgress(BackupControl);
impl RusticProgress for EngineProgress {
    fn is_hidden(&self) -> bool {
        false
    }
    fn set_length(&self, length: u64) {
        self.0.state.lock().expect("backup progress lock").total = Some(length);
    }
    fn set_title(&self, _title: &str) {}
    fn inc(&self, bytes: u64) {
        self.0.processed.fetch_add(bytes, Ordering::Relaxed);
        self.0.emit(false);
    }
    fn finish(&self) {
        self.0.emit(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_callbacks_are_throttled_and_contain_only_compact_progress() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let control = BackupControl::with_cancellation(
            || false,
            move |event| sink.lock().unwrap().push(event),
        );
        control.phase(BackupPhase::Snapshot, Some(4096));
        for _ in 0..1024 {
            control.read_bytes(1);
        }
        assert_eq!(events.lock().unwrap().len(), 1);
        EngineProgress(control.clone()).finish();
        let recorded = events.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[1].source_bytes, 1024);
        assert_eq!(recorded[1].phase, BackupPhase::Snapshot);
        assert_eq!(recorded[1].total_bytes, Some(4096));
    }
}
