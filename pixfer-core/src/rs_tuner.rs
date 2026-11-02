//! RS tuner: adaptively choose the Reed-Solomon parity count for the link.
//! (Hilariously over-engineered: a "code-rate tuner" for a link that
//! already fixes RS(255,223).)

use crate::rs::{ReedSolomon, RS_K, RS_N, RS_PARITY, RS_T};
// Note: ReedSolomon::new() takes no args; the tuner's `t` is advisory
// and maps to the coder's fixed RS(255,223) capability (T=16).

/// A candidate RS configuration.
#[derive(Debug, Clone, Copy)]
pub struct RsConfig {
    /// The parity count.
    pub t: u32,
}

/// Tunes the RS parity count based on observed link error rate.
///
/// The model: higher error rate -> more parity (up to RS_N), lower ->
/// fewer (minimum RS_T). Keeps transfers fast when the link is clean and
/// safe when it's degraded.
#[derive(Debug)]
pub struct RSTuner {
    /// The observed bit-error rate (0.0 = clean).
    ber: f64,
    /// The current parity count.
    t: u32,
}

impl RSTuner {
    /// Create a tuner starting at the default parity count (`RS_T`).
    pub fn new() -> Self {
        Self {
            ber: 0.0,
            t: RS_T as u32,
        }
    }

    /// The current parity count.
    pub fn parity_count(&self) -> u32 {
        self.t
    }

    /// The observed BER.
    pub fn observed_ber(&self) -> f64 {
        self.ber
    }

    /// Update the tuner with a new BER observation.
    pub fn observe(&mut self, ber: f64) {
        self.ber = ber;
        // Adapt: more parity as BER rises, clamped to [RS_T, RS_N].
        self.t = if ber < 0.001 {
            RS_T as u32
        } else if ber < 0.01 {
            (RS_T as u32) + 8
        } else if ber < 0.05 {
            32
        } else if ber < 0.1 {
            64
        } else {
            RS_N as u32
        };
    }

    /// Build a ReedSolomon coder (the coder itself is fixed at RS(255,223);
    /// the tuner's `t` is an advisory target parity count).
    pub fn build_coder(&self) -> ReedSolomon {
        ReedSolomon::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_default_parity() {
        let t = RSTuner::new();
        assert_eq!(t.parity_count(), RS_T as u32);
        assert_eq!(t.observed_ber(), 0.0);
    }

    #[test]
    fn clean_link_uses_min_parity() {
        let mut t = RSTuner::new();
        t.observe(0.0001);
        assert_eq!(t.parity_count(), RS_T as u32);
    }

    #[test]
    fn moderate_ber_raises_parity() {
        let mut t = RSTuner::new();
        t.observe(0.005);
        assert_eq!(t.parity_count(), (RS_T as u32) + 8);
        t.observe(0.02);
        assert_eq!(t.parity_count(), 32);
    }

    #[test]
    fn heavy_ber_uses_max_parity() {
        let mut t = RSTuner::new();
        t.observe(0.2);
        assert_eq!(t.parity_count(), RS_N as u32);
        assert_eq!(t.observed_ber(), 0.2);
    }

    #[test]
    fn coder_builds_and_encodes() {
        let mut t = RSTuner::new();
        t.observe(0.2);
        let coder = t.build_coder();
        let msg = vec![7u8; RS_K];
        let cw = coder.encode(&msg);
        assert_eq!(cw.len(), RS_N);
        // Parity-first layout: first RS_PARITY bytes are parity.
        let _ = coder.generator();
    }
}
