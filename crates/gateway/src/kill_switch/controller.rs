use crate::types::KillSwitchState;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

const STATE_OPERATIONAL: u8 = 0;
const STATE_DRAIN: u8 = 1;
const STATE_HALTED: u8 = 2;

#[derive(Clone)]
pub struct KillSwitchController {
    state: Arc<AtomicU8>,
    consecutive_criticals: Arc<AtomicU8>,
    auto_drain_threshold: u32,
    auto_halt_on_credential_scrub: bool,
}

impl KillSwitchController {
    pub fn new(auto_drain_threshold: u32, auto_halt_on_credential_scrub: bool) -> Self {
        Self {
            state: Arc::new(AtomicU8::new(STATE_OPERATIONAL)),
            consecutive_criticals: Arc::new(AtomicU8::new(0)),
            auto_drain_threshold,
            auto_halt_on_credential_scrub,
        }
    }

    pub fn state(&self) -> KillSwitchState {
        match self.state.load(Ordering::SeqCst) {
            STATE_DRAIN => KillSwitchState::Drain,
            STATE_HALTED => KillSwitchState::Halted,
            _ => KillSwitchState::Operational,
        }
    }

    pub fn is_operational(&self) -> bool {
        self.state.load(Ordering::SeqCst) == STATE_OPERATIONAL
    }

    pub fn trigger_drain(&self) {
        let prev = self.state.swap(STATE_DRAIN, Ordering::SeqCst);
        if prev == STATE_OPERATIONAL {
            tracing::warn!("kill switch: transitioning to DRAIN");
        }
    }

    pub fn trigger_halt(&self) {
        self.state.store(STATE_HALTED, Ordering::SeqCst);
        tracing::error!("kill switch: transitioning to HALTED");
    }

    pub fn resume(&self) -> bool {
        let prev = self.state.swap(STATE_OPERATIONAL, Ordering::SeqCst);
        self.consecutive_criticals.store(0, Ordering::SeqCst);
        if prev != STATE_OPERATIONAL {
            tracing::info!("kill switch: resumed to OPERATIONAL");
            true
        } else {
            false
        }
    }

    /// Notify of a critical alert. May auto-drain if threshold exceeded.
    pub fn notify_critical(&self) -> bool {
        let count = self.consecutive_criticals.fetch_add(1, Ordering::SeqCst) + 1;
        if count as u32 >= self.auto_drain_threshold && self.is_operational() {
            tracing::warn!(
                "auto-drain triggered: {count} consecutive criticals >= threshold {}",
                self.auto_drain_threshold
            );
            self.trigger_drain();
            return true;
        }
        false
    }

    /// Reset critical counter (e.g., on successful request).
    pub fn reset_criticals(&self) {
        self.consecutive_criticals.store(0, Ordering::SeqCst);
    }

    /// Notify of credential scrub detection. Immediate halt if configured.
    pub fn notify_credential_scrub(&self) -> bool {
        if self.auto_halt_on_credential_scrub {
            tracing::error!("auto-HALT: credential pattern detected in outbound response");
            self.trigger_halt();
            return true;
        }
        false
    }
}
