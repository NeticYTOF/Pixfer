//! Thermal model / thermal throttling for mobile devices. (Hilariously
//! over-engineered: a thermal governor for a phone running hot from a
//! full-bright screen.)

use crate::config::{Config, ConfigProfile};

/// Thermal operating states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    Nominal,
    Warn,
    Throttle,
    Critical,
}

impl ThermalState {
    pub fn as_str(self) -> &'static str {
        match self {
            ThermalState::Nominal => "nominal",
            ThermalState::Warn => "warn",
            ThermalState::Throttle => "throttle",
            ThermalState::Critical => "critical",
        }
    }
}

/// The thermal state of a device.
#[derive(Debug, Clone)]
pub struct ThermalState2 {
    pub temperature_c: f64,
    pub state: ThermalState,
}

/// Returns true when the temperature crosses the throttle threshold.
pub fn thermal_throttle(temperature_c: f64) -> bool {
    temperature_c >= 45.0
}

/// A thermal budget: max temperature + hysteresis band.
#[derive(Debug, Clone)]
pub struct ThermalBudget {
    pub max_temp_c: f64,
    /// Hysteresis: temperature must drop below (max - hysteresis) to clear.
    pub hysteresis_c: f64,
}

impl ThermalBudget {
    pub fn in_budget(&self, temperature_c: f64) -> bool {
        temperature_c <= self.max_temp_c
    }

    /// Whether a previously-throttled device has cooled enough to resume.
    pub fn cleared(&self, temperature_c: f64) -> bool {
        temperature_c <= self.max_temp_c - self.hysteresis_c
    }
}

/// Maps a temperature to a thermal state using thresholds.
pub fn classify(temperature_c: f64) -> ThermalState {
    if temperature_c >= 55.0 {
        ThermalState::Critical
    } else if temperature_c >= 50.0 {
        ThermalState::Throttle
    } else if temperature_c >= 45.0 {
        ThermalState::Warn
    } else {
        ThermalState::Nominal
    }
}

/// A thermal governor that drives the state machine.
#[derive(Debug, Clone)]
pub struct ThermalGovernor {
    current_c: f64,
    state: ThermalState,
    budget: ThermalBudget,
}

impl ThermalGovernor {
    pub fn new(budget: ThermalBudget) -> Self {
        Self {
            current_c: 25.0,
            state: ThermalState::Nominal,
            budget,
        }
    }

    /// Update with a new temperature reading and reclassify.
    pub fn update(&mut self, temperature_c: f64) -> ThermalState {
        self.current_c = temperature_c;
        self.state = classify(temperature_c);
        self.state
    }

    pub fn state(&self) -> ThermalState {
        self.state
    }

    pub fn temperature_c(&self) -> f64 {
        self.current_c
    }

    /// Whether the device is within the thermal budget.
    pub fn in_budget(&self) -> bool {
        self.budget.in_budget(self.current_c)
    }
}

/// Pick a thermal budget from a config profile (tighter for low power).
pub fn pick_thermal_budget(config: &Config) -> ThermalBudget {
    match config.profile {
        ConfigProfile::LowPower => ThermalBudget {
            max_temp_c: 45.0,
            hysteresis_c: 5.0,
        },
        ConfigProfile::Balanced => ThermalBudget {
            max_temp_c: 50.0,
            hysteresis_c: 5.0,
        },
        ConfigProfile::MaxThroughput => ThermalBudget {
            max_temp_c: 55.0,
            hysteresis_c: 5.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_crossing() {
        assert!(!thermal_throttle(44.0));
        assert!(thermal_throttle(45.0));
        assert!(thermal_throttle(50.0));
    }

    #[test]
    fn state_transitions() {
        assert_eq!(classify(25.0), ThermalState::Nominal);
        assert_eq!(classify(46.0), ThermalState::Warn);
        assert_eq!(classify(51.0), ThermalState::Throttle);
        assert_eq!(classify(56.0), ThermalState::Critical);
    }

    #[test]
    fn hysteresis_behavior() {
        let b = ThermalBudget {
            max_temp_c: 50.0,
            hysteresis_c: 5.0,
        };
        assert!(b.in_budget(50.0));
        assert!(!b.in_budget(51.0));
        // Cleared only below 45C.
        assert!(!b.cleared(46.0));
        assert!(b.cleared(44.0));
    }

    #[test]
    fn governor_updates_state() {
        let mut g = ThermalGovernor::new(ThermalBudget {
            max_temp_c: 50.0,
            hysteresis_c: 5.0,
        });
        assert_eq!(g.update(46.0), ThermalState::Warn);
        assert_eq!(g.update(51.0), ThermalState::Throttle);
        assert!(!g.in_budget());
        assert_eq!(g.state(), ThermalState::Throttle);
    }

    #[test]
    fn pick_budget_follows_profile() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let max = Config::for_profile(ConfigProfile::MaxThroughput);
        assert_eq!(pick_thermal_budget(&low).max_temp_c, 45.0);
        assert_eq!(pick_thermal_budget(&max).max_temp_c, 55.0);
    }

    #[test]
    fn overflow_safe_classification() {
        assert_eq!(classify(100.0), ThermalState::Critical);
        assert_eq!(classify(0.0), ThermalState::Nominal);
    }
}
