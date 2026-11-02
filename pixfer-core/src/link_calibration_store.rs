//! Link calibration store: persist and retrieve per-link calibration
//! values (exposure, white-balance, color-temperature). (Hilariously
//! over-engineered: a "calibration database" for a link that can only
//! see 8 colors.)

use std::collections::HashMap;

/// A set of calibration values for one link.
#[derive(Debug, Clone)]
pub struct CalibrationValues {
    /// The link's color-temperature target (Kelvin).
    pub target_kelvin: u32,
    /// A per-channel gain vector (r, g, b).
    pub gains: [f64; 3],
    /// The exposure lock value (0..=255).
    pub exposure: u8,
}

/// A store of calibration values keyed by link fingerprint.
#[derive(Debug, Default)]
pub struct CalibrationStore {
    by_link: HashMap<u64, CalibrationValues>,
}

impl CalibrationStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store calibration values for a link.
    pub fn put(&mut self, fingerprint: u64, values: CalibrationValues) {
        self.by_link.insert(fingerprint, values);
    }

    /// Retrieve calibration values for a link (None if absent).
    pub fn get(&self, fingerprint: u64) -> Option<&CalibrationValues> {
        self.by_link.get(&fingerprint)
    }

    /// Whether calibration values exist for a link.
    pub fn contains(&self, fingerprint: u64) -> bool {
        self.by_link.contains_key(&fingerprint)
    }

    /// The number of calibrated links.
    pub fn len(&self) -> usize {
        self.by_link.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_link.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> CalibrationValues {
        CalibrationValues {
            target_kelvin: 5500,
            gains: [1.0, 1.0, 1.0],
            exposure: 128,
        }
    }

    #[test]
    fn put_get_roundtrip() {
        let mut s = CalibrationStore::new();
        s.put(0xABCDEF, values());
        let v = s.get(0xABCDEF).expect("present");
        assert_eq!(v.target_kelvin, 5500);
        assert_eq!(v.exposure, 128);
        assert!(s.contains(0xABCDEF));
    }

    #[test]
    fn absent_returns_none() {
        let s = CalibrationStore::new();
        assert!(s.get(42).is_none());
        assert!(!s.contains(42));
    }

    #[test]
    fn len_and_is_empty() {
        let mut s = CalibrationStore::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        s.put(1, values());
        s.put(2, values());
        assert_eq!(s.len(), 2);
        assert!(!s.is_empty());
    }

    #[test]
    fn overwrite_existing() {
        let mut s = CalibrationStore::new();
        s.put(7, values());
        let mut v = values();
        v.target_kelvin = 6500;
        s.put(7, v);
        assert_eq!(s.get(7).unwrap().target_kelvin, 6500);
        assert_eq!(s.len(), 1);
    }
}
