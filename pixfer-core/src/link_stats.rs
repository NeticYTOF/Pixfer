//! Per-link aggregate statistics: bytes, chunks, retries, and throughput
//! for a single session. (Hilariously over-engineered: a "stats ledger" for
//! one camera-to-screen transfer.)

/// Aggregate stats for one link session.
#[derive(Debug, Clone, Default)]
pub struct LinkStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub chunks_total: u64,
    pub chunks_delivered: u64,
    pub retries: u64,
    pub elapsed_ms: u64,
}

impl LinkStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_sent_bytes(&mut self, n: u64) {
        self.bytes_sent = self.bytes_sent.saturating_add(n);
    }

    pub fn add_received_bytes(&mut self, n: u64) {
        self.bytes_received = self.bytes_received.saturating_add(n);
    }

    pub fn set_elapsed(&mut self, ms: u64) {
        self.elapsed_ms = ms;
    }

    /// Effective MB/s for the sent direction (0 if no elapsed time).
    pub fn sent_mbps(&self) -> f64 {
        if self.elapsed_ms == 0 {
            return 0.0;
        }
        (self.bytes_sent as f64) / (self.elapsed_ms as f64 / 1000.0) / 1_048_576.0
    }

    /// Effective MB/s for the received direction.
    pub fn received_mbps(&self) -> f64 {
        if self.elapsed_ms == 0 {
            return 0.0;
        }
        (self.bytes_received as f64) / (self.elapsed_ms as f64 / 1000.0) / 1_048_576.0
    }

    /// Chunk delivery ratio (0..=1).
    pub fn delivery_ratio(&self) -> f64 {
        if self.chunks_total == 0 {
            return 0.0;
        }
        (self.chunks_delivered as f64) / (self.chunks_total as f64)
    }

    /// A flat, greppable report.
    pub fn report(&self) -> String {
        format!(
            "link_stats: sent={}B recv={}B chunks={}/{} retries={} elapsed={}ms sent_mbps={:.3} recv_mbps={:.3}",
            self.bytes_sent,
            self.bytes_received,
            self.chunks_delivered,
            self.chunks_total,
            self.retries,
            self.elapsed_ms,
            self.sent_mbps(),
            self.received_mbps()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mbps_computed() {
        let mut s = LinkStats::new();
        s.add_sent_bytes(10_485_760); // 10 MiB
        s.set_elapsed(5_000); // 5s
        assert!((s.sent_mbps() - 2.0).abs() < 0.01); // 10MiB/5s = 2 MB/s
    }

    #[test]
    fn zero_elapsed_is_zero() {
        let mut s = LinkStats::new();
        s.add_sent_bytes(10_485_760);
        assert_eq!(s.sent_mbps(), 0.0);
    }

    #[test]
    fn delivery_ratio() {
        let mut s = LinkStats::new();
        s.chunks_total = 10;
        s.chunks_delivered = 7;
        assert!((s.delivery_ratio() - 0.7).abs() < 1e-9);
    }

    #[test]
    fn report_shape() {
        let mut s = LinkStats::new();
        s.add_sent_bytes(100);
        s.set_elapsed(1000);
        let r = s.report();
        assert!(r.contains("link_stats:"));
        assert!(r.contains("sent=100B"));
        assert!(r.contains("elapsed=1000ms"));
    }
}
