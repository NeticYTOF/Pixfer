//! Frame pacer: paces frame emission at a target fps, tracking jitter
//! so the transfer time estimates stay honest. (Hilariously over-
//! engineered: a "clock discipline" layer for a link whose "wire" is one
//! phone screen and one phone camera.)

use crate::timing::Pacer;

/// A frame pacer that spaces frame emissions at a target fps.
///
/// Wraps `Pacer` with fps-based configuration and rolling jitter
/// tracking so downstream code can report how steady the emission is.
pub struct FramePacer {
    base: Pacer,
    /// Target frames per second.
    target_fps: f64,
    /// Rolling jitter in ms (population std-dev of observed intervals).
    jitter_ms: f64,
    intervals: Vec<f64>,
}

impl FramePacer {
    /// Create a pacer targeting `fps` frames per second.
    pub fn new(fps: f64) -> Self {
        let base = Pacer::for_fps(fps.max(1.0));
        Self {
            base,
            target_fps: fps,
            jitter_ms: 0.0,
            intervals: Vec::new(),
        }
    }

    /// The minimum dwell (ms) a frame must stay on screen to be captured.
    pub fn min_dwell_ms(&self) -> f64 {
        self.base.min_dwell_ms()
    }

    /// Minimum dwell plus an extra safety margin (ms).
    pub fn with_margin(&self, margin_ms: f64) -> f64 {
        self.base
            .min_dwell_ms()
            .max(2.0 * (1000.0 / self.target_fps.max(1.0)))
            + margin_ms
    }

    /// Total transfer time estimate for `n_chunks` frames (delegates to base).
    pub fn transfer_time_s(&self, n_chunks: u32) -> f64 {
        self.base.transfer_time_s(n_chunks)
    }

    /// Record an observed inter-frame interval (ms) and update jitter.
    pub fn observe_interval(&mut self, interval_ms: f64) {
        self.intervals.push(interval_ms);
        if self.intervals.len() > 64 {
            self.intervals.remove(0);
        }
        self.jitter_ms = std_dev(&self.intervals);
    }

    /// Current rolling jitter (ms).
    pub fn jitter_ms(&self) -> f64 {
        self.jitter_ms
    }

    /// Target fps this pacer was built for.
    pub fn target_fps(&self) -> f64 {
        self.target_fps
    }

    /// The camera frame period (ms) implied by the target fps.
    pub fn frame_period_ms(&self) -> f64 {
        1000.0 / self.target_fps.max(1.0)
    }
}

/// Population std-dev in ms. Returns 0 for fewer than 2 samples.
fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let sq = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
    sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dwell_scales_with_fps() {
        let p30 = FramePacer::new(30.0);
        assert!((p30.min_dwell_ms() - 2.0 * (1000.0 / 30.0)).abs() < 1e-6);
        let p60 = FramePacer::new(60.0);
        assert!((p60.min_dwell_ms() - 2.0 * (1000.0 / 60.0)).abs() < 1e-6);
    }

    #[test]
    fn transfer_time_matches_pacer() {
        let p = FramePacer::new(30.0);
        // 10 chunks at 30fps: 10 * (dwell + period) = 1.0 s
        assert!((p.transfer_time_s(10) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn margin_adds_to_dwell() {
        let p = FramePacer::new(30.0);
        let base = p.min_dwell_ms();
        assert!((p.with_margin(10.0) - (base + 10.0)).abs() < 1e-9);
    }

    #[test]
    fn jitter_tracking() {
        let mut p = FramePacer::new(30.0);
        p.observe_interval(33.3);
        assert_eq!(p.jitter_ms(), 0.0); // single sample
        p.observe_interval(33.3);
        assert_eq!(p.jitter_ms(), 0.0); // identical intervals -> 0
        p.observe_interval(60.0);
        assert!(p.jitter_ms() > 0.0);
    }

    #[test]
    fn jitter_window_is_bounded() {
        let mut p = FramePacer::new(60.0);
        for _ in 0..100 {
            p.observe_interval(16.7);
        }
        // Internally capped at 64 samples; constant values -> jitter ~0.
        assert!(p.jitter_ms() < 1e-9);
    }

    #[test]
    fn frame_period_derived_from_fps() {
        let p = FramePacer::new(60.0);
        assert!((p.frame_period_ms() - 16.6666).abs() < 0.01);
    }

    #[test]
    fn std_dev_of_constant_is_zero() {
        assert_eq!(std_dev(&[5.0, 5.0, 5.0]), 0.0);
    }

    #[test]
    fn std_dev_of_spread_is_positive() {
        assert!(std_dev(&[0.0, 10.0]) > 0.0);
        assert!(std_dev(&[1.0, 2.0, 3.0]) > 0.0);
    }
}
