// SPDX-License-Identifier: MIT
//! Small workflow-local admission controller; socket/origin budgets remain broker-owned.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

pub(crate) struct Admission {
    state: Mutex<State>,
    changed: Notify,
}
struct State {
    adaptive: bool,
    ceiling: usize,
    limit: usize,
    active: usize,
    started: Instant,
    bytes: u64,
    previous_rate: f64,
    slow_windows: u8,
    cooldown: Instant,
}
pub(crate) struct Lease(Arc<Admission>);
impl Drop for Lease {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().expect("download admission");
        state.active -= 1;
        drop(state);
        self.0.changed.notify_waiters();
    }
}
impl Admission {
    pub(crate) fn new(ceiling: usize, adaptive: bool) -> Arc<Self> {
        let now = Instant::now();
        Arc::new(Self {
            state: Mutex::new(State {
                adaptive,
                ceiling,
                limit: if adaptive { ceiling.min(2) } else { ceiling },
                active: 0,
                started: now,
                bytes: 0,
                previous_rate: 0.,
                slow_windows: 0,
                cooldown: now,
            }),
            changed: Notify::new(),
        })
    }
    pub(crate) fn limit(&self) -> usize {
        self.state.lock().expect("download admission").limit
    }
    pub(crate) async fn acquire(self: &Arc<Self>) -> Lease {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let mut state = self.state.lock().expect("download admission");
                if state.active < state.limit {
                    if state.active == 0 {
                        state.started = Instant::now();
                        state.bytes = 0;
                    }
                    state.active += 1;
                    return Lease(self.clone());
                }
            }
            changed.await;
        }
    }
    pub(crate) fn observe(&self, bytes: u64, congestion: bool) {
        let mut state = self.state.lock().expect("download admission");
        if !state.adaptive {
            return;
        }
        let now = Instant::now();
        if congestion {
            if now >= state.cooldown {
                state.limit = (state.limit / 2).max(1);
                state.cooldown = now + Duration::from_secs(1);
                state.started = now;
                state.bytes = 0;
                state.previous_rate = 0.;
                state.slow_windows = 0;
            }
        } else if now >= state.cooldown && bytes > 0 {
            state.bytes = state.bytes.saturating_add(bytes);
            let elapsed = now.duration_since(state.started);
            if elapsed >= Duration::from_millis(250) && state.active >= state.limit {
                let rate = state.bytes as f64 / elapsed.as_secs_f64();
                if state.previous_rate == 0. || rate >= state.previous_rate * 0.95 {
                    state.limit = (state.limit + 1).min(state.ceiling);
                    state.slow_windows = 0;
                } else if rate < state.previous_rate * 0.8 {
                    state.slow_windows += 1;
                    if state.slow_windows >= 2 {
                        state.limit = state.limit.saturating_sub(1).max(1);
                        state.slow_windows = 0;
                    }
                } else {
                    state.slow_windows = 0;
                }
                state.previous_rate = if state.previous_rate == 0. {
                    rate
                } else {
                    state.previous_rate * 0.5 + rate * 0.5
                };
                state.started = now;
                state.bytes = 0;
            }
        }
        drop(state);
        self.changed.notify_waiters();
    }
}
