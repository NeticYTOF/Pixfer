//! Camera/display calibration: exposure offset, block-pixel drift detection,
//! and named calibration profiles in a registry. (Hilariously over-engineered:
//! a full calibration layer for a light link.)

use std::collections::BTreeMap;

use crate::config::Config;
use crate::rs::{RS_N, RS_T};

/// A calibrated camera/display parameter set.
#[derive(Debug, Clone)]
pub struct Calibration {
    pub exposure_offset: f64,
    pub block_px: u32,
    pub finder_tolerance_px: u32,
    pub rs_t: usize,
    pub calibrated_at_ms: u64,
}

impl Calibration {
    pub fn from_config(config: &Config) -> Self {
        Self {
            exposure_offset: 0.0,
            block_px: config.block_px,
            finder_tolerance_px: 1,
            rs_t: RS_T,
            calibrated_at_ms: 0,
        }
    }

    /// Relative drift of a measured block size vs. the calibrated size (percent).
    pub fn drift_pct(&self, measured_block_px: u32) -> f64 {
        if measured_block_px == 0 || self.block_px == 0 {
            return 0.0;
        }
        ((measured_block_px as f64) / (self.block_px as f64) - 1.0) * 100.0
    }

    /// Apply the exposure offset to a raw exposure value.
    pub fn apply_exposure(&self, raw: f64) -> f64 {
        raw + self.exposure_offset
    }

    /// Strict validation.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.block_px == 0 {
            problems.push("block_px must be > 0".into());
        }
        if 2 * self.rs_t + 1 > RS_N {
            problems.push("rs_t too large for RS_N".into());
        }
        problems
    }

    /// A JSON-ish report.
    pub fn report(&self) -> String {
        format!(
            "{{\"exposure_offset\": {}, \"block_px\": {}, \"finder_tolerance_px\": {}, \"rs_t\": {}}}",
            self.exposure_offset, self.block_px, self.finder_tolerance_px, self.rs_t
        )
    }
}

/// A named calibration profile.
#[derive(Debug, Clone)]
pub struct CalibrationProfile {
    pub name: &'static str,
    pub calibration: Calibration,
}

/// A registry of named calibration profiles with a "current" pointer.
#[derive(Default)]
pub struct CalibrationRegistry {
    profiles: BTreeMap<&'static str, CalibrationProfile>,
    current: Option<&'static str>,
}

impl CalibrationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, profile: CalibrationProfile) {
        self.profiles.insert(profile.name, profile);
    }

    pub fn get(&self, name: &str) -> Option<&Calibration> {
        self.profiles.get(name).map(|p| &p.calibration)
    }

    pub fn set_current(&mut self, name: &str) -> bool {
        if let Some(p) = self.profiles.values().find(|p| p.name == name) {
            self.current = Some(p.name);
            true
        } else {
            false
        }
    }

    pub fn current(&self) -> Option<&Calibration> {
        self.current.and_then(|n| self.get(n))
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.profiles.keys().copied().collect()
    }

    pub fn report(&self) -> String {
        let items: Vec<String> = self
            .profiles
            .values()
            .map(|p| format!("  \"{}\": {}", p.name, p.calibration.report()))
            .collect();
        let mut s = String::from("{\n");
        s.push_str(&items.join(",\n"));
        s.push_str("\n}");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::default()
    }

    #[test]
    fn from_config_picks_up_block_px() {
        let c = Calibration::from_config(&cfg());
        assert_eq!(c.block_px, cfg().block_px);
        assert_eq!(c.rs_t, RS_T);
        assert_eq!(c.exposure_offset, 0.0);
    }

    #[test]
    fn drift_pct_math() {
        let c = Calibration::from_config(&cfg()); // block_px = 4 (default)
                                                  // Measured 5px vs calibrated 4px = +25%
        assert!((c.drift_pct(5) - 25.0).abs() < 1e-6);
        assert!((c.drift_pct(2) - (-50.0)).abs() < 1e-6);
        assert_eq!(c.drift_pct(0), 0.0);
    }

    #[test]
    fn apply_exposure_adds_offset() {
        let mut c = Calibration::from_config(&cfg());
        c.exposure_offset = 12.5;
        assert!((c.apply_exposure(100.0) - 112.5).abs() < 1e-6);
    }

    #[test]
    fn validation_catches_bad_values() {
        let mut c = Calibration::from_config(&cfg());
        c.block_px = 0;
        c.rs_t = RS_N; // 2*255+1 > 255
        let problems = c.validate();
        assert!(problems.iter().any(|p| p.contains("block_px")));
        assert!(problems.iter().any(|p| p.contains("rs_t")));
        assert!(Calibration::from_config(&cfg()).validate().is_empty());
    }

    #[test]
    fn registry_tracks_current_profile() {
        let mut r = CalibrationRegistry::new();
        let mut base = Calibration::from_config(&cfg());
        base.exposure_offset = 5.0;
        r.register(CalibrationProfile {
            name: "office",
            calibration: base,
        });
        let mut dim = Calibration::from_config(&cfg());
        dim.exposure_offset = -3.0;
        r.register(CalibrationProfile {
            name: "dim_room",
            calibration: dim,
        });
        assert!(r.set_current("office"));
        assert!(!r.set_current("nope"));
        assert_eq!(r.current().unwrap().exposure_offset, 5.0);
        assert_eq!(r.names(), vec!["dim_room", "office"]);
        let rep = r.report();
        assert!(rep.contains("\"office\""));
        assert!(rep.contains("\"dim_room\""));
    }

    #[test]
    fn report_shape() {
        let c = Calibration::from_config(&cfg());
        let r = c.report();
        assert!(r.starts_with("{"));
        assert!(r.contains("\"block_px\""));
        assert!(r.contains("\"rs_t\""));
    }
}
