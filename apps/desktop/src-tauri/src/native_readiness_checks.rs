// SPDX-License-Identifier: MIT
use super::*;
use std::{collections::VecDeque, sync::OnceLock};
#[derive(Default)]
struct Checks {
    active: HashMap<String, cobble_core::OperationControl>,
    cancelled: VecDeque<String>,
}
fn state() -> &'static Mutex<Checks> {
    static STATE: OnceLock<Mutex<Checks>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}
pub(super) struct Guard {
    request_id: String,
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut state) = state().lock() {
            state.active.remove(&self.request_id);
        }
    }
}
pub(super) fn begin(
    request_id: Option<String>,
) -> Result<(cobble_core::OperationControl, Guard), String> {
    let id = request_id.unwrap_or(next_pack_id()?);
    if id.is_empty() || id.len() > 128 {
        return Err("READINESS_REQUEST_INVALID".into());
    }
    let mut state = state().lock().map_err(|_| "READINESS_STATE_UNAVAILABLE")?;
    if state.cancelled.contains(&id) {
        return Err("READINESS_CANCELLED".into());
    }
    if state.active.len() >= 8 || state.active.contains_key(&id) {
        return Err("READINESS_CHECK_BUSY".into());
    }
    let control = cobble_core::OperationControl::new(Arc::new(|_| {}));
    state.active.insert(id.clone(), control.clone());
    Ok((control, Guard { request_id: id }))
}
pub(super) fn cancel(request_id: String) -> Result<(), String> {
    if request_id.is_empty() || request_id.len() > 128 {
        return Err("READINESS_REQUEST_INVALID".into());
    }
    let mut state = state().lock().map_err(|_| "READINESS_STATE_UNAVAILABLE")?;
    if let Some(control) = state.active.get(&request_id) {
        control.cancel();
    }
    if !state.cancelled.contains(&request_id) {
        state.cancelled.push_back(request_id);
        if state.cancelled.len() > 32 {
            state.cancelled.pop_front();
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readiness_cancel_before_dispatch_and_during_hashing_are_bounded_and_remove_guard() {
        let before = next_pack_id().unwrap();
        cancel(before.clone()).unwrap();
        assert!(begin(Some(before)).is_err());
        let id = next_pack_id().unwrap();
        let (control, guard) = begin(Some(id.clone())).unwrap();
        cancel(id.clone()).unwrap();
        assert!(control.is_cancelled());
        drop(guard);
        assert!(!state().lock().unwrap().active.contains_key(&id));
    }
}
