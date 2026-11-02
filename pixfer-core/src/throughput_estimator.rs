//! Throughput estimation: an exponential moving average of observed
//! bytes/sec, with a confidence interval. (Hilariously over-engineered: a
//! goodput estimator for a link with no standard bandwidth negotiation.)

use crate::config::{Config, ConfigProfile};

/// An EMA-based throughput estimator.
#[derive(Debug, Clone)]
pub struct ThroughputEstimator {
    estimate: f64,
    alpha: f64,
    samples: u32,
    min_samples_for_confidence: u32,
}

impl ThroughputEstimator {
    pub fn new(alpha: f64, min_samples_for_confidence: u32) -> Self {
        assert!((0.0..=1.0).contains(&alpha));
        Self {
            estimate: 0.0,
            alpha,
            samples: 0,
            min_samples_for_confidence,
        }
    }

    /// Fold in a new observed bytes/sec.
    pub fn observe(&mut self, bytes_per_sec: f64) {
        self.samples = self.samples.saturating_add(1);
        if self.samples == 1 {
            self.estimate = bytes_per_sec;
        } else {
            self.estimate = self.alpha * bytes_per_sec + (1.0 - self.alpha) * self.estimate;
        }
    }

    pub fn estimate(&self) -> f64 {
        self.estimate
    }

    /// Confidence: high once we've seen enough samples.
    pub fn is_confident(&self) -> bool {
        self.samples >= self.min_samples_for_confidence
    }

    pub fn sample_count(&self) -> u32 {
        self.samples
    }
}

/// Pick estimator parameters from a config profile.
pub fn pick_estimator(config: &Config) -> ThroughputEstimator {
    match config.profile {
        ConfigProfile::LowPower => ThroughputEstimator::new(0.5, 3),
        ConfigProfile::Balanced => ThroughputEstimator::new(0.3, 5),
        ConfigProfile::MaxThroughput => ThroughputEstimator::new(0.1, 10),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_converges() {
        let mut e = ThroughputEstimator::new(0.5, 3);
        for _ in 0..20 {
            e.observe(2_000_000.0);
        }
        assert!((e.estimate() - 2_000_000.0).abs() < 1.0);
        assert!(e.is_confident());
    }

    #[test]
    fn first_sample_sets_estimate() {
        let mut e = ThroughputEstimator::new(0.3, 5);
        e.observe(1_000_000.0);
        assert!((e.estimate() - 1_000_000.0).abs() < 1e-6);
        assert!(!e.is_confident());
    }

    #[test]
    fn confidence_threshold() {
        let mut e = ThroughputEstimator::new(0.3, 5);
        for _ in 0..4 {
            e.observe(2_000_000.0);
        }
        assert!(!e.is_confident());
        e.observe(2_000_000.0);
        assert!(e.is_confident());
    }

    #[test]
    fn pick_estimator_follows_profile() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let max = Config::for_profile(ConfigProfile::MaxThroughput);
        let el = pick_estimator(&low);
        let em = pick_estimator(&max);
        assert!(!el.is_confident());
        assert!(!em.is_confident());
    }
}
