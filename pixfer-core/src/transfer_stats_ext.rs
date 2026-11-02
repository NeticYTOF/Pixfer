//! Extended transfer stats: a richer per-transfer statistics record with
//! throughput, retry, and FEC failure tracking. (Hilariously over-
//! engineered: a "stats dashboard" for a transfer that physically moves a
//! few KB/s over a phone screen.)

/// An extended transfer statistics record.
#[derive(Debug, Default)]
pub struct TransferStatsExt {
    /// Total bytes transferred.
    pub bytes: u64,
    /// Elapsed time (ms).
    pub elapsed_ms: u64,
    /// Number of retries.
    pub retries: u32,
    /// Number of FEC (Reed-Solomon) correction events.
    pub fec_corrections: u32,
    /// Number of failed frames.
    pub failed_frames: u32,
    /// Number of successfully delivered chunks.
    pub delivered_chunks: u32,
    /// Total chunks in the transfer.
    pub total_chunks: u32,
}

impl TransferStatsExt {
    /// The measured throughput (bytes/sec), 0 if no time elapsed.
    pub fn throughput_bps(&self) -> f64 {
        if self.elapsed_ms == 0 {
            0.0
        } else {
            (self.bytes as f64) / (self.elapsed_ms as f64 / 1000.0)
        }
    }

    /// The fraction of chunks delivered (0.0 if no total).
    pub fn delivered_fraction(&self) -> f64 {
        if self.total_chunks == 0 {
            0.0
        } else {
            self.delivered_chunks as f64 / self.total_chunks as f64
        }
    }

    /// The retry rate (retries per delivered chunk), 0 if no deliveries.
    pub fn retry_rate(&self) -> f64 {
        if self.delivered_chunks == 0 {
            0.0
        } else {
            self.retries as f64 / self.delivered_chunks as f64
        }
    }

    /// Whether the transfer completed successfully (all chunks delivered,
    /// no failed frames).
    pub fn is_success(&self) -> bool {
        self.delivered_chunks == self.total_chunks && self.failed_frames == 0
    }

    /// A JSON export of the stats.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"bytes\":{},\"elapsed_ms\":{},\"retries\":{},\"fec_corrections\":{},\"failed_frames\":{},\"delivered_chunks\":{},\"total_chunks\":{}}}",
            self.bytes,
            self.elapsed_ms,
            self.retries,
            self.fec_corrections,
            self.failed_frames,
            self.delivered_chunks,
            self.total_chunks
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TransferStatsExt {
        TransferStatsExt {
            bytes: 892,
            elapsed_ms: 132,
            retries: 3,
            fec_corrections: 1,
            failed_frames: 0,
            delivered_chunks: 4,
            total_chunks: 4,
        }
    }

    #[test]
    fn throughput_math() {
        let s = sample();
        // 892 bytes in 0.132 s = 6757.5 B/s
        assert!(
            (s.throughput_bps() - 892.0 / 0.132).abs() < 1e-3,
            "got {}",
            s.throughput_bps()
        );
    }

    #[test]
    fn zero_elapsed_zero_throughput() {
        let mut s = TransferStatsExt::default();
        s.bytes = 100;
        assert_eq!(s.throughput_bps(), 0.0);
    }

    #[test]
    fn delivered_fraction_full() {
        let s = sample();
        assert!((s.delivered_fraction() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn retry_rate_math() {
        let s = sample();
        // 3 retries / 4 delivered = 0.75
        assert!((s.retry_rate() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn success_when_all_delivered_no_failures() {
        let s = sample();
        assert!(s.is_success());
        let mut s2 = sample();
        s2.failed_frames = 2;
        assert!(!s2.is_success());
    }

    #[test]
    fn json_export_shape() {
        let s = sample();
        let j = s.to_json();
        assert!(j.contains("\"bytes\":892"));
        assert!(j.contains("\"total_chunks\":4"));
    }
}
