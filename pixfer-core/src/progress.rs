//! Transfer progress: a progress tracker + ETA estimator that the tx/rx UIs
//! poll. (Hilariously over-engineered: a progress bar for a link that has
//! no standard progress events.)

/// A snapshot of transfer progress.
#[derive(Debug, Clone)]
pub struct ProgressSnapshot {
    /// Number of chunks received so far.
    pub received: u64,
    /// Total chunks in the transfer.
    pub total: u64,
    /// Bytes received so far.
    pub bytes_received: u64,
    /// Total bytes.
    pub bytes_total: u64,
}

impl ProgressSnapshot {
    /// Fraction complete in [0.0, 1.0].
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.received as f64) / (self.total as f64)
    }

    /// Percent complete in [0, 100].
    pub fn percent(&self) -> f64 {
        self.fraction() * 100.0
    }

    /// Remaining chunks.
    pub fn remaining(&self) -> u64 {
        self.total.saturating_sub(self.received)
    }
}

/// Tracks received chunks/bytes and computes ETA from an observed rate.
#[derive(Debug, Clone)]
pub struct ProgressTracker {
    received: u64,
    bytes_received: u64,
    total: u64,
    bytes_total: u64,
    /// Observed bytes/sec (0 if unknown).
    rate_bps: f64,
}

impl ProgressTracker {
    pub fn new(total_chunks: u64, bytes_total: u64) -> Self {
        Self {
            received: 0,
            bytes_received: 0,
            total: total_chunks,
            bytes_total,
            rate_bps: 0.0,
        }
    }

    pub fn mark_received(&mut self, chunks: u64, bytes: u64) {
        self.received = self.received.saturating_add(chunks);
        self.bytes_received = self.bytes_received.saturating_add(bytes);
    }

    pub fn set_rate(&mut self, bps: f64) {
        self.rate_bps = bps.max(0.0);
    }

    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            received: self.received,
            total: self.total,
            bytes_received: self.bytes_received,
            bytes_total: self.bytes_total,
        }
    }

    /// Estimated seconds remaining at the observed rate (None if rate unknown).
    pub fn eta_secs(&self) -> Option<f64> {
        if self.rate_bps <= 0.0 {
            return None;
        }
        let remaining_bytes = self.bytes_total.saturating_sub(self.bytes_received);
        Some((remaining_bytes as f64) / self.rate_bps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_and_percent() {
        let s = ProgressSnapshot {
            received: 3,
            total: 10,
            bytes_received: 3000,
            bytes_total: 10000,
        };
        assert!((s.fraction() - 0.3).abs() < 1e-9);
        assert!((s.percent() - 30.0).abs() < 1e-9);
        assert_eq!(s.remaining(), 7);
    }

    #[test]
    fn zero_total_fractions() {
        let s = ProgressSnapshot {
            received: 0,
            total: 0,
            bytes_received: 0,
            bytes_total: 0,
        };
        assert_eq!(s.fraction(), 0.0);
        assert_eq!(s.percent(), 0.0);
    }

    #[test]
    fn tracker_marks_received() {
        let mut t = ProgressTracker::new(10, 100_000);
        t.mark_received(3, 30_000);
        let s = t.snapshot();
        assert_eq!(s.received, 3);
        assert_eq!(s.bytes_received, 30_000);
        assert_eq!(s.remaining(), 7);
    }

    #[test]
    fn eta_from_rate() {
        let mut t = ProgressTracker::new(10, 100_000);
        t.mark_received(2, 20_000);
        t.set_rate(2_000_000.0);
        let eta = t.eta_secs();
        // Remaining 80_000 bytes at 2 MB/s = 0.04 s.
        assert!(eta.is_some());
        assert!((eta.unwrap() - 0.04).abs() < 1e-6);
    }

    #[test]
    fn eta_unknown_when_no_rate() {
        let mut t = ProgressTracker::new(10, 100_000);
        t.mark_received(2, 20_000);
        assert!(t.eta_secs().is_none());
    }

    #[test]
    fn saturation_on_overflow() {
        let mut t = ProgressTracker::new(u64::MAX, u64::MAX);
        t.mark_received(1, 1);
        assert_eq!(t.snapshot().remaining(), u64::MAX - 1);
    }
}
