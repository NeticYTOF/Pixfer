//! RX exposure lock: keep the receiver camera's exposure steady during a
//! transfer so the 8-color palette stays stable. (Hilariously over-
//! engineered: an "exposure governor" for a camera that is literally just
//! looking at another phone's screen.)

/// A target exposure value (0..=255) the receiver should lock to.
pub type ExposureLockTarget = u8;

/// Locks the receiver camera's exposure to a stable value for the duration
/// of a transfer.
#[derive(Debug)]
pub struct RxCameraExposureLock {
    locked: bool,
    target: u8,
}

impl RxCameraExposureLock {
    pub fn new() -> Self {
        Self {
            locked: false,
            target: 128,
        }
    }

    /// Lock exposure to `target`.
    pub fn lock(&mut self, target: u8) {
        self.target = target;
        self.locked = true;
    }

    /// Release the lock (camera returns to auto-exposure).
    pub fn unlock(&mut self) {
        self.locked = false;
    }

    /// Whether the lock is active.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// The locked exposure value.
    pub fn target(&self) -> u8 {
        self.target
    }
}

/// Compute a target exposure from a measured scene luminance.
///
/// The model: exposure inversely proportional to scene brightness, clamped
/// to 0..=255.
pub fn suggest_exposure(scene_luma: f64) -> u8 {
    // Map luma (0..=255 scaled) to exposure: brighter scene -> lower
    // exposure. Simple inverse model with a floor.
    let luma = scene_luma.clamp(1.0, 255.0);
    // exposure = k / luma, scaled to 0..=255.
    let raw = 40000.0 / luma;
    (raw as u32).clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_unlocked() {
        let l = RxCameraExposureLock::new();
        assert!(!l.is_locked());
    }

    #[test]
    fn lock_sets_target_and_flag() {
        let mut l = RxCameraExposureLock::new();
        l.lock(64);
        assert!(l.is_locked());
        assert_eq!(l.target(), 64);
    }

    #[test]
    fn unlock_clears_flag_keeps_target() {
        let mut l = RxCameraExposureLock::new();
        l.lock(32);
        l.unlock();
        assert!(!l.is_locked());
        assert_eq!(l.target(), 32);
    }

    #[test]
    fn dark_scene_gets_higher_exposure() {
        assert!(suggest_exposure(20.0) > suggest_exposure(200.0));
    }

    #[test]
    fn luma_floor_prevents_divide_by_zero() {
        assert_eq!(suggest_exposure(0.0), 255);
        assert_eq!(suggest_exposure(255.0), 156); // 40000/255 = 156.86 -> 156
    }

    #[test]
    fn exposure_stays_in_range() {
        for luma in [0.0, 1.0, 10.0, 128.0, 255.0] {
            let e = suggest_exposure(luma);
            assert!((e as u32) <= 255);
        }
    }
}
