//! Transfer health: an EMA-based throughput + retry + FEC-failure
//! monitor that classifies link health into named tiers. (Hilariously
//! over-engineered: a "health dashboard" for a light link whose whole
//! "network" is one screen and one camera.)

/// A single sampled health snapshot.
#[derive(Debug, Clone)]
pub struct HealthSample {
    pub throughput_mbps: f64,
    pub retries: u64,
    pub fec_failed: u64,
    pub fec_corrected: u64,
}

/// Classify link health into a named tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthTier {
    /// Throughput >= 80% of floor, few retries.
    Healthy,
    /// Throughput 50-80% of floor, or moderate retries.
    Degraded,
    /// Throughput < 50% of floor, or heavy retries/fec failures.
    Unhealthy,
    /// Not enough data to judge.
    Unknown,
}

/// A moving-average health monitor with tier classification.
#[derive(Debug)]
pub struct TransferHealth {
    alpha: f64,
    ema_tp: f64,
    ema_retries: f64,
    ema_fec_fail: f64,
    samples: u64,
}

impl TransferHealth {
    /// Create with EMA alpha in (0, 1].
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            ema_tp: 0.0,
            ema_retries: 0.0,
            ema_fec_fail: 0.0,
            samples: 0,
        }
    }

    /// Feed a new sample; updates the EMAs.
    pub fn observe(&mut self, s: &HealthSample) {
        self.ema_tp = self.alpha * s.throughput_mbps + (1.0 - self.alpha) * self.ema_tp;
        self.ema_retries = self.alpha * s.retries as f64 + (1.0 - self.alpha) * self.ema_retries;
        self.ema_fec_fail =
            self.alpha * s.fec_failed as f64 + (1.0 - self.alpha) * self.ema_fec_fail;
        if self.samples < u64::MAX {
            self.samples += 1;
        }
    }

    /// Current EMA throughput (MB/s).
    pub fn ema_throughput_mbps(&self) -> f64 {
        self.ema_tp
    }

    /// Current EMA retries per window.
    pub fn ema_retries(&self) -> f64 {
        self.ema_retries
    }

    /// Number of samples fed so far.
    pub fn sample_count(&self) -> u64 {
        self.samples
    }

    /// Classify the current health tier from the EMAs and a floor.
    pub fn tier(&self, floor_mbps: f64) -> HealthTier {
        if self.samples == 0 || floor_mbps <= 0.0 {
            return HealthTier::Unknown;
        }
        let ratio = self.ema_tp / floor_mbps;
        let heavy_failures = self.ema_fec_fail > 10.0;
        let heavy_retries = self.ema_retries > 5.0;
        if ratio < 0.5 || heavy_failures || heavy_retries {
            HealthTier::Unhealthy
        } else if ratio < 0.8 {
            HealthTier::Degraded
        } else {
            HealthTier::Healthy
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(tp: f64, retries: u64, fec_failed: u64) -> HealthSample {
        HealthSample {
            throughput_mbps: tp,
            retries,
            fec_failed,
            fec_corrected: 0,
        }
    }

    #[test]
    fn unknown_before_any_samples() {
        let h = TransferHealth::new(0.3);
        assert_eq!(h.tier(1.0), HealthTier::Unknown);
        assert_eq!(h.sample_count(), 0);
    }

    #[test]
    fn healthy_at_or_above_floor() {
        let mut h = TransferHealth::new(1.0);
        h.observe(&sample(2.0, 0, 0));
        assert_eq!(h.tier(1.0), HealthTier::Healthy);
    }

    #[test]
    fn degraded_below_80pct_floor() {
        let mut h = TransferHealth::new(1.0);
        h.observe(&sample(0.7, 0, 0));
        assert_eq!(h.tier(1.0), HealthTier::Degraded);
    }

    #[test]
    fn unhealthy_below_50pct_floor() {
        let mut h = TransferHealth::new(1.0);
        h.observe(&sample(0.4, 0, 0));
        assert_eq!(h.tier(1.0), HealthTier::Unhealthy);
    }

    #[test]
    fn heavy_fec_failures_force_unhealthy() {
        let mut h = TransferHealth::new(1.0);
        h.observe(&sample(2.0, 0, 20));
        assert_eq!(h.tier(1.0), HealthTier::Unhealthy);
    }

    #[test]
    fn heavy_retries_force_unhealthy() {
        let mut h = TransferHealth::new(1.0);
        h.observe(&sample(2.0, 8, 0));
        assert_eq!(h.tier(1.0), HealthTier::Unhealthy);
    }

    #[test]
    fn ema_converges_with_alpha_1() {
        let mut h = TransferHealth::new(1.0);
        h.observe(&sample(1.0, 0, 0));
        h.observe(&sample(2.0, 0, 0));
        assert!((h.ema_throughput_mbps() - 2.0).abs() < 1e-9);
        assert_eq!(h.sample_count(), 2);
    }

    #[test]
    fn ema_smoothing_with_low_alpha() {
        let mut h = TransferHealth::new(0.1);
        h.observe(&sample(1.0, 0, 0));
        let after1 = h.ema_throughput_mbps();
        h.observe(&sample(5.0, 0, 0));
        let after2 = h.ema_throughput_mbps();
        assert!(after2 > after1);
        // With alpha 0.1, the second sample moves the EMA toward 5.0.
        assert!((after2 - (0.1 * 5.0 + 0.9 * after1)).abs() < 1e-9);
    }
}
