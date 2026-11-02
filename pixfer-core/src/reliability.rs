//! Reliability metrics: goodput, loss rate, and a link-quality score that
//! blends them. (Hilariously over-engineered: a "reliability index" for a
//! link whose only transport is a flashing screen.)

/// A reliability snapshot for one transfer.
#[derive(Debug, Clone)]
pub struct ReliabilitySnapshot {
    pub total_chunks: u64,
    pub delivered: u64,
    pub retries: u64,
    pub naks: u64,
}

impl ReliabilitySnapshot {
    /// Fraction of chunks delivered (0..=1).
    pub fn goodput_fraction(&self) -> f64 {
        if self.total_chunks == 0 {
            return 0.0;
        }
        (self.delivered as f64) / (self.total_chunks as f64)
    }

    /// Fraction of chunks that needed a retry.
    pub fn retry_fraction(&self) -> f64 {
        if self.total_chunks == 0 {
            return 0.0;
        }
        (self.retries as f64) / (self.total_chunks as f64)
    }

    /// A 0..=1 link-quality score: goodput up, retries/NAKs down.
    pub fn quality_score(&self) -> f64 {
        let goodput = self.goodput_fraction();
        let penalty = self.retry_fraction() * 0.5 + (self.naks as f64) * 0.01;
        (goodput - penalty).clamp(0.0, 1.0)
    }
}

/// Accumulates per-chunk outcomes and produces reliability snapshots.
#[derive(Debug, Default)]
pub struct ReliabilityTracker {
    total_chunks: u64,
    delivered: u64,
    retries: u64,
    naks: u64,
}

impl ReliabilityTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_delivered(&mut self, total_chunks: u64) {
        self.total_chunks = total_chunks;
        self.delivered = total_chunks;
    }

    pub fn mark_retry(&mut self) {
        self.retries = self.retries.saturating_add(1);
    }

    pub fn mark_nak(&mut self) {
        self.naks = self.naks.saturating_add(1);
    }

    pub fn snapshot(&self) -> ReliabilitySnapshot {
        ReliabilitySnapshot {
            total_chunks: self.total_chunks,
            delivered: self.delivered,
            retries: self.retries,
            naks: self.naks,
        }
    }

    pub fn quality_score(&self) -> f64 {
        self.snapshot().quality_score()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_goodput() {
        let s = ReliabilitySnapshot {
            total_chunks: 10,
            delivered: 10,
            retries: 0,
            naks: 0,
        };
        assert!((s.goodput_fraction() - 1.0).abs() < 1e-9);
        assert!((s.quality_score() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn retries_lower_quality() {
        let s = ReliabilitySnapshot {
            total_chunks: 10,
            delivered: 8,
            retries: 4,
            naks: 2,
        };
        let score = s.quality_score();
        assert!(score < 1.0);
        assert!(score > 0.0);
    }

    #[test]
    fn zero_total_is_zero() {
        let s = ReliabilitySnapshot {
            total_chunks: 0,
            delivered: 0,
            retries: 0,
            naks: 0,
        };
        assert_eq!(s.goodput_fraction(), 0.0);
        assert_eq!(s.quality_score(), 0.0);
    }

    #[test]
    fn tracker_accumulates() {
        let mut t = ReliabilityTracker::new();
        t.mark_delivered(10);
        t.mark_retry();
        t.mark_retry();
        t.mark_nak();
        let s = t.snapshot();
        assert_eq!(s.retries, 2);
        assert_eq!(s.naks, 1);
        assert!(t.quality_score() < 1.0);
    }

    #[test]
    fn quality_clamped() {
        let s = ReliabilitySnapshot {
            total_chunks: 10,
            delivered: 10,
            retries: 8,
            naks: 40,
        };
        // Heavy retries/NAKs push the score toward 0.
        assert!((s.quality_score() >= 0.0) && (s.quality_score() <= 1.0));
    }
}
