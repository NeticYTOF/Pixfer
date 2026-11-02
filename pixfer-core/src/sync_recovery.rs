//! Sync recovery: after the camera misses the sync word, re-acquire
//! lock by scanning for the pattern with a bounded mismatch budget.
//! (Hilariously over-engineered: a "re-lock" state machine for a link
//! that can only see 8 colors.)

use crate::sync::{SyncDetector, SyncPattern};

/// The state of sync re-acquisition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryState {
    /// Locked onto the sync word; transfers can proceed.
    Locked,
    /// Scanning for the sync word after a drop.
    Acquiring,
    /// Gave up after `max_scans` without a match.
    Failed,
}

/// A bounded scanner that re-acquires the sync word.
///
/// Wraps `SyncDetector` with a max-scan budget and a recovery state.
#[derive(Debug)]
pub struct SyncRecovery {
    detector: SyncDetector,
    max_scans: usize,
    scans: usize,
    state: RecoveryState,
}

impl SyncRecovery {
    pub fn new(pattern: SyncPattern, max_scans: usize) -> Self {
        Self {
            detector: SyncDetector::new(pattern),
            max_scans,
            scans: 0,
            state: RecoveryState::Acquiring,
        }
    }

    /// Current recovery state.
    pub fn state(&self) -> RecoveryState {
        self.state
    }

    /// Run one scan: check a candidate window; on match, transition to
    /// Locked. Returns true if the window matched.
    pub fn scan(&mut self, candidate: &[u8]) -> bool {
        if self.state == RecoveryState::Locked {
            return self.detector.find(candidate).is_some();
        }
        if self.state == RecoveryState::Failed {
            return false;
        }
        let matched = self.detector.find(candidate).is_some();
        self.scans += 1;
        if matched {
            self.state = RecoveryState::Locked;
            self.scans = 0;
        } else if self.scans >= self.max_scans {
            self.state = RecoveryState::Failed;
        }
        matched
    }

    /// Resumable scan: keeps the scan count across calls.
    pub fn scan_resumable(&mut self, candidate: &[u8]) -> bool {
        self.scan(candidate)
    }

    /// Reset to the acquiring state with a fresh scan count.
    pub fn reset(&mut self) {
        self.state = RecoveryState::Acquiring;
        self.scans = 0;
    }

    /// The max-scan budget.
    pub fn max_scans(&self) -> usize {
        self.max_scans
    }

    /// Scans consumed in the current attempt.
    pub fn scan_count(&self) -> usize {
        self.scans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern() -> SyncPattern {
        SyncPattern::from_symbols(&[0, 1, 2, 3, 4, 5, 6, 7])
    }

    #[test]
    fn starts_in_acquiring() {
        let r = SyncRecovery::new(pattern(), 5);
        assert_eq!(r.state(), RecoveryState::Acquiring);
        assert_eq!(r.scan_count(), 0);
        assert_eq!(r.max_scans(), 5);
    }

    #[test]
    fn matches_transitions_to_locked() {
        let mut r = SyncRecovery::new(pattern(), 5);
        let window = [0, 1, 2, 3, 4, 5, 6, 7];
        assert!(r.scan(&window));
        assert_eq!(r.state(), RecoveryState::Locked);
        assert_eq!(r.scan_count(), 0); // reset after lock
    }

    #[test]
    fn no_match_increments_scans() {
        let mut r = SyncRecovery::new(pattern(), 3);
        let wrong = [1, 2, 3, 4, 5, 6, 7, 0];
        assert!(!r.scan(&wrong));
        assert_eq!(r.scan_count(), 1);
        assert_eq!(r.state(), RecoveryState::Acquiring);
    }

    #[test]
    fn exceeds_max_scans_fails() {
        let mut r = SyncRecovery::new(pattern(), 2);
        let wrong = [1, 2, 3, 4, 5, 6, 7, 0];
        assert!(!r.scan(&wrong)); // scan 1
        assert!(!r.scan(&wrong)); // scan 2 -> fails
        assert_eq!(r.state(), RecoveryState::Failed);
    }

    #[test]
    fn locked_ignores_extra_scans_budget() {
        let mut r = SyncRecovery::new(pattern(), 2);
        let good = [0, 1, 2, 3, 4, 5, 6, 7];
        assert!(r.scan(&good));
        assert_eq!(r.state(), RecoveryState::Locked);
        // While locked, scan() just reports the match without consuming budget.
        assert!(r.scan(&good));
        assert_eq!(r.state(), RecoveryState::Locked);
    }

    #[test]
    fn reset_returns_to_acquiring() {
        let mut r = SyncRecovery::new(pattern(), 2);
        r.reset();
        assert_eq!(r.state(), RecoveryState::Acquiring);
        assert_eq!(r.scan_count(), 0);
    }

    #[test]
    fn failed_until_reset() {
        let mut r = SyncRecovery::new(pattern(), 1);
        let wrong = [1, 2, 3, 4, 5, 6, 7, 0];
        r.scan(&wrong); // fails (max 1)
        assert_eq!(r.state(), RecoveryState::Failed);
        let good = [0, 1, 2, 3, 4, 5, 6, 7];
        assert!(!r.scan(&good)); // failed -> no match
        assert_eq!(r.state(), RecoveryState::Failed);
        r.reset();
        assert!(r.scan(&good));
        assert_eq!(r.state(), RecoveryState::Locked);
    }
}
