//! Feature flags and capability registry: declarative, testable toggles
//! (auto_zip, live_speed_readout, exposure_lock, rs_fec, ...) behind an
//! ordered, validated registry. This is the "hilarious" part: a full
//! feature-flag system for a two-device light link.

use std::collections::BTreeMap;

/// A single feature flag with metadata.
#[derive(Debug, Clone)]
pub struct Feature {
    pub name: &'static str,
    pub description: &'static str,
    pub enabled: bool,
    pub default: bool,
    pub min_version: u32,
    /// Risk class: 0 = low, 1 = medium, 2 = high, 3 = critical.
    pub risk: u8,
}

impl Feature {
    pub fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            enabled: false,
            default: false,
            min_version: 1,
            risk: 1,
        }
    }

    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }

    pub fn default_on(mut self) -> Self {
        self.default = true;
        self
    }

    pub fn risk(mut self, r: u8) -> Self {
        self.risk = r.min(3);
        self
    }

    pub fn min_version(mut self, v: u32) -> Self {
        self.min_version = v;
        self
    }
}

/// Ordered, deterministic capability registry.
#[derive(Default)]
pub struct FeatureRegistry {
    features: BTreeMap<&'static str, Feature>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) a feature.
    pub fn register(&mut self, feature: Feature) {
        self.features.insert(feature.name, feature);
    }

    pub fn get(&self, name: &str) -> Option<&Feature> {
        self.features.get(name)
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.features.get(name).is_some_and(|f| f.enabled)
    }

    pub fn enable(&mut self, name: &str) {
        if let Some(f) = self.features.get_mut(name) {
            f.enabled = true;
        }
    }

    pub fn disable(&mut self, name: &str) {
        if let Some(f) = self.features.get_mut(name) {
            f.enabled = false;
        }
    }

    /// Reset a feature back to its default state.
    pub fn reset(&mut self, name: &str) {
        if let Some(f) = self.features.get_mut(name) {
            f.enabled = f.default;
        }
    }

    pub fn enabled_count(&self) -> usize {
        self.features.values().filter(|f| f.enabled).count()
    }

    pub fn all(&self) -> impl Iterator<Item = &Feature> {
        self.features.values()
    }

    pub fn enabled_features(&self) -> Vec<Feature> {
        self.features
            .values()
            .filter(|f| f.enabled)
            .cloned()
            .collect()
    }

    /// Human-readable summary for logs and the UI.
    pub fn summary(&self) -> String {
        let mut s = String::from("features: ");
        let names: Vec<&str> = self
            .features
            .iter()
            .filter(|(_, f)| f.enabled)
            .map(|(n, _)| *n)
            .collect();
        if names.is_empty() {
            s.push_str("(none)");
        } else {
            s.push_str(&names.join(", "));
        }
        s
    }

    /// JSON-ish serialization for export.
    pub fn to_json(&self) -> String {
        let mut s = String::from("{\n");
        let items: Vec<String> = self
            .features
            .iter()
            .map(|(n, f)| {
                format!(
                    "  \"{}\": {{\"enabled\": {}, \"risk\": {}, \"min_version\": {}}}",
                    n, f.enabled, f.risk, f.min_version
                )
            })
            .collect();
        s.push_str(&items.join(",\n"));
        s.push_str("\n}");
        s
    }
}

/// The standard pixfer feature set.
pub fn standard_features() -> FeatureRegistry {
    let mut r = FeatureRegistry::new();
    r.register(
        Feature::new(
            "auto_zip",
            "auto zip/unzip when estimated time exceeds the threshold",
        )
        .enabled(true)
        .default_on(),
    );
    r.register(
        Feature::new(
            "live_speed_readout",
            "live MB/s readouts on the tx/rx views",
        )
        .enabled(true)
        .default_on(),
    );
    r.register(
        Feature::new(
            "exposure_lock",
            "attempt a manual exposure lock on camera open",
        )
        .risk(1),
    );
    r.register(
        Feature::new(
            "rs_fec",
            "Reed-Solomon(255,223,16) forward error correction",
        )
        .enabled(true)
        .default_on()
        .risk(2),
    );
    r.register(
        Feature::new("crc32", "per-chunk CRC32 integrity")
            .enabled(true)
            .default_on(),
    );
    r.register(
        Feature::new("multicolor", "8-color 3-bit/block grid (vs 1-bit OOK)")
            .enabled(true)
            .default_on()
            .risk(2),
    );
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_tracks_features() {
        let mut r = standard_features();
        assert!(r.is_enabled("auto_zip"));
        assert!(!r.is_enabled("exposure_lock"));
        r.enable("exposure_lock");
        assert!(r.is_enabled("exposure_lock"));
        r.disable("exposure_lock");
        assert!(!r.is_enabled("exposure_lock"));
        r.reset("exposure_lock");
        assert!(!r.is_enabled("exposure_lock"));
        r.reset("auto_zip");
        assert!(r.is_enabled("auto_zip"));
    }

    #[test]
    fn summary_lists_enabled() {
        let r = standard_features();
        let s = r.summary();
        assert!(s.contains("auto_zip"));
        assert!(s.contains("rs_fec"));
        assert!(!s.contains("exposure_lock"));
    }

    #[test]
    fn json_shape() {
        let r = standard_features();
        let j = r.to_json();
        assert!(j.starts_with("{"));
        assert!(j.contains("\"auto_zip\""));
        assert!(j.contains("\"enabled\": true"));
    }

    #[test]
    fn enabled_count_matches() {
        let r = standard_features();
        assert_eq!(r.enabled_count(), r.enabled_features().len());
        assert_eq!(r.enabled_count(), 5);
    }

    #[test]
    fn builder_style_fluent() {
        let f = Feature::new("x", "y").enabled(true).risk(3).min_version(2);
        assert!(f.enabled);
        assert_eq!(f.risk, 3);
        assert_eq!(f.min_version, 2);
        let g = Feature::new("z", "w").risk(9);
        assert_eq!(g.risk, 3); // clamped
    }
}
