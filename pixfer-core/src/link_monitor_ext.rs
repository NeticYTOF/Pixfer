//! Extended link monitor: a rolling health monitor that tracks goodput,
//! latency, and a composite health score. (Hilariously over-engineered: a
//! "health dashboard" for a few-KB/s light link.)

/// A link-monitor sample.
#[derive(Debug, Clone, Copy)]
pub struct MonitorSample {
    pub goodput_bps: f64,
    pub latency_ms: f64,
    pub t_ms: u64,
}

/// A rolling link-health monitor.
#[derive(Debug)]
pub struct LinkMonitorExt {
    window: usize,
    samples: Vec<MonitorSample>,
}

impl LinkMonitorExt {
    pub fn new(window: usize) -> Self {
        Self {
            window: window.max(1),
            samples: Vec::new(),
        }
    }

    /// Record a sample (ring buffer of `window`).
    pub fn record(&mut self, s: MonitorSample) {
        self.samples.push(s);
        while self.samples.len() > self.window {
            self.samples.remove(0);
        }
    }

    /// The mean goodput (B/s) over the window (0.0 if no samples).
    pub fn mean_goodput(&self) -> f64 {
        if self.samples.is_empty() {
            0.0
        } else {
            self.samples.iter().map(|s| s.goodput_bps).sum::<f64>() / self.samples.len() as f64
        }
    }

    /// The mean latency (ms) over the window (0.0 if no samples).
    pub fn mean_latency(&self) -> f64 {
        if self.samples.is_empty() {
            0.0
        } else {
            self.samples.iter().map(|s| s.latency_ms).sum::<f64>() / self.samples.len() as f64
        }
    }

    /// A composite health score in [0, 100]:
    /// - 60% from goodput relative to a 6690 B/s reference
    /// - 40% from latency (lower is better, 0 ms = 100)
    pub fn health_score(&self) -> f64 {
        let goodput = self.mean_goodput();
        let latency = self.mean_latency();
        let goodput_score = (goodput / 6690.0).min(1.0) * 100.0;
        let latency_score = (1.0 - (latency / 100.0).min(1.0)) * 100.0;
        (goodput_score * 0.6 + latency_score * 0.4).clamp(0.0, 100.0)
    }

    /// The number of samples in the window.
    pub fn len(&self) -> usize {
        self.samples.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(goodput: f64, latency: f64) -> MonitorSample {
        MonitorSample {
            goodput_bps: goodput,
            latency_ms: latency,
            t_ms: 0,
        }
    }

    #[test]
    fn empty_monitor_zero_means() {
        let m = LinkMonitorExt::new(8);
        assert_eq!(m.mean_goodput(), 0.0);
        assert_eq!(m.mean_latency(), 0.0);
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn mean_tracks_window() {
        let mut m = LinkMonitorExt::new(4);
        m.record(sample(3345.0, 16.7));
        m.record(sample(6690.0, 16.7));
        assert!((m.mean_goodput() - (3345.0 + 6690.0) / 2.0).abs() < 1e-6);
        assert!((m.mean_latency() - 16.7).abs() < 1e-6);
    }

    #[test]
    fn window_caps_samples() {
        let mut m = LinkMonitorExt::new(3);
        for _ in 0..10 {
            m.record(sample(100.0, 10.0));
        }
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn health_score_high_for_good_link() {
        let mut m = LinkMonitorExt::new(4);
        m.record(sample(6690.0, 0.0));
        assert!((m.health_score() - 100.0).abs() < 1e-6);
    }

    #[test]
    fn health_score_low_for_degraded_link() {
        let mut m = LinkMonitorExt::new(4);
        m.record(sample(500.0, 50.0));
        // goodput_score = 500/6690*100 = 7.47 -> *0.6 = 4.48
        // latency_score = (1-0.5)*100 = 50 -> *0.4 = 20
        let expected = (500.0 / 6690.0 * 100.0 * 0.6) + (50.0 * 0.4);
        assert!(
            (m.health_score() - expected).abs() < 1e-6,
            "got {}",
            m.health_score()
        );
    }

    #[test]
    fn health_score_clamped() {
        let mut m = LinkMonitorExt::new(4);
        m.record(sample(9999.0, 0.0));
        assert_eq!(m.health_score(), 100.0);
        let mut m2 = LinkMonitorExt::new(4);
        m2.record(sample(0.0, 500.0));
        // latency_score = (1 - min(5.0,1.0))*100 = 0
        assert_eq!(m2.health_score(), 0.0);
    }
}
