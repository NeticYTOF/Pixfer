//! Transfer watchdog: monitors transfer liveness and raises a timeout
//! event when no progress is made. (Hilariously over-engineered: a
//! "watchdog" for a light link that physically moves a few KB/s.)

use crate::timing::{Clock, SystemClock};

/// The state of the watchdog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogState {
    /// No transfer in progress.
    Idle,
    /// Transfer in progress, within budget.
    Active,
    /// No progress for longer than the timeout.
    TimedOut,
}

/// A watchdog that detects stalled transfers.
///
/// Given a timeout and a clock, it tracks the last-progress timestamp
/// and reports the current state.
#[derive(Debug)]
pub struct TransferWatchdog {
    timeout_ms: u64,
    last_progress_ms: u64,
    active: bool,
    state: WatchdogState,
}

impl TransferWatchdog {
    /// Create with a timeout in ms.
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout_ms,
            last_progress_ms: 0,
            active: false,
            state: WatchdogState::Idle,
        }
    }

    /// Mark the start of a transfer.
    pub fn start(&mut self, clock: &dyn Clock) {
        self.active = true;
        self.last_progress_ms = clock.now_ms();
        self.state = WatchdogState::Active;
    }

    /// Record progress (a chunk delivered, an ACK received, etc.).
    pub fn record_progress(&mut self, clock: &dyn Clock) {
        if self.active {
            self.last_progress_ms = clock.now_ms();
            self.state = WatchdogState::Active;
        }
    }

    /// Poll the watchdog: returns true if the transfer has timed out.
    pub fn check(&mut self, clock: &dyn Clock) -> bool {
        if !self.active {
            self.state = WatchdogState::Idle;
            return false;
        }
        let now = clock.now_ms();
        if now.saturating_sub(self.last_progress_ms) >= self.timeout_ms {
            self.state = WatchdogState::TimedOut;
            return true;
        }
        self.state = WatchdogState::Active;
        false
    }

    /// End the transfer (success or abort).
    pub fn stop(&mut self) {
        self.active = false;
        self.state = WatchdogState::Idle;
    }

    /// The current state.
    pub fn state(&self) -> WatchdogState {
        self.state
    }

    /// The configured timeout (ms).
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockClock {
        now_ms: u64,
    }
    impl Clock for MockClock {
        fn now_ms(&self) -> u64 {
            self.now_ms
        }
    }

    #[test]
    fn idle_until_start() {
        let mut w = TransferWatchdog::new(1000);
        assert_eq!(w.state(), WatchdogState::Idle);
        let clock = MockClock { now_ms: 0 };
        w.start(&clock);
        assert_eq!(w.state(), WatchdogState::Active);
    }

    #[test]
    fn times_out_after_budget() {
        let mut w = TransferWatchdog::new(500);
        let mut clock = MockClock { now_ms: 0 };
        w.start(&clock);
        clock.now_ms = 400;
        assert!(!w.check(&clock)); // 400 < 500
        clock.now_ms = 600;
        assert!(w.check(&clock)); // 600 - 0 >= 500
        assert_eq!(w.state(), WatchdogState::TimedOut);
    }

    #[test]
    fn progress_resets_timer() {
        let mut w = TransferWatchdog::new(500);
        let mut clock = MockClock { now_ms: 0 };
        w.start(&clock);
        clock.now_ms = 300;
        w.record_progress(&clock);
        clock.now_ms = 700;
        // Last progress at 300, so 700 - 300 = 400 < 500 -> not timed out.
        assert!(!w.check(&clock));
        clock.now_ms = 850;
        assert!(w.check(&clock));
    }

    #[test]
    fn stop_returns_to_idle() {
        let mut w = TransferWatchdog::new(1000);
        let clock = MockClock { now_ms: 0 };
        w.start(&clock);
        w.stop();
        assert_eq!(w.state(), WatchdogState::Idle);
    }

    #[test]
    fn timeout_is_preserved() {
        let w = TransferWatchdog::new(2500);
        assert_eq!(w.timeout_ms(), 2500);
    }

    #[test]
    fn check_when_idle_is_false() {
        let mut w = TransferWatchdog::new(100);
        let clock = MockClock { now_ms: 10_000 };
        assert!(!w.check(&clock));
        assert_eq!(w.state(), WatchdogState::Idle);
    }
}
