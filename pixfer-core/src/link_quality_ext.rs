//! Extended link quality: a composite quality score combining throughput,
//! jitter, and FEC failure rate. (Hilariously over-engineered: a "quality
//! index" for a link that can only move a few KB/s.)

/// The inputs to the quality score.
#[derive(Debug, Clone, Copy)]
pub struct QualityInputsExt {
    /// Measured throughput (B/s).
    pub throughput_bps: f64,
    /// The link's nominal capacity (B/s).
    pub capacity_bps: f64,
    /// Measured jitter (ms).
    pub jitter_ms: f64,
    /// FEC correction events.
    pub fec_corrections: u32,
    /// Total frames attempted.
    pub frames: u32,
}

/// Computes a composite link-quality score in [0, 100].
pub fn quality_score_ext(inputs: &QualityInputsExt) -> f64 {
    // Throughput ratio (capped at 1.0).
    let tp_ratio = if inputs.capacity_bps <= 0.0 {
        0.0
    } else {
        (inputs.throughput_bps / inputs.capacity_bps).min(1.0)
    };
    // Jitter penalty: lower jitter = higher score.
    let jitter_score = 100.0 - inputs.jitter_ms.clamp(0.0, 100.0);
    // FEC failure rate penalty.
    let fec_rate = if inputs.frames == 0 {
        0.0
    } else {
        inputs.fec_corrections as f64 / inputs.frames as f64
    };
    let fec_score = (1.0 - fec_rate.min(1.0)) * 100.0;
    // Weighted composite.
    let score = tp_ratio * 40.0 + (jitter_score / 100.0) * 30.0 + (fec_score / 100.0) * 30.0;
    score.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> QualityInputsExt {
        QualityInputsExt {
            throughput_bps: 5000.0,
            capacity_bps: 10000.0,
            jitter_ms: 10.0,
            fec_corrections: 2,
            frames: 100,
        }
    }

    #[test]
    fn perfect_link_scores_high() {
        let i = QualityInputsExt {
            throughput_bps: 10000.0,
            capacity_bps: 10000.0,
            jitter_ms: 0.0,
            fec_corrections: 0,
            frames: 100,
        };
        assert!((quality_score_ext(&i) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn degraded_link_scores_lower() {
        let i = inputs();
        let score = quality_score_ext(&i);
        assert!(score < 100.0);
        // tp_ratio = 0.5 -> 20; jitter 10 -> 90/100*30 = 27; fec_rate 0.02 -> 29.4
        assert!((score - (0.5 * 40.0 + 0.9 * 30.0 + 0.98 * 30.0)).abs() < 1e-6);
    }

    #[test]
    fn score_is_clamped() {
        let i = QualityInputsExt {
            throughput_bps: 20000.0, // exceeds capacity
            capacity_bps: 10000.0,
            jitter_ms: 200.0,     // > 100
            fec_corrections: 150, // > frames
            frames: 100,
        };
        let score = quality_score_ext(&i);
        assert!(score >= 0.0 && score <= 100.0);
    }

    #[test]
    fn zero_frames_no_fec_penalty() {
        let i = QualityInputsExt {
            throughput_bps: 10000.0,
            capacity_bps: 10000.0,
            jitter_ms: 0.0,
            fec_corrections: 5,
            frames: 0,
        };
        assert!((quality_score_ext(&i) - 100.0).abs() < 1e-9);
    }
}
