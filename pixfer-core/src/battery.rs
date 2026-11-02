//! Battery model for mobile devices: drain-rate time-to-empty estimation and
//! a governor that forces the low_power profile on low battery. (Hilariously
//! over-engineered: a battery governor that just watches the battery.)

use crate::config::ConfigProfile;

/// A snapshot of the battery state.
#[derive(Debug, Clone)]
pub struct BatteryState {
    /// Charge level in percent [0, 100].
    pub level_pct: f64,
    pub charging: bool,
    /// Temperature in Celsius.
    pub temperature_c: f64,
    pub cycle_count: u32,
}

/// Estimates time-to-empty from a constant drain rate (mAh/s).
#[derive(Debug, Clone)]
pub struct BatteryModel {
    /// Current charge in mAh.
    charge_mah: f64,
    /// Total capacity in mAh.
    capacity_mah: f64,
    /// Drain rate in mAh/s.
    drain_rate_mah_s: f64,
}

impl BatteryModel {
    pub fn new(charge_mah: f64, capacity_mah: f64, drain_rate_mah_s: f64) -> Self {
        Self {
            charge_mah,
            capacity_mah,
            drain_rate_mah_s,
        }
    }

    /// Seconds until the battery empties at the current drain rate.
    pub fn time_to_empty_secs(&self) -> f64 {
        if self.drain_rate_mah_s <= 0.0 {
            return f64::INFINITY;
        }
        (self.charge_mah / self.drain_rate_mah_s).max(0.0)
    }

    pub fn charge_pct(&self) -> f64 {
        if self.capacity_mah <= 0.0 {
            return 0.0;
        }
        (self.charge_mah / self.capacity_mah) * 100.0
    }

    /// Current charge in mAh.
    pub fn charge_mah_abs(&self) -> f64 {
        self.charge_mah
    }

    /// Advance the model by `dt` seconds, draining the charge.
    pub fn advance(&mut self, dt_s: f64) {
        let drained = self.drain_rate_mah_s * dt_s;
        self.charge_mah = (self.charge_mah - drained).max(0.0);
    }
}

/// A governor that forces the low_power profile when the battery is low.
pub struct BatteryGovernor {
    state: BatteryState,
    low_threshold_pct: f64,
}

impl BatteryGovernor {
    pub fn new(state: BatteryState, low_threshold_pct: f64) -> Self {
        Self {
            state,
            low_threshold_pct,
        }
    }

    pub fn should_use_low_power(&self) -> bool {
        !self.state.charging && self.state.level_pct <= self.low_threshold_pct
    }

    pub fn profile(&self) -> ConfigProfile {
        if self.should_use_low_power() {
            ConfigProfile::LowPower
        } else {
            ConfigProfile::Balanced
        }
    }

    pub fn state(&self) -> &BatteryState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_to_empty_from_drain() {
        let m = BatteryModel::new(1500.0, 3000.0, 5.0);
        assert!((m.time_to_empty_secs() - 300.0).abs() < 1e-9);
    }

    #[test]
    fn zero_drain_is_infinite() {
        let m = BatteryModel::new(1500.0, 3000.0, 0.0);
        assert_eq!(m.time_to_empty_secs(), f64::INFINITY);
    }

    #[test]
    fn charge_pct() {
        let m = BatteryModel::new(1500.0, 3000.0, 5.0);
        assert!((m.charge_pct() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn advance_drains_charge() {
        let mut m = BatteryModel::new(100.0, 3000.0, 10.0);
        m.advance(5.0); // drain 50 mAh
        assert!((m.charge_mah_abs() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn low_battery_triggers_low_power() {
        let st = BatteryState {
            level_pct: 15.0,
            charging: false,
            temperature_c: 30.0,
            cycle_count: 120,
        };
        let g = BatteryGovernor::new(st, 20.0);
        assert!(g.should_use_low_power());
        assert_eq!(g.profile(), ConfigProfile::LowPower);
    }

    #[test]
    fn charging_does_not_trigger_low_power() {
        let st = BatteryState {
            level_pct: 10.0,
            charging: true,
            temperature_c: 30.0,
            cycle_count: 50,
        };
        let g = BatteryGovernor::new(st, 20.0);
        assert!(!g.should_use_low_power());
        assert_eq!(g.profile(), ConfigProfile::Balanced);
    }

    #[test]
    fn charge_mah_abs_accessor() {
        let m = BatteryModel::new(42.0, 3000.0, 1.0);
        assert!((m.charge_mah_abs() - 42.0).abs() < 1e-9);
    }
}
