//! Extended transfer progress: a fine-grained progress tracker with
//! per-chunk granularity and ETA estimation. (Hilariously over-engineered: a
//! "progress dashboard" for a transfer that takes minutes to move a few
//! KB.)

use crate::transfer_plan::TransferPlan;

/// A fine-grained progress snapshot.
#[derive(Debug, Clone, Copy)]
pub struct ProgressSnapshotExt {
    /// The number of delivered chunks.
    pub delivered: u32,
    /// The total chunk count.
    pub total: u32,
    /// The elapsed time (ms).
    pub elapsed_ms: u64,
}

/// Tracks transfer progress at chunk granularity with ETA.
#[derive(Debug, Default)]
pub struct TransferProgressExt {
    delivered: u32,
    total: u32,
    elapsed_ms: u64,
}

impl TransferProgressExt {
    pub fn new(total_chunks: u32) -> Self {
        Self {
            delivered: 0,
            total: total_chunks,
            elapsed_ms: 0,
        }
    }

    /// Mark a chunk as delivered.
    pub fn mark_delivered(&mut self) {
        if self.delivered < self.total {
            self.delivered += 1;
        }
    }

    /// The current progress fraction (0.0..=1.0).
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.delivered as f64 / self.total as f64
        }
    }

    /// The current progress snapshot.
    pub fn snapshot(&self) -> ProgressSnapshotExt {
        ProgressSnapshotExt {
            delivered: self.delivered,
            total: self.total,
            elapsed_ms: self.elapsed_ms,
        }
    }

    /// The estimated time remaining (ms), based on average per-chunk time.
    pub fn eta_ms(&self) -> u64 {
        if self.delivered == 0 || self.total == 0 {
            0
        } else {
            let per_chunk = self.elapsed_ms / self.delivered as u64;
            let remaining = self.total - self.delivered;
            per_chunk * remaining as u64
        }
    }

    /// Whether the transfer is complete.
    pub fn is_complete(&self) -> bool {
        self.delivered >= self.total
    }

    /// The total chunk count.
    pub fn total(&self) -> u32 {
        self.total
    }

    /// The delivered chunk count.
    pub fn delivered_count(&self) -> u32 {
        self.delivered
    }

    /// Set the elapsed time (ms).
    pub fn set_elapsed_ms(&mut self, ms: u64) {
        self.elapsed_ms = ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero() {
        let p = TransferProgressExt::new(10);
        assert_eq!(p.delivered_count(), 0);
        assert_eq!(p.total(), 10);
        assert!(!p.is_complete());
        assert_eq!(p.fraction(), 0.0);
    }

    #[test]
    fn mark_delivered_increments() {
        let mut p = TransferProgressExt::new(3);
        p.mark_delivered();
        p.mark_delivered();
        p.mark_delivered();
        assert_eq!(p.delivered_count(), 3);
        assert!(p.is_complete());
        assert!((p.fraction() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn mark_delivered_clamps_at_total() {
        let mut p = TransferProgressExt::new(2);
        p.mark_delivered();
        p.mark_delivered();
        p.mark_delivered(); // clamped
        assert_eq!(p.delivered_count(), 2);
    }

    #[test]
    fn snapshot_shape() {
        let mut p = TransferProgressExt::new(5);
        p.mark_delivered();
        p.set_elapsed_ms(500);
        let s = p.snapshot();
        assert_eq!(s.delivered, 1);
        assert_eq!(s.total, 5);
        assert_eq!(s.elapsed_ms, 500);
    }

    #[test]
    fn eta_estimation() {
        let mut p = TransferProgressExt::new(10);
        for _ in 0..5 {
            p.mark_delivered();
        }
        p.set_elapsed_ms(1000);
        // 5 chunks in 1000 ms -> 200 ms/chunk, 5 remaining -> 1000 ms ETA.
        assert_eq!(p.eta_ms(), 1000);
    }

    #[test]
    fn eta_zero_when_no_progress() {
        let mut p = TransferProgressExt::new(10);
        p.set_elapsed_ms(500);
        assert_eq!(p.eta_ms(), 0);
    }
}
