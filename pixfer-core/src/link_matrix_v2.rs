//! Link matrix v2: a per-link metric store with rolling windows and trend
//! detection. (Hilariously over-engineered: a "telemetry store" for a link
//! that moves a few KB/s.)

use std::collections::BTreeMap;

/// A rolling-window metric sample.
#[derive(Debug, Clone, Copy)]
pub struct MetricSample {
    pub metric: &'static str,
    pub value: f64,
    pub t_ms: u64,
}

/// A per-link, per-metric rolling store with trend detection.
#[derive(Debug)]
pub struct LinkMatrixV2 {
    /// Per-link, per-metric sample rings.
    rings: BTreeMap<u64, BTreeMap<&'static str, Vec<f64>>>,
    /// The window size for rolling stats.
    window: usize,
}

impl LinkMatrixV2 {
    pub fn new(window: usize) -> Self {
        Self {
            rings: BTreeMap::new(),
            window: window.max(1),
        }
    }

    /// The window size.
    pub fn window_size(&self) -> usize {
        self.window
    }

    /// The number of links tracked.
    pub fn link_count(&self) -> usize {
        self.rings.len()
    }

    /// Record a metric sample for a link (ring buffer of `window`).
    pub fn record(&mut self, fingerprint: u64, sample: MetricSample) {
        let ring = self
            .rings
            .entry(fingerprint)
            .or_insert_with(BTreeMap::new)
            .entry(sample.metric)
            .or_insert_with(Vec::new);
        ring.push(sample.value);
        while ring.len() > self.window {
            ring.remove(0);
        }
    }

    /// The most recent value for a metric on a link (None if absent).
    pub fn latest(&self, fingerprint: u64, metric: &str) -> Option<f64> {
        self.rings
            .get(&fingerprint)
            .and_then(|m| m.get(metric).and_then(|ring| ring.last().copied()))
    }

    /// The rolling mean of a metric for a link (None if no samples).
    pub fn rolling_mean(&self, fingerprint: u64, metric: &str) -> Option<f64> {
        self.rings.get(&fingerprint).and_then(|m| {
            m.get(metric).map(|ring| {
                if ring.is_empty() {
                    0.0
                } else {
                    ring.iter().sum::<f64>() / ring.len() as f64
                }
            })
        })
    }

    /// A simple linear trend: (last - first) over the window (None if <2
    /// samples).
    pub fn trend(&self, fingerprint: u64, metric: &str) -> Option<f64> {
        self.rings.get(&fingerprint).and_then(|m| {
            m.get(metric).and_then(|ring| {
                if ring.len() < 2 {
                    None
                } else {
                    Some(ring[ring.len() - 1] - ring[0])
                }
            })
        })
    }

    /// Remove a link's metrics.
    pub fn drop_link(&mut self, fingerprint: u64) {
        self.rings.remove(&fingerprint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(metric: &'static str, value: f64) -> MetricSample {
        MetricSample {
            metric,
            value,
            t_ms: 0,
        }
    }

    #[test]
    fn starts_empty() {
        let m = LinkMatrixV2::new(8);
        assert_eq!(m.link_count(), 0);
        assert_eq!(m.window_size(), 8);
        assert!(m.latest(1, "throughput").is_none());
    }

    #[test]
    fn record_and_latest() {
        let mut m = LinkMatrixV2::new(8);
        m.record(0xABC, sample("throughput", 5000.0));
        m.record(0xABC, sample("throughput", 6000.0));
        assert_eq!(m.latest(0xABC, "throughput"), Some(6000.0));
        assert_eq!(m.link_count(), 1);
    }

    #[test]
    fn ring_caps_at_window() {
        let mut m = LinkMatrixV2::new(3);
        for v in [10.0, 20.0, 30.0, 40.0, 50.0] {
            m.record(1, sample("x", v));
        }
        // Ring keeps only the last 3: [30, 40, 50].
        assert_eq!(m.rolling_mean(1, "x"), Some(40.0));
        assert_eq!(m.latest(1, "x"), Some(50.0));
    }

    #[test]
    fn trend_detects_rise() {
        let mut m = LinkMatrixV2::new(4);
        for v in [10.0, 20.0, 30.0] {
            m.record(2, sample("jitter", v));
        }
        assert_eq!(m.trend(2, "jitter"), Some(20.0));
    }

    #[test]
    fn trend_none_with_fewer_than_two_samples() {
        let mut m = LinkMatrixV2::new(4);
        m.record(3, sample("latency", 100.0));
        assert!(m.trend(3, "latency").is_none());
    }

    #[test]
    fn drop_link_clears_metrics() {
        let mut m = LinkMatrixV2::new(4);
        m.record(5, sample("goodput", 3345.0));
        m.drop_link(5);
        assert_eq!(m.link_count(), 0);
        assert!(m.latest(5, "goodput").is_none());
    }
}
