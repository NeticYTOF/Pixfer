//! Frame pacing and dwell-time scheduling: how long each frame must stay
//! on the screen so the peer camera captures it, plus a `Pacer` that
//! enforces a minimum dwell and an inter-frame gap.

/// Camera frame period assumptions (ms). The pacer uses these to compute
/// the minimum dwell a frame needs to be captured.
pub struct Pacer {
    /// Camera frame period in ms (1000/fps).
    frame_period_ms: f64,
    /// Extra safety margin added to the dwell.
    margin_ms: f64,
    /// Minimum dwell in ms.
    min_dwell_ms: f64,
}

impl Pacer {
    pub fn for_fps(fps: f64) -> Self {
        assert!(fps > 0.0, "fps must be positive");
        Self {
            frame_period_ms: 1000.0 / fps,
            margin_ms: 0.0,
            min_dwell_ms: (2.0 * (1000.0 / fps)),
        }
    }

    pub fn with_margin(mut self, margin_ms: f64) -> Self {
        self.margin_ms = margin_ms;
        self
    }

    /// Minimum ms a frame must remain on screen to be reliably captured.
    pub fn min_dwell_ms(&self) -> f64 {
        self.min_dwell_ms.max(self.frame_period_ms * 2.0) + self.margin_ms
    }

    /// Seconds to transfer `size` bytes at `throughput` B/s.
    pub fn estimate_transfer_s(size: u64, throughput: f64) -> f64 {
        (size as f64) / throughput
    }

    /// Total transfer time estimate for `n_chunks` frames:
    /// n * (dwell + ack round-trip), where the round-trip is another
    /// camera frame period on the peer side.
    pub fn transfer_time_s(&self, n_chunks: u32) -> f64 {
        let per_chunk = self.min_dwell_ms() + self.frame_period_ms;
        (n_chunks as f64) * per_chunk / 1000.0
    }
}

/// A tiny monotonic-clock abstraction so tests can inject time.
pub trait Clock {
    fn now_ms(&self) -> u64;
}

pub struct SystemClock;
impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacer_dwell_scales_with_fps() {
        let p = Pacer::for_fps(30.0);
        assert!((p.min_dwell_ms() - 2.0 * (1000.0 / 30.0)).abs() < 1e-6);
        let p60 = Pacer::for_fps(60.0);
        assert!((p60.min_dwell_ms() - 2.0 * (1000.0 / 60.0)).abs() < 1e-6);
        // 10 chunks at 30fps: 10 * (dwell + period) = 1.0 s
        assert!((p.transfer_time_s(10) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn margin_extends_dwell() {
        let p = Pacer::for_fps(30.0).with_margin(10.0);
        assert!((p.min_dwell_ms() - (2.0 * (1000.0 / 30.0) + 10.0)).abs() < 1e-6);
    }

    #[test]
    fn system_clock_advances() {
        let c = SystemClock;
        let a = c.now_ms();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(c.now_ms() >= a);
    }
}
