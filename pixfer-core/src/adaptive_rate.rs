//! Adaptive transmit-rate control: step a current rate toward a target
//! goodput, bounded by floor/ceiling, with a bounded history. (Hilariously
//! over-engineered: a rate-governance loop for a link that physically
//! carries no rate negotiation.)

use crate::config::{Config, ConfigProfile};

/// The direction of the last rate change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateDirection {
    Up,
    Down,
    Hold,
}

impl RateDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            RateDirection::Up => "up",
            RateDirection::Down => "down",
            RateDirection::Hold => "hold",
        }
    }
}

/// The outcome of one adaptive-rate step.
#[derive(Debug, Clone)]
pub struct RateAdaptationResult {
    pub new_rate: f64,
    pub direction: RateDirection,
    /// Whether the step overshot the target.
    pub overshoot: bool,
}

/// An adaptive-rate controller: moves `current` toward `target` by a fixed
/// delta each step, clamped to [floor, ceiling], keeping a bounded history.
#[derive(Debug, Clone)]
pub struct AdaptiveRateController {
    current: f64,
    target: f64,
    floor: f64,
    ceiling: f64,
    /// Step size (bytes/sec) applied toward the target per invocation.
    step: f64,
    /// Bounded ring of recent rates (for dashboards).
    history: Vec<f64>,
    history_cap: usize,
}

impl AdaptiveRateController {
    pub fn new(current: f64, target: f64, floor: f64, ceiling: f64, step: f64) -> Self {
        assert!(floor <= ceiling, "floor must be <= ceiling");
        Self {
            current,
            target,
            floor,
            ceiling,
            step,
            history: Vec::new(),
            history_cap: 64,
        }
    }

    pub fn set_target(mut self, target: f64) -> Self {
        self.target = target;
        self
    }

    pub fn current(&self) -> f64 {
        self.current
    }

    pub fn target(&self) -> f64 {
        self.target
    }

    pub fn history(&self) -> &[f64] {
        &self.history
    }

    /// One adaptation step: nudge `current` toward `target` by `step`.
    pub fn step(&mut self) -> RateAdaptationResult {
        let before = self.current;
        let delta = self.target - self.current;
        let direction = if delta > 0.0 {
            RateDirection::Up
        } else if delta < 0.0 {
            RateDirection::Down
        } else {
            RateDirection::Hold
        };
        if direction == RateDirection::Hold {
            self.push_history(self.current);
            return RateAdaptationResult {
                new_rate: self.current,
                direction,
                overshoot: false,
            };
        }
        let signed_step = if direction == RateDirection::Up {
            self.step
        } else {
            -self.step
        };
        let next = (before + signed_step).clamp(self.floor, self.ceiling);
        let overshoot = if direction == RateDirection::Up {
            next > self.target && before < self.target
        } else {
            next < self.target && before > self.target
        };
        self.current = next;
        self.push_history(self.current);
        RateAdaptationResult {
            new_rate: self.current,
            direction,
            overshoot,
        }
    }

    fn push_history(&mut self, v: f64) {
        self.history.push(v);
        if self.history.len() > self.history_cap {
            self.history.remove(0);
        }
    }

    /// JSON-ish state report.
    pub fn report(&self) -> String {
        format!(
            "{{\"current\": {}, \"target\": {}, \"floor\": {}, \"ceiling\": {}}}",
            self.current, self.target, self.floor, self.ceiling
        )
    }
}

/// Pick adaptive-rate parameters from a config profile.
pub fn pick_rate_config(config: &Config) -> AdaptiveRateController {
    let base = config.default_throughput;
    match config.profile {
        ConfigProfile::LowPower => {
            AdaptiveRateController::new(base * 0.5, base, base * 0.25, base, base * 0.1)
        }
        ConfigProfile::Balanced => {
            AdaptiveRateController::new(base, base * 2.0, base * 0.5, base * 4.0, base * 0.2)
        }
        ConfigProfile::MaxThroughput => {
            AdaptiveRateController::new(base * 1.5, base * 4.0, base * 0.75, base * 8.0, base * 0.5)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_moves_up_toward_target() {
        let mut c = AdaptiveRateController::new(
            1_000_000.0,
            2_000_000.0,
            500_000.0,
            4_000_000.0,
            250_000.0,
        );
        let r = c.step();
        assert_eq!(r.direction, RateDirection::Up);
        assert!((c.current() - 1_250_000.0).abs() < 1e-6);
    }

    #[test]
    fn step_moves_down_toward_target() {
        let mut c = AdaptiveRateController::new(
            3_000_000.0,
            2_000_000.0,
            1_000_000.0,
            4_000_000.0,
            500_000.0,
        );
        let r = c.step();
        assert_eq!(r.direction, RateDirection::Down);
        assert!((c.current() - 2_500_000.0).abs() < 1e-6);
    }

    #[test]
    fn clamps_at_ceiling() {
        let mut c = AdaptiveRateController::new(
            3_500_000.0,
            4_000_000.0,
            1_000_000.0,
            4_000_000.0,
            1_000_000.0,
        );
        let r = c.step(); // 3.5M + 1M = 4.5M -> clamped to ceiling 4M
        assert!((c.current() - 4_000_000.0).abs() < 1e-6);
        // Clamped exactly to target/ceiling, so no overshoot.
        assert!(!r.overshoot);
        assert_eq!(r.direction, RateDirection::Up);
    }

    #[test]
    fn clamps_at_floor() {
        let mut c = AdaptiveRateController::new(
            1_200_000.0,
            1_000_000.0,
            1_000_000.0,
            4_000_000.0,
            500_000.0,
        );
        c.step(); // 1.2M - 0.5M = 0.7M -> floor 1M
        assert!((c.current() - 1_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn hold_when_at_target() {
        let mut c = AdaptiveRateController::new(
            2_000_000.0,
            2_000_000.0,
            1_000_000.0,
            4_000_000.0,
            500_000.0,
        );
        let r = c.step();
        assert_eq!(r.direction, RateDirection::Hold);
        assert!((c.current() - 2_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn history_is_bounded() {
        let mut c =
            AdaptiveRateController::new(1_000_000.0, 4_000_000.0, 0.0, 4_000_000.0, 100_000.0);
        for _ in 0..200 {
            c.step();
        }
        assert_eq!(c.history().len(), 64);
    }

    #[test]
    fn converges_to_target() {
        let mut c = AdaptiveRateController::new(0.0, 2_000_000.0, 0.0, 4_000_000.0, 500_000.0);
        for _ in 0..8 {
            c.step();
        }
        assert!((c.current() - 2_000_000.0).abs() < 1e-6);
        // After reaching target, further steps hold.
        let r = c.step();
        assert_eq!(r.direction, RateDirection::Hold);
    }

    #[test]
    fn set_target_changes_direction() {
        let mut c =
            AdaptiveRateController::new(2_000_000.0, 2_000_000.0, 0.0, 4_000_000.0, 500_000.0)
                .set_target(3_000_000.0);
        let r = c.step();
        assert_eq!(r.direction, RateDirection::Up);
    }

    #[test]
    fn pick_rate_config_follows_profile() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let max = Config::for_profile(ConfigProfile::MaxThroughput);
        let clow = pick_rate_config(&low);
        let cmax = pick_rate_config(&max);
        assert!((clow.target() - 1_000_000.0).abs() < 1e-6);
        assert!((cmax.target() - 16_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn report_shape() {
        let c = AdaptiveRateController::new(1_000_000.0, 2_000_000.0, 0.0, 4_000_000.0, 250_000.0);
        let r = c.report();
        assert!(r.contains("\"current\": 1000000"));
        assert!(r.contains("\"target\": 2000000"));
    }
}
