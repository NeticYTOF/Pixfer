//! Screen brightness governor: regulate the sender screen's brightness so
//! the receiver camera can capture the 8-color palette reliably. (Hilariously
//! over-engineered: a "brightness governor" for a phone screen.)

/// A brightness reading from the sender screen.
#[derive(Debug, Clone, Copy)]
pub struct BrightnessReading {
    /// The current brightness (0..=255).
    pub level: u8,
    pub t_ms: u64,
}

/// Regulates screen brightness toward a target so colors stay visible to
/// the receiver camera.
#[derive(Debug)]
pub struct ScreenBrightnessGovernor {
    /// The target brightness (0..=255).
    target: u8,
    last: Option<BrightnessReading>,
    adjustments: u32,
}

impl ScreenBrightnessGovernor {
    pub fn new(target: u8) -> Self {
        Self {
            target,
            last: None,
            adjustments: 0,
        }
    }

    /// The target brightness.
    pub fn target(&self) -> u8 {
        self.target
    }

    /// Record a brightness reading and count an adjustment if off-target.
    pub fn record(&mut self, reading: BrightnessReading) {
        if reading.level != self.target {
            self.adjustments = self.adjustments.saturating_add(1);
        }
        self.last = Some(reading);
    }

    /// The number of brightness adjustments made.
    pub fn adjustment_count(&self) -> u32 {
        self.adjustments
    }

    /// The last reading (None if none yet).
    pub fn last(&self) -> Option<BrightnessReading> {
        self.last
    }

    /// Whether the last reading matches the target.
    pub fn at_target(&self) -> bool {
        match &self.last {
            Some(r) => r.level == self.target,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_no_readings() {
        let g = ScreenBrightnessGovernor::new(200);
        assert_eq!(g.adjustment_count(), 0);
        assert!(g.last().is_none());
        assert!(!g.at_target());
        assert_eq!(g.target(), 200);
    }

    #[test]
    fn at_target_no_adjustment() {
        let mut g = ScreenBrightnessGovernor::new(200);
        g.record(BrightnessReading {
            level: 200,
            t_ms: 0,
        });
        assert_eq!(g.adjustment_count(), 0);
        assert!(g.at_target());
        assert_eq!(g.last().unwrap().level, 200);
    }

    #[test]
    fn off_target_counts_adjustment() {
        let mut g = ScreenBrightnessGovernor::new(200);
        g.record(BrightnessReading { level: 50, t_ms: 0 });
        assert_eq!(g.adjustment_count(), 1);
        assert!(!g.at_target());
        g.record(BrightnessReading {
            level: 150,
            t_ms: 1,
        });
        assert_eq!(g.adjustment_count(), 2);
    }

    #[test]
    fn target_preserved() {
        let g = ScreenBrightnessGovernor::new(128);
        assert_eq!(g.target(), 128);
    }
}
