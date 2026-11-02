//! Power-profile / energy-budget module for the light link. (Hilariously
//! over-engineered: a power governor for a phone whose screen is the only
//! transmitter.)

use crate::config::{Config, ConfigProfile};

/// A named power profile: brightness, camera fps, expected and budget mA.
#[derive(Debug, Clone)]
pub struct PowerProfile {
    pub name: &'static str,
    /// Screen brightness 0.0..=1.0.
    pub screen_brightness: f64,
    /// Camera capture rate in fps.
    pub camera_fps: u32,
    /// Expected steady-state draw in mA.
    pub expected_ma: f64,
    /// The energy budget in mAh.
    pub budget_mah: f64,
}

/// Tracks energy used (mAh) against a budget.
#[derive(Debug, Clone)]
pub struct PowerBudget {
    used_mah: f64,
    budget_mah: f64,
}

impl PowerBudget {
    pub fn new(budget_mah: f64) -> Self {
        Self {
            used_mah: 0.0,
            budget_mah,
        }
    }

    pub fn add(&mut self, delta_mah: f64) {
        self.used_mah = self.used_mah + delta_mah;
    }

    pub fn within_budget(&self) -> bool {
        self.used_mah <= self.budget_mah
    }

    pub fn remaining_mah(&self) -> f64 {
        (self.budget_mah - self.used_mah).max(0.0)
    }

    pub fn used(&self) -> f64 {
        self.used_mah
    }

    pub fn budget(&self) -> f64 {
        self.budget_mah
    }
}

/// A power report: used, budget, ok, remaining.
#[derive(Debug, Clone)]
pub struct PowerReport {
    pub used_mah: f64,
    pub budget_mah: f64,
    pub ok: bool,
    pub remaining_mah: f64,
}

/// Picks a power profile from a `Config`/`ConfigProfile` (low_power ->
/// lowest brightness/fps).
pub fn pick_power_profile(config: &Config) -> PowerProfile {
    match config.profile {
        ConfigProfile::LowPower => PowerProfile {
            name: "low_power",
            screen_brightness: 0.3,
            camera_fps: 15,
            expected_ma: 180.0,
            budget_mah: 3000.0,
        },
        ConfigProfile::Balanced => PowerProfile {
            name: "balanced",
            screen_brightness: 0.6,
            camera_fps: 30,
            expected_ma: 320.0,
            budget_mah: 3000.0,
        },
        ConfigProfile::MaxThroughput => PowerProfile {
            name: "max_throughput",
            screen_brightness: 1.0,
            camera_fps: 60,
            expected_ma: 520.0,
            budget_mah: 3000.0,
        },
    }
}

/// A governor that, given a `PowerBudget`, reports power state.
pub struct PowerGovernor {
    profile: PowerProfile,
    budget: PowerBudget,
}

impl PowerGovernor {
    pub fn new(profile: PowerProfile, budget: PowerBudget) -> Self {
        Self { profile, budget }
    }

    pub fn report(&self) -> PowerReport {
        PowerReport {
            used_mah: self.budget.used(),
            budget_mah: self.budget.budget(),
            ok: self.budget.within_budget(),
            remaining_mah: self.budget.remaining_mah(),
        }
    }

    pub fn profile(&self) -> &PowerProfile {
        &self.profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_profile_follows_config() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let max = Config::for_profile(ConfigProfile::MaxThroughput);
        let pl = pick_power_profile(&low);
        let pm = pick_power_profile(&max);
        assert_eq!(pl.name, "low_power");
        assert_eq!(pm.name, "max_throughput");
        assert!(pl.camera_fps < pm.camera_fps);
        assert!(pl.screen_brightness < pm.screen_brightness);
    }

    #[test]
    fn budget_tracking() {
        let mut b = PowerBudget::new(100.0);
        b.add(40.0);
        assert!((b.used() - 40.0).abs() < 1e-9);
        assert!(b.within_budget());
        assert!((b.remaining_mah() - 60.0).abs() < 1e-9);
        b.add(70.0);
        assert!(!b.within_budget());
        assert!((b.remaining_mah()).abs() < 1e-9); // clamped to 0
    }

    #[test]
    fn governor_report() {
        let prof = pick_power_profile(&Config::for_profile(ConfigProfile::Balanced));
        let budget = PowerBudget::new(prof.budget_mah);
        let g = PowerGovernor::new(prof.clone(), budget);
        let r = g.report();
        assert!(r.ok);
        assert_eq!(r.budget_mah, prof.budget_mah);
    }

    #[test]
    fn budget_overflow_safe() {
        let mut b = PowerBudget::new(1000.0);
        b.add(2000.0);
        assert!(!b.within_budget());
        assert!((b.remaining_mah()).abs() < 1e-9);
    }
}
