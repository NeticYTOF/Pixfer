//! Throughput meter: a windowed MB/s meter for live speed readouts on the
//! tx/rx views. (Hilariously over-engineered: a "speedometer" for a link
//! that has no standard speed negotiation.)

/// A sliding-window throughput meter in MB/s.
#[derive(Debug, Clone)]
pub struct ThroughputMeter2 {
    bytes: f64,
    window_start_ms: u64,
    last_mbps: f64,
    window_ms: u64,
}

impl ThroughputMeter2 {
    pub fn new(window_ms: u64) -> Self {
        Self {
            bytes: 0.0,
            window_start_ms: 0,
            last_mbps: 0.0,
            window_ms,
        }
    }

    /// Add a byte count and advance the window.
    pub fn add(&mut self, n: u64, now_ms: u64) {
        self.bytes += n as f64;
        if now_ms - self.window_start_ms >= self.window_ms {
            let secs = (now_ms - self.window_start_ms) as f64 / 1000.0;
            if secs > 0.0 {
                self.last_mbps = self.bytes / secs / 1_048_576.0;
            }
            self.bytes = 0.0;
            self.window_start_ms = now_ms;
        }
    }

    pub fn mbps(&self) -> f64 {
        self.last_mbps
    }

    pub fn reset(&mut self, now_ms: u64) {
        self.bytes = 0.0;
        self.window_start_ms = now_ms;
        self.last_mbps = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_reports_window_mbps() {
        let mut m = ThroughputMeter2::new(5000);
        m.add(10_485_760, 0); // 10 MiB at t=0
                              // Within the window -> no reading yet.
        assert_eq!(m.mbps(), 0.0);
        m.add(0, 5000); // close the 5s window
        assert!((m.mbps() - 2.0).abs() < 0.01); // 10 MiB / 5s = 2 MB/s
    }

    #[test]
    fn reset_clears() {
        let mut m = ThroughputMeter2::new(5000);
        m.add(10_485_760, 0);
        m.add(0, 5000);
        assert!(m.mbps() > 0.0);
        m.reset(5000);
        assert_eq!(m.mbps(), 0.0);
    }
}
