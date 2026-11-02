//! Extended transfer watchdog: a staler watchdog with configurable grace
//! periods and stall detection. (Hilariously over-engineered: a "watchdog"
//! for a transfer that can stall waiting for an ACK frame on the peer's
//! screen.)

use crate::transfer_watchdog::WatchdogState;

/// An extended watchdog with grace periods and stall detection.
#[derive(Debug)]
pub struct TransferWatchdogExt {
    /// The base timeout (ms).
    timeout_ms: u64,
    /// Extra grace (ms) added to the timeout.
    grace_ms: u64,
    /// The number of times the watchdog has fired.
    fired: u32,
    /// Whether the transfer is considered stalled (no progress beyond the
    /// grace period).
    stalled: bool,
    state: WatchdogState,
}

impl TransferWatchdogExt {
    pub fn new(timeout_ms: u64, grace_ms: u64) -> Self {
        Self {
            timeout_ms,
            grace_ms,
            fired: 0,
            stalled: false,
            state: WatchdogState::Active,
        }
    }

    /// The effective timeout including grace (ms).
    pub fn effective_timeout_ms(&self) -> u64 {
        self.timeout_ms.saturating_add(self.grace_ms)
    }

    /// The number of times the watchdog has fired.
    pub fn fire_count(&self) -> u32 {
        self.fired
    }

    /// Whether the transfer is stalled.
    pub fn is_stalled(&self) -> bool {
        self.stalled
    }

    /// The current watchdog state.
    pub fn state(&self) -> WatchdogState {
        self.state
    }

    /// Mark progress (resets the stall timer).
    pub fn note_progress(&mut self) {
        self.stalled = false;
        self.state = WatchdogState::Active;
    }

    /// Mark a stall (no progress beyond the grace period).
    pub fn note_stall(&mut self) {
        self.stalled = true;
        self.state = WatchdogState::TimedOut;
        self.fired = self.fired.saturating_add(1);
    }

    /// Reset the watchdog.
    pub fn reset(&mut self) {
        self.fired = 0;
        self.stalled = false;
        self.state = WatchdogState::Active;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_timeout_includes_grace() {
        let w = TransferWatchdogExt::new(1000, 500);
        assert_eq!(w.effective_timeout_ms(), 1500);
    }

    #[test]
    fn starts_running_not_stalled() {
        let w = TransferWatchdogExt::new(1000, 0);
        assert_eq!(w.state(), WatchdogState::Active);
        assert!(!w.is_stalled());
        assert_eq!(w.fire_count(), 0);
    }

    #[test]
    fn progress_resets_stall() {
        let mut w = TransferWatchdogExt::new(1000, 0);
        w.note_stall();
        assert!(w.is_stalled());
        w.note_progress();
        assert!(!w.is_stalled());
        assert_eq!(w.state(), WatchdogState::Active);
    }

    #[test]
    fn stall_increments_fire_count() {
        let mut w = TransferWatchdogExt::new(1000, 0);
        w.note_stall();
        w.note_stall();
        assert_eq!(w.fire_count(), 2);
        assert_eq!(w.state(), WatchdogState::TimedOut);
    }

    #[test]
    fn reset_clears_state() {
        let mut w = TransferWatchdogExt::new(1000, 100);
        w.note_stall();
        w.reset();
        assert_eq!(w.fire_count(), 0);
        assert!(!w.is_stalled());
        assert_eq!(w.state(), WatchdogState::Active);
        assert_eq!(w.effective_timeout_ms(), 1100);
    }
}
