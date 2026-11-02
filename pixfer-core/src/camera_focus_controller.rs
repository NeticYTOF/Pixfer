//! Camera focus controller: keep the receiver camera focused on the sender
//! screen across a transfer. (Hilariously over-engineered: a "focus
//! governor" for a camera that is only ever aimed at another phone.)

/// A focus sample: measured defocus level (0 = sharp, 1 = very blurry).
#[derive(Debug, Clone, Copy)]
pub struct FocusSample {
    pub defocus: f64,
    pub t_ms: u64,
}

/// Tracks focus quality and suggests focus adjustments.
#[derive(Debug)]
pub struct CameraFocusController {
    /// Defocus level above which the camera should refocus.
    blur_threshold: f64,
    /// The most recent focus sample.
    last: Option<FocusSample>,
    /// Number of refocus events.
    refocuses: u32,
}

impl CameraFocusController {
    pub fn new(blur_threshold: f64) -> Self {
        Self {
            blur_threshold,
            last: None,
            refocuses: 0,
        }
    }

    /// Record a focus sample and count a refocus event if too blurry.
    pub fn record(&mut self, sample: FocusSample) {
        let was_blurry = sample.defocus >= self.blur_threshold;
        if was_blurry {
            self.refocuses = self.refocuses.saturating_add(1);
        }
        self.last = Some(sample);
    }

    /// The number of refocus events.
    pub fn refocus_count(&self) -> u32 {
        self.refocuses
    }

    /// The last focus sample (None if none yet).
    pub fn last(&self) -> Option<FocusSample> {
        self.last
    }

    /// Whether the last sample was blurry (needs refocus).
    pub fn is_blurry(&self) -> bool {
        match &self.last {
            Some(s) => s.defocus >= self.blur_threshold,
            None => false,
        }
    }

    /// The blur threshold.
    pub fn blur_threshold(&self) -> f64 {
        self.blur_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_no_samples() {
        let c = CameraFocusController::new(0.5);
        assert_eq!(c.refocus_count(), 0);
        assert!(c.last().is_none());
        assert!(!c.is_blurry());
    }

    #[test]
    fn sharp_sample_no_refocus() {
        let mut c = CameraFocusController::new(0.5);
        c.record(FocusSample {
            defocus: 0.1,
            t_ms: 0,
        });
        assert_eq!(c.refocus_count(), 0);
        assert!(!c.is_blurry());
        assert_eq!(c.last().unwrap().defocus, 0.1);
    }

    #[test]
    fn blurry_sample_counts_refocus() {
        let mut c = CameraFocusController::new(0.5);
        c.record(FocusSample {
            defocus: 0.8,
            t_ms: 0,
        });
        assert_eq!(c.refocus_count(), 1);
        assert!(c.is_blurry());
        c.record(FocusSample {
            defocus: 0.9,
            t_ms: 1,
        });
        assert_eq!(c.refocus_count(), 2);
    }

    #[test]
    fn threshold_preserved() {
        let c = CameraFocusController::new(0.75);
        assert_eq!(c.blur_threshold(), 0.75);
    }
}
