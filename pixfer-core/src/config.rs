//! Over-engineered configuration: a `Config` value object, named
//! `ConfigProfile`s, a `ConfigBuilder`, environment overrides, and strict
//! validation. This is the "configuration layer" that a production
//! light-link product would actually need.

use crate::transfer;

/// Throughput / behavior profiles. Each profile pre-sets throughput,
/// zip threshold, retry budget, and exposure preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigProfile {
    /// Conservative: lower throughput assumption, more retries.
    LowPower,
    /// The default, balanced profile.
    Balanced,
    /// Aggressive: higher throughput assumption, fewer retries, auto-exposure lock.
    MaxThroughput,
}

impl ConfigProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            ConfigProfile::LowPower => "low_power",
            ConfigProfile::Balanced => "balanced",
            ConfigProfile::MaxThroughput => "max_throughput",
        }
    }
}

/// The single source of truth for tunable link parameters.
#[derive(Debug, Clone)]
pub struct Config {
    pub profile: ConfigProfile,
    /// Effective bytes/sec assumption for transfer planning.
    pub default_throughput: f64,
    /// Zip if estimated seconds exceeds this.
    pub zip_threshold_s: f64,
    /// Max ARQ retransmissions before giving up.
    pub max_retries: u32,
    /// Default ACK/NAK round-trip timeout (ms).
    pub ack_timeout_ms: u64,
    /// Block pixel size for the 270x480 grid.
    pub block_px: u32,
    /// Whether to attempt a manual exposure lock on camera open.
    pub lock_exposure: bool,
    /// Minimum measured MB/s before raising a ThroughputLow.
    pub throughput_floor_mbps: f64,
    /// Camera capture resolution (width, height).
    pub camera_width: u32,
    pub camera_height: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            profile: ConfigProfile::Balanced,
            default_throughput: transfer::DEFAULT_THROUGHPUT,
            zip_threshold_s: transfer::ZIP_THRESHOLD_S,
            max_retries: 3,
            ack_timeout_ms: 2500,
            block_px: 4,
            lock_exposure: false,
            throughput_floor_mbps: 1.0,
            camera_width: 640,
            camera_height: 480,
        }
    }
}

impl Config {
    /// Build a config from a named profile.
    pub fn for_profile(profile: ConfigProfile) -> Self {
        let mut c = Config::default();
        c.profile = profile;
        match profile {
            ConfigProfile::LowPower => {
                c.default_throughput = 1_000_000.0;
                c.max_retries = 5;
                c.ack_timeout_ms = 4000;
                c.throughput_floor_mbps = 0.5;
            }
            ConfigProfile::Balanced => {
                // defaults
            }
            ConfigProfile::MaxThroughput => {
                c.default_throughput = 4_000_000.0;
                c.max_retries = 2;
                c.ack_timeout_ms = 1500;
                c.throughput_floor_mbps = 2.0;
                c.lock_exposure = true;
                c.camera_width = 640;
                c.camera_height = 480;
            }
        }
        c
    }

    /// Strict validation; returns a `Vec<String>` of problems (empty = valid).
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.default_throughput <= 0.0 {
            problems.push("default_throughput must be > 0".into());
        }
        if self.zip_threshold_s <= 0.0 {
            problems.push("zip_threshold_s must be > 0".into());
        }
        if self.max_retries == 0 {
            problems.push("max_retries must be >= 1".into());
        }
        if self.ack_timeout_ms == 0 {
            problems.push("ack_timeout_ms must be > 0".into());
        }
        if self.block_px == 0 {
            problems.push("block_px must be > 0".into());
        }
        if self.camera_width == 0 || self.camera_height == 0 {
            problems.push("camera dimensions must be > 0".into());
        }
        problems
    }

    /// Effective needs_zip using this config's throughput + threshold.
    pub fn needs_zip(&self, size: u64) -> bool {
        let est = (size as f64) / self.default_throughput;
        est > self.zip_threshold_s
    }

    pub fn estimated_seconds(&self, size: u64) -> f64 {
        (size as f64) / self.default_throughput
    }
}

/// Fluent builder mirroring the profile presets but allowing per-field
/// overrides — the classic "I added a builder for a struct with a Default"
/// flavor of over-engineering.
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    profile: Option<ConfigProfile>,
    default_throughput: Option<f64>,
    zip_threshold_s: Option<f64>,
    max_retries: Option<u32>,
    ack_timeout_ms: Option<u64>,
    block_px: Option<u32>,
    lock_exposure: Option<bool>,
    throughput_floor_mbps: Option<f64>,
    camera_width: Option<u32>,
    camera_height: Option<u32>,
}

impl ConfigBuilder {
    pub fn profile(mut self, p: ConfigProfile) -> Self {
        self.profile = Some(p);
        self
    }
    pub fn throughput(mut self, v: f64) -> Self {
        self.default_throughput = Some(v);
        self
    }
    pub fn zip_threshold(mut self, v: f64) -> Self {
        self.zip_threshold_s = Some(v);
        self
    }
    pub fn max_retries(mut self, v: u32) -> Self {
        self.max_retries = Some(v);
        self
    }
    pub fn ack_timeout(mut self, v: u64) -> Self {
        self.ack_timeout_ms = Some(v);
        self
    }
    pub fn block_px(mut self, v: u32) -> Self {
        self.block_px = Some(v);
        self
    }
    pub fn lock_exposure(mut self, v: bool) -> Self {
        self.lock_exposure = Some(v);
        self
    }
    pub fn throughput_floor(mut self, v: f64) -> Self {
        self.throughput_floor_mbps = Some(v);
        self
    }
    pub fn camera_resolution(mut self, w: u32, h: u32) -> Self {
        self.camera_width = Some(w);
        self.camera_height = Some(h);
        self
    }
    pub fn build(self) -> Config {
        let base = match self.profile {
            Some(p) => Config::for_profile(p),
            None => Config::default(),
        };
        Config {
            profile: self.profile.unwrap_or(base.profile),
            default_throughput: self.default_throughput.unwrap_or(base.default_throughput),
            zip_threshold_s: self.zip_threshold_s.unwrap_or(base.zip_threshold_s),
            max_retries: self.max_retries.unwrap_or(base.max_retries),
            ack_timeout_ms: self.ack_timeout_ms.unwrap_or(base.ack_timeout_ms),
            block_px: self.block_px.unwrap_or(base.block_px),
            lock_exposure: self.lock_exposure.unwrap_or(base.lock_exposure),
            throughput_floor_mbps: self.throughput_floor_mbps
                .unwrap_or(base.throughput_floor_mbps),
            camera_width: self.camera_width.unwrap_or(base.camera_width),
            camera_height: self.camera_height.unwrap_or(base.camera_height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_presets_differ() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let max = Config::for_profile(ConfigProfile::MaxThroughput);
        assert!(low.default_throughput < max.default_throughput);
        assert_eq!(max.lock_exposure, true);
        assert_eq!(low.max_retries, 5);
    }

    #[test]
    fn builder_overrides_profile() {
        let c = ConfigBuilder::default()
            .profile(ConfigProfile::MaxThroughput)
            .throughput(8_000_000.0)
            .build();
        assert_eq!(c.default_throughput, 8_000_000.0);
        assert_eq!(c.profile, ConfigProfile::MaxThroughput);
    }

    #[test]
    fn validation_catches_bads() {
        let mut c = Config::default();
        c.default_throughput = 0.0;
        c.max_retries = 0;
        let problems = c.validate();
        assert!(problems.len() >= 2);
        assert_eq!(Config::default().validate(), Vec::<String>::new());
    }

    #[test]
    fn needs_zip_respects_config() {
        let c = Config::for_profile(ConfigProfile::MaxThroughput);
        // 4 MB/s and 30 s threshold -> 120 MB boundary
        assert!(!c.needs_zip(120_000_000));
        assert!(c.needs_zip(121_000_000));
    }
}
