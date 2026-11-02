//! Exposure controller: keeps the receiving camera's exposure stable so
//! the 8-color palette doesn't drift. (Hilariously over-engineered: an
//! "exposure governor" for a camera that can only see 8 colors.)

/// The exposure state of the receiving camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExposureReading {
    /// Current exposure value (camera units, 0..=255).
    pub value: u8,
    /// Whether exposure is locked (manual).
    pub locked: bool,
    /// Measured average luminance (0..=1).
    pub avg_luma: f64,
}

/// A target exposure point the controller tries to hold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExposureTarget {
    pub value: u8,
}

/// A simple PID-flavored exposure governor.
///
/// Drives the camera's exposure toward a target luminance by adjusting
/// the exposure value. (Hilariously over-engineered: a "control loop" for
/// a link whose whole "network" is one screen and one camera.)
#[derive(Debug)]
pub struct ExposureController {
    /// Target average luminance (0..=1).
    target_luma: f64,
    /// Proportional gain.
    kp: f64,
    /// Integral gain.
    ki: f64,
    /// Derivative gain.
    kd: f64,
    /// Integral accumulator.
    integral: f64,
    /// Previous error.
    prev_err: f64,
    has_prev: bool,
}

impl ExposureController {
    /// Create with target luminance and PID gains.
    pub fn new(target_luma: f64) -> Self {
        Self {
            target_luma: target_luma.clamp(0.0, 1.0),
            kp: 128.0,
            ki: 2.0,
            kd: 4.0,
            integral: 0.0,
            prev_err: 0.0,
            has_prev: false,
        }
    }

    /// Compute the next exposure value (0..=255) from a reading.
    pub fn next_exposure(&mut self, r: &ExposureReading) -> u8 {
        if r.locked {
            return r.value;
        }
        let err = self.target_luma - r.avg_luma;
        self.integral = (self.integral + err).clamp(-10.0, 10.0);
        let deriv = if self.has_prev {
            err - self.prev_err
        } else {
            0.0
        };
        self.prev_err = err;
        self.has_prev = true;
        let out = self.kp * err + self.ki * self.integral + self.kd * deriv;
        let next = (r.value as f64) + out;
        (next.round().clamp(0.0, 255.0)) as u8
    }

    /// The target luminance this controller holds.
    pub fn target_luma(&self) -> f64 {
        self.target_luma
    }

    /// Reset the integral + derivative state (e.g. on link re-acquire).
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_err = 0.0;
        self.has_prev = false;
    }
}

/// A convenience: lock exposure to a fixed value (the "it depends on your
/// profile" flavor of over-engineering).
pub fn locked_exposure(value: u8) -> ExposureReading {
    ExposureReading {
        value,
        locked: true,
        avg_luma: 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(v: u8, luma: f64) -> ExposureReading {
        ExposureReading {
            value: v,
            locked: false,
            avg_luma: luma,
        }
    }

    #[test]
    fn locked_reading_is_passthrough() {
        let mut c = ExposureController::new(0.5);
        let r = locked_exposure(128);
        assert_eq!(c.next_exposure(&r), 128);
    }

    #[test]
    fn luma_below_target_raises_exposure() {
        let mut c = ExposureController::new(0.5);
        let r = reading(64, 0.2);
        let next = c.next_exposure(&r);
        assert!(next > 64, "should open the aperture: {next}");
    }

    #[test]
    fn luma_above_target_lowers_exposure() {
        let mut c = ExposureController::new(0.5);
        let r = reading(200, 0.8);
        let next = c.next_exposure(&r);
        assert!(next < 200, "should close the aperture: {next}");
    }

    #[test]
    fn target_luma_is_preserved() {
        let c = ExposureController::new(0.3);
        assert!((c.target_luma() - 0.3).abs() < 1e-9);
    }

    #[test]
    fn reset_clears_integral() {
        let mut c = ExposureController::new(0.5);
        c.next_exposure(&reading(100, 0.2));
        c.reset();
        // After reset, the integral and derivative state is zeroed.
        let next = c.next_exposure(&reading(100, 0.2));
        assert!(next > 100);
    }

    #[test]
    fn output_stays_in_range() {
        let mut c = ExposureController::new(0.9);
        let r = reading(250, 0.1);
        let next = c.next_exposure(&r);
        assert_eq!(next, 255);
        let r2 = reading(5, 0.95);
        let next2 = c.next_exposure(&r2);
        assert_eq!(next2, 0);
    }
}
