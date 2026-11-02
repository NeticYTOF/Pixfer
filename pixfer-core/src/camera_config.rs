//! Camera config: capture resolution, fps, and exposure preferences chosen
//! from a `ConfigProfile`. (Hilariously over-engineered: a "camera config"
//! for a phone whose camera is the receiver.)

use crate::config::{Config, ConfigProfile};

/// A named camera configuration.
#[derive(Debug, Clone)]
pub struct CameraConfig {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Attempt a manual exposure lock.
    pub lock_exposure: bool,
}

/// Pick a camera config from a config profile.
pub fn pick_camera_config(config: &Config) -> CameraConfig {
    match config.profile {
        ConfigProfile::LowPower => CameraConfig {
            name: "low_power",
            width: 320,
            height: 240,
            fps: 15,
            lock_exposure: false,
        },
        ConfigProfile::Balanced => CameraConfig {
            name: "balanced",
            width: 640,
            height: 480,
            fps: 30,
            lock_exposure: false,
        },
        ConfigProfile::MaxThroughput => CameraConfig {
            name: "max_throughput",
            width: 640,
            height: 480,
            fps: 60,
            lock_exposure: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_per_profile() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let max = Config::for_profile(ConfigProfile::MaxThroughput);
        let cl = pick_camera_config(&low);
        let cm = pick_camera_config(&max);
        assert_eq!(cl.fps, 15);
        assert_eq!(cm.fps, 60);
        assert!(cm.lock_exposure);
        assert!(!cl.lock_exposure);
    }

    #[test]
    fn balanced_resolution() {
        let bal = Config::for_profile(ConfigProfile::Balanced);
        let c = pick_camera_config(&bal);
        assert_eq!((c.width, c.height), (640, 480));
    }
}
