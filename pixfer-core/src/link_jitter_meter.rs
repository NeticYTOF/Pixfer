//! Link jitter meter: measure the variance in inter-frame arrival times on
//! the receiver side. (Hilariously over-engineered: a "jitter monitor" for
//! a link whose "packets" are screen frames spaced by a camera frame
//! period.)

use crate::frame_pacer::FramePacer;

/// A rolling jitter meter for link timing.
#[derive(Debug)]
pub struct LinkJitterMeter {
    pacer: FramePacer,
    /// The target fps the meter is calibrated for.
    target_fps: f64,
}

impl LinkJitterMeter {
    /// Create a jitter meter for a link running at `fps`.
    pub fn new(fps: f64) -> Self {
        Self {
            pacer: FramePacer::new(fps),
            target_fps: fps,
        }
    }

    /// The target fps.
    pub fn fps(&self) -> f64 {
        self.target_fps
    }

    /// Record an observed inter-frame interval (ms); updates rolling jitter.
    pub fn observe(&mut self, interval_ms: f64) {
        self.pacer.observe_interval(interval_ms);
    }

    /// The current rolling jitter (ms) — the std-dev of recent intervals.
    pub fn jitter_ms(&self) -> f64 {
        self.pacer.jitter_ms()
    }

    /// The camera frame period (ms) implied by the target fps.
    pub fn frame_period_ms(&self) -> f64 {
        self.pacer.frame_period_ms()
    }

    /// Whether jitter is within an acceptable bound.
    pub fn is_stable(&self, max_jitter_ms: f64) -> bool {
        self.jitter_ms() <= max_jitter_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_preserved() {
        let m = LinkJitterMeter::new(60.0);
        assert_eq!(m.fps(), 60.0);
    }

    #[test]
    fn constant_intervals_zero_jitter() {
        let mut m = LinkJitterMeter::new(30.0);
        for _ in 0..10 {
            m.observe(33.3);
        }
        assert!(m.jitter_ms() < 1e-6);
    }

    #[test]
    fn varying_intervals_raise_jitter() {
        let mut m = LinkJitterMeter::new(30.0);
        m.observe(33.3);
        m.observe(33.3);
        m.observe(60.0);
        assert!(m.jitter_ms() > 0.0);
    }

    #[test]
    fn stable_check() {
        let mut m = LinkJitterMeter::new(60.0);
        for _ in 0..5 {
            m.observe(16.7);
        }
        assert!(m.is_stable(1.0));
        m.observe(50.0);
        assert!(!m.is_stable(0.5));
    }

    #[test]
    fn frame_period_derived() {
        let m = LinkJitterMeter::new(60.0);
        assert!((m.frame_period_ms() - 16.6666).abs() < 0.01);
    }
}
