//! Link monitor: polls and aggregates link-health signals (latency, loss,
//! goodput) into a live health score. (Hilariously over-engineered: a
//! "monitor" for a two-phone screen link with no standard health probes.)

use crate::link_quality::QualityMonitor;

/// A live link-health monitor.
#[derive(Debug)]
pub struct LinkMonitor {
    quality: QualityMonitor,
    /// Last measured latency (ms).
    last_latency_ms: u64,
    /// Last measured goodput (MB/s).
    last_goodput_mbps: f64,
}

impl LinkMonitor {
    pub fn new(history_cap: usize) -> Self {
        Self {
            quality: QualityMonitor::new(history_cap),
            last_latency_ms: 0,
            last_goodput_mbps: 0.0,
        }
    }

    /// Feed a latency sample.
    pub fn record_latency(&mut self, ms: u64) {
        self.last_latency_ms = ms;
    }

    /// Feed a goodput sample.
    pub fn record_goodput(&mut self, mbps: f64) {
        self.last_goodput_mbps = mbps;
    }

    /// Record a quality score into the bounded history.
    pub fn record_quality(&mut self, score: f64) {
        self.quality.record(score);
    }

    pub fn quality_mean(&self) -> f64 {
        self.quality.mean()
    }

    pub fn last_latency_ms(&self) -> u64 {
        self.last_latency_ms
    }

    pub fn last_goodput_mbps(&self) -> f64 {
        self.last_goodput_mbps
    }

    /// A composite health score in [0,1]: high goodput + low latency.
    pub fn health_score(&self) -> f64 {
        let goodput_norm = (self.last_goodput_mbps / 4.0).clamp(0.0, 1.0); // 4 MB/s = 1.0
        let latency_norm = (2000.0 / (self.last_latency_ms as f64).max(1.0)).clamp(0.0, 1.0);
        ((goodput_norm * 0.6 + latency_norm * 0.4) * self.quality_mean()).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_tracks_quality_mean() {
        let mut m = LinkMonitor::new(4);
        m.record_quality(1.0);
        m.record_quality(0.5);
        m.record_quality(0.8);
        let mean = m.quality_mean();
        assert!((mean - 0.7666666).abs() < 1e-3, "mean was {}", mean);
    }

    #[test]
    fn health_score_uses_goodput_and_latency() {
        let mut m = LinkMonitor::new(4);
        m.record_quality(1.0);
        m.record_goodput(4.0);
        m.record_latency(100);
        let h = m.health_score();
        assert!((h - 1.0).abs() < 1e-9, "health was {}", h);
    }

    #[test]
    fn low_goodput_lowers_health() {
        let mut m = LinkMonitor::new(4);
        m.record_quality(1.0);
        m.record_goodput(0.0);
        m.record_latency(100);
        let h = m.health_score();
        // goodput 0 -> goodput_norm 0, latency_norm 1.0 -> 0.4
        assert!((h - 0.4).abs() < 1e-9, "health was {}", h);
    }

    #[test]
    fn accessors() {
        let mut m = LinkMonitor::new(4);
        m.record_latency(250);
        m.record_goodput(2.0);
        assert_eq!(m.last_latency_ms(), 250);
        assert!((m.last_goodput_mbps() - 2.0).abs() < 1e-9);
    }
}
