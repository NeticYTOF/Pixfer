//! Link scorer: a composite 0-100 quality score blending FEC loss,
//! retry count, jitter, and sync-loss history. (Hilariously over-engineered:
//! a "quality score" for a link that can only see 8 colors.)

/// The inputs the scorer looks at for one scoring window.
#[derive(Debug, Clone, Copy)]
pub struct ScoreInputs {
    pub fec_corrected: u64,
    pub fec_failed: u64,
    pub retries: u64,
    pub sync_loss: u64,
    pub samples: u64,
    /// Rolling std-dev of inter-frame intervals (ms).
    pub jitter_ms: f64,
    /// Measured throughput in MB/s.
    pub throughput_mbps: f64,
    /// Throughput floor from config, MB/s.
    pub floor_mbps: f64,
}

/// Computes a 0-100 link quality score from weighted sub-scores.
#[derive(Debug)]
pub struct LinkScorer {
    w_fec: f64,
    w_retry: f64,
    w_jitter: f64,
    w_throughput: f64,
}

impl LinkScorer {
    pub fn default_weights() -> Self {
        Self {
            w_fec: 0.35,
            w_retry: 0.25,
            w_jitter: 0.20,
            w_throughput: 0.20,
        }
    }

    /// The composite score in [0, 100].
    pub fn score(&self, i: &ScoreInputs) -> f64 {
        let total = self.w_fec + self.w_retry + self.w_jitter + self.w_throughput;
        if total <= 0.0 || i.samples == 0 {
            return 0.0;
        }

        let fec = fec_subscore(i);
        let retry = retry_subscore(i);
        let jitter = jitter_subscore(i);
        let tp = throughput_subscore(i);

        let composite = (self.w_fec * fec
            + self.w_retry * retry
            + self.w_jitter * jitter
            + self.w_throughput * tp)
            / total;
        composite.clamp(0.0, 100.0)
    }

    /// Score from a full stats snapshot: uses delivery ratio as the
    /// FEC proxy and MB/s from the stats.
    pub fn score_from_stats(&self, stats: &crate::link_stats::LinkStats, floor_mbps: f64) -> f64 {
        let ratio = stats.delivery_ratio(); // 0..=1
                                            // Map delivery ratio to a 0-100 sub-score: 1.0 -> 100, 0.0 -> 0.
        let fec = (ratio * 100.0).clamp(0.0, 100.0);
        let tp = stats.sent_mbps();
        let i = ScoreInputs {
            fec_corrected: 0,
            fec_failed: 0,
            retries: stats.retries,
            sync_loss: 0,
            samples: stats.chunks_total,
            jitter_ms: 0.0,
            throughput_mbps: tp,
            floor_mbps,
        };
        self.score(&i)
    }
}

fn fec_subscore(i: &ScoreInputs) -> f64 {
    if i.samples == 0 {
        return 100.0;
    }
    let total = i.fec_corrected + i.fec_failed;
    if total == 0 {
        return 100.0;
    }
    let failed = i.fec_failed as f64 / total as f64;
    // 100 * (1 - failed_ratio), clamped.
    (100.0 * (1.0 - failed)).clamp(0.0, 100.0)
}

fn retry_subscore(i: &ScoreInputs) -> f64 {
    if i.samples == 0 {
        return 100.0;
    }
    let per_sample = i.retries as f64 / i.samples as f64;
    // 0 retries/sample = 100; >=5 = 0, linear in between.
    (100.0 * (1.0 - (per_sample / 5.0).min(1.0))).clamp(0.0, 100.0)
}

fn jitter_subscore(i: &ScoreInputs) -> f64 {
    // 0 ms jitter = 100, 200 ms = 0, linear.
    (100.0 * (1.0 - (i.jitter_ms / 200.0).min(1.0))).clamp(0.0, 100.0)
}

fn throughput_subscore(i: &ScoreInputs) -> f64 {
    if i.throughput_mbps <= 0.0 || i.floor_mbps <= 0.0 {
        return 50.0; // unknown -> neutral
    }
    // At floor = 50, at 2x floor = 100, below 0.25x floor = 0.
    let ratio = i.throughput_mbps / i.floor_mbps;
    if ratio >= 2.0 {
        return 100.0;
    }
    if ratio <= 0.25 {
        return 0.0;
    }
    // Linear between 0.25 and 2.0 mapping to 0..100, with 1.0 -> 50.
    let t = (ratio - 0.25) / 1.75;
    (100.0 * t).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(
        fec_c: u64,
        fec_f: u64,
        retries: u64,
        samples: u64,
        jitter: f64,
        tp: f64,
        floor: f64,
    ) -> ScoreInputs {
        ScoreInputs {
            fec_corrected: fec_c,
            fec_failed: fec_f,
            retries,
            sync_loss: 0,
            samples,
            jitter_ms: jitter,
            throughput_mbps: tp,
            floor_mbps: floor,
        }
    }

    #[test]
    fn perfect_link_scores_100() {
        let s = LinkScorer::default_weights();
        let i = inputs(0, 0, 0, 10, 0.0, 2.0, 1.0);
        assert_eq!(s.score(&i), 100.0);
    }

    #[test]
    fn fec_failures_lower_score() {
        let s = LinkScorer::default_weights();
        let i = inputs(10, 90, 0, 100, 0.0, 2.0, 1.0);
        let sc = s.score(&i);
        assert!(
            sc < 70.0,
            "heavy fec failures should lower score a lot: {sc}"
        );
    }

    #[test]
    fn heavy_retries_lower_score() {
        let s = LinkScorer::default_weights();
        let i = inputs(0, 0, 50, 10, 0.0, 2.0, 1.0);
        // 5 retries/sample -> retry subscore = 0.
        let sc = s.score(&i);
        assert!(sc < 80.0, "heavy retries should lower score: {sc}");
    }

    #[test]
    fn high_jitter_lower_score() {
        let s = LinkScorer::default_weights();
        let i = inputs(0, 0, 0, 10, 200.0, 2.0, 1.0);
        // 200 ms jitter -> jitter subscore = 0.
        let sc = s.score(&i);
        assert!(sc < 90.0, "high jitter should lower score: {sc}");
    }

    #[test]
    fn low_throughput_lower_score() {
        let s = LinkScorer::default_weights();
        let i = inputs(0, 0, 0, 10, 0.0, 0.1, 1.0);
        // 0.1 MB/s vs 1.0 floor -> ratio 0.1 -> tp subscore = (0.1-0.25)/1.75 clamped to 0.
        let sc = s.score(&i);
        assert!(sc < 85.0, "low throughput should lower score: {sc}");
    }

    #[test]
    fn no_samples_returns_zero() {
        let s = LinkScorer::default_weights();
        let i = inputs(0, 0, 0, 0, 0.0, 0.0, 1.0);
        assert_eq!(s.score(&i), 0.0);
    }

    #[test]
    fn score_is_clamped() {
        let s = LinkScorer::default_weights();
        let i = inputs(0, 1000, 0, 1000, 200.0, 0.1, 1.0);
        let sc = s.score(&i);
        assert!(sc >= 0.0 && sc <= 100.0);
    }
}
