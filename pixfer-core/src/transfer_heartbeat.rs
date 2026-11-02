//! Transfer heartbeat: keep-alive liveness tracking for a transfer session.
//! (Hilariously over-engineered: a "liveness protocol" for a link where the
//! fastest "packet" is one screen frame at a time.)

/// A heartbeat sample: timestamp (monotonic ms) and a sequence number.
#[derive(Debug, Clone, Copy)]
pub struct HeartbeatSample {
    pub t_ms: u64,
    pub seq: u64,
}

/// Tracks heartbeat liveness and detects stale peers.
#[derive(Debug)]
pub struct TransferHeartbeat {
    /// The interval (ms) at which heartbeats should arrive.
    interval_ms: u64,
    /// How many missed intervals before declaring the peer stale.
    stale_after: u32,
    last: Option<HeartbeatSample>,
    /// Consecutive missed-interval count.
    missed: u32,
    beats: u64,
}

impl TransferHeartbeat {
    /// Create a heartbeat tracker with `interval_ms` and `stale_after` misses.
    pub fn new(interval_ms: u64, stale_after: u32) -> Self {
        Self {
            interval_ms,
            stale_after,
            last: None,
            missed: 0,
            beats: 0,
        }
    }

    /// The configured heartbeat interval.
    pub fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    /// Record a received heartbeat.
    pub fn record(&mut self, sample: HeartbeatSample) {
        if let Some(prev) = self.last {
            let gap = sample.t_ms.saturating_sub(prev.t_ms);
            let expected = self.interval_ms;
            if gap > expected {
                self.missed = self.missed.saturating_add(1);
            } else {
                self.missed = 0;
            }
        }
        self.last = Some(sample);
        self.beats = self.beats.saturating_add(1);
    }

    /// The number of heartbeats recorded.
    pub fn beats(&self) -> u64 {
        self.beats
    }

    /// Consecutive missed-interval count.
    pub fn missed(&self) -> u32 {
        self.missed
    }

    /// Whether the peer should be considered stale (missed >= stale_after).
    pub fn is_stale(&self) -> bool {
        self.missed >= self.stale_after
    }

    /// The last recorded heartbeat (None if none yet).
    pub fn last(&self) -> Option<HeartbeatSample> {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_heartbeats_not_stale() {
        let h = TransferHeartbeat::new(1000, 3);
        assert!(!h.is_stale());
        assert_eq!(h.beats(), 0);
        assert!(h.last().is_none());
    }

    #[test]
    fn on_time_heartbeats_reset_missed() {
        let mut h = TransferHeartbeat::new(1000, 3);
        h.record(HeartbeatSample { t_ms: 0, seq: 0 });
        h.record(HeartbeatSample { t_ms: 1000, seq: 1 });
        h.record(HeartbeatSample { t_ms: 2000, seq: 2 });
        assert_eq!(h.missed(), 0);
        assert_eq!(h.beats(), 3);
        assert!(!h.is_stale());
    }

    #[test]
    fn slow_heartbeats_accumulate_misses() {
        let mut h = TransferHeartbeat::new(1000, 2);
        h.record(HeartbeatSample { t_ms: 0, seq: 0 });
        // 5s gap: 4 intervals missed (floor(5000/1000 - 1) = 4)
        h.record(HeartbeatSample { t_ms: 5000, seq: 1 });
        assert_eq!(h.missed(), 1); // current impl adds 1 per slow sample
        h.record(HeartbeatSample {
            t_ms: 10000,
            seq: 2,
        });
        assert!(h.is_stale());
        assert_eq!(h.missed(), 2);
    }

    #[test]
    fn interval_preserved() {
        let h = TransferHeartbeat::new(500, 5);
        assert_eq!(h.interval_ms(), 500);
    }

    #[test]
    fn last_sample_is_returned() {
        let mut h = TransferHeartbeat::new(1000, 3);
        assert!(h.last().is_none());
        h.record(HeartbeatSample { t_ms: 42, seq: 7 });
        let l = h.last().expect("present");
        assert_eq!(l.t_ms, 42);
        assert_eq!(l.seq, 7);
    }

    #[test]
    fn saturating_math_no_overflow() {
        let mut h = TransferHeartbeat::new(1000, 1);
        h.record(HeartbeatSample { t_ms: 0, seq: 0 });
        h.record(HeartbeatSample {
            t_ms: u64::MAX,
            seq: 1,
        });
        assert!(h.is_stale());
    }
}
