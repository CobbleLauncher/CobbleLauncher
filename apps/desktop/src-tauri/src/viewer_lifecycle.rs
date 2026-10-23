// SPDX-License-Identifier: MIT
//! One small gate serializes Viewer publication against native revocation.
use std::sync::{
    Arc, Mutex, MutexGuard, Weak,
    atomic::{AtomicBool, Ordering},
};
#[derive(Default)]
pub(super) struct State {
    pending: Vec<(String, Weak<AtomicBool>)>,
}
static STATE: Mutex<State> = Mutex::new(State {
    pending: Vec::new(),
});
pub(super) fn lock() -> Result<MutexGuard<'static, State>, String> {
    STATE
        .lock()
        .map_err(|_| "Viewer lifecycle unavailable".into())
}
impl State {
    pub(super) fn track(&mut self, id: &str, cancel: &Arc<AtomicBool>) -> Result<(), String> {
        self.pending.retain(|(_, flag)| flag.strong_count() > 0);
        if self.pending.len() >= 32 {
            return Err("Too many Viewer sessions".into());
        }
        self.pending.push((id.into(), Arc::downgrade(cancel)));
        Ok(())
    }
    pub(super) fn revoke(&mut self, id: &str) {
        self.pending.retain(|(plugin, flag)| {
            if plugin == id {
                if let Some(flag) = flag.upgrade() {
                    flag.store(true, Ordering::Release);
                }
                false
            } else {
                flag.strong_count() > 0
            }
        });
    }
}
pub(super) fn publish<T>(
    cancel: &AtomicBool,
    validate: impl FnOnce() -> Result<(), String>,
    publish: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _gate = lock()?;
    if cancel.load(Ordering::Acquire) {
        return Err("Viewer request cancelled".into());
    }
    validate()?;
    publish()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disable_during_handoff_cancels_pending_and_prevents_late_publication() {
        let cancel = Arc::new(AtomicBool::new(false));
        lock().unwrap().track("handoff-fixture", &cancel).unwrap();
        let revocation = std::thread::spawn(|| {
            lock().unwrap().revoke("handoff-fixture");
        });
        revocation.join().unwrap();
        let published = AtomicBool::new(false);
        assert!(
            publish(
                &cancel,
                || Ok(()),
                || {
                    published.store(true, Ordering::Release);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!published.load(Ordering::Acquire));
    }
    #[test]
    fn source_provider_revocation_does_not_cancel_completed_independent_viewer() {
        let cancel = Arc::new(AtomicBool::new(false));
        lock()
            .unwrap()
            .track("independent-viewer-fixture", &cancel)
            .unwrap();
        publish(&cancel, || Ok(()), || Ok(())).unwrap();
        lock().unwrap().revoke("source-provider-fixture");
        assert!(!cancel.load(Ordering::Acquire));
        lock().unwrap().revoke("independent-viewer-fixture");
        assert!(cancel.load(Ordering::Acquire));
    }
}
