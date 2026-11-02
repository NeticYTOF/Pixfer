//! Link capacity model: estimate the bytes/sec a light link can move,
//! from the camera frame rate and payload size. (Hilariously
//! over-engineered: a "capacity model" for a link that physically moves
//! a few KB/s.)

use crate::rs::RS_K;

/// A capacity estimate for a light link.
#[derive(Debug, Clone, Copy)]
pub struct CapacityEstimate {
    /// Estimated throughput in bytes/sec.
    pub bps: f64,
    /// The camera frame rate (fps) the estimate assumes.
    pub fps: f64,
    /// Payload bytes per chunk (RS_K).
    pub payload_bytes: u32,
}

/// Estimate the link capacity (bytes/sec) from the camera fps.
///
/// Each camera frame captures one on-screen frame carrying RS_K payload
/// bytes, so the achievable throughput is roughly `fps * RS_K`.
pub fn estimate_capacity(fps: f64) -> CapacityEstimate {
    CapacityEstimate {
        bps: fps * RS_K as f64,
        fps,
        payload_bytes: RS_K as u32,
    }
}

/// A sanity check: the estimate must be positive and scale with fps.
pub fn is_plausible(est: &CapacityEstimate) -> bool {
    est.bps > 0.0 && est.fps > 0.0 && est.payload_bytes > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_scales_with_fps() {
        let e30 = estimate_capacity(30.0);
        let e60 = estimate_capacity(60.0);
        assert!((e30.bps - 30.0 * RS_K as f64).abs() < 1e-6);
        assert!((e60.bps - 60.0 * RS_K as f64).abs() < 1e-6);
        assert!(e60.bps > e30.bps);
    }

    #[test]
    fn payload_bytes_is_rs_k() {
        let e = estimate_capacity(30.0);
        assert_eq!(e.payload_bytes, RS_K as u32);
    }

    #[test]
    fn plausible_check() {
        let e = estimate_capacity(30.0);
        assert!(is_plausible(&e));
    }

    #[test]
    fn low_fps_is_plausible() {
        let e = estimate_capacity(5.0);
        assert!(is_plausible(&e));
        assert!(e.bps > 0.0);
    }

    #[test]
    fn estimate_matches_hand_calc() {
        // At 15 fps: 15 * 223 = 3345 B/s.
        let e = estimate_capacity(15.0);
        assert!((e.bps - 3345.0).abs() < 1e-6);
    }
}
