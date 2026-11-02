//! Color temperature correction: compensate for warm/cool drift between
//! the sender's screen and the receiver's camera white-balance.
//! (Hilariously over-engineered: a "white-balance" layer for a link
//! that can only see 8 colors.)

use crate::color_palette::{color_for_value, Color};

/// A color-temperature reading (Kelvin-ish).
#[derive(Debug, Clone, Copy)]
pub struct TemperatureReading {
    /// Measured color temperature in Kelvin.
    pub kelvin: u32,
    /// Whether the reading is valid.
    pub valid: bool,
}

/// A white-balance corrector that shifts the 8-color palette to
/// compensate for a color-temperature offset.
#[derive(Debug)]
pub struct TemperatureCorrector {
    /// Target color temperature (Kelvin).
    target_kelvin: u32,
    /// Per-channel gain multipliers (r,g,b) applied to palette colors.
    gain_r: f64,
    gain_g: f64,
    gain_b: f64,
}

impl TemperatureCorrector {
    /// Create a corrector targeting `target_kelvin`.
    pub fn new(target_kelvin: u32) -> Self {
        Self {
            target_kelvin,
            gain_r: 1.0,
            gain_g: 1.0,
            gain_b: 1.0,
        }
    }

    /// Adjust the gains to compensate for a measured temperature.
    ///
    /// Warmer-than-target readings scale down blue and up red; cooler
    /// readings do the opposite.
    pub fn compensate(&mut self, reading: &TemperatureReading) {
        if !reading.valid || reading.kelvin == 0 {
            return;
        }
        // Normalized deviation from target, clamped to [-1, 1].
        let dev = ((reading.kelvin as f64) - (self.target_kelvin as f64)) / 1000.0;
        let d = dev.clamp(-1.0, 1.0);
        // Warm (positive deviation): boost red, cut blue. Cool: the reverse.
        self.gain_r = (1.0 + 0.2 * d).clamp(0.5, 1.5);
        self.gain_g = 1.0; // green is the neutral anchor.
        self.gain_b = (1.0 - 0.2 * d).clamp(0.5, 1.5);
    }

    /// The corrected 8-color palette after compensation.
    pub fn corrected_palette(&self) -> [Color; 8] {
        let mut out = [Color::rgb(0, 0, 0); 8];
        for v in 0..8u8 {
            let c = color_for_value(v);
            out[v as usize & 0b111] = Color::rgb(
                (c.r as f64 * self.gain_r).round().clamp(0.0, 255.0) as u8,
                (c.g as f64 * self.gain_g).round().clamp(0.0, 255.0) as u8,
                (c.b as f64 * self.gain_b).round().clamp(0.0, 255.0) as u8,
            );
        }
        out
    }

    /// The current per-channel gains.
    pub fn gains(&self) -> (f64, f64, f64) {
        (self.gain_r, self.gain_g, self.gain_b)
    }

    /// The target temperature (Kelvin).
    pub fn target_kelvin(&self) -> u32 {
        self.target_kelvin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(kelvin: u32) -> TemperatureReading {
        TemperatureReading {
            kelvin,
            valid: true,
        }
    }

    #[test]
    fn neutral_reading_no_change() {
        let mut c = TemperatureCorrector::new(5500);
        c.compensate(&reading(5500));
        assert!((c.gains().0 - 1.0).abs() < 1e-9);
        assert!((c.gains().2 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn warm_reading_boosts_red_cuts_blue() {
        let mut c = TemperatureCorrector::new(5500);
        c.compensate(&reading(7000)); // 1500K warmer than target
        let (r, g, b) = c.gains();
        assert!(r > 1.0, "warm: red gain should rise: {r}");
        assert!(b < 1.0, "warm: blue gain should fall: {b}");
        assert!((g - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cool_reading_cuts_red_boosts_blue() {
        let mut c = TemperatureCorrector::new(5500);
        c.compensate(&reading(4000)); // 1500K cooler
        let (r, _g, b) = c.gains();
        assert!(r < 1.0, "cool: red gain should fall: {r}");
        assert!(b > 1.0, "cool: blue gain should rise: {b}");
    }

    #[test]
    fn invalid_reading_ignored() {
        let mut c = TemperatureCorrector::new(5500);
        c.compensate(&TemperatureReading {
            kelvin: 7000,
            valid: false,
        });
        assert!((c.gains().0 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn gains_are_clamped() {
        let mut c = TemperatureCorrector::new(5500);
        c.compensate(&reading(15500)); // huge warm deviation
        let (r, _g, b) = c.gains();
        assert!(r <= 1.5 && r >= 0.5);
        assert!(b <= 1.5 && b >= 0.5);
    }

    #[test]
    fn corrected_palette_applies_gains() {
        let mut c = TemperatureCorrector::new(5500);
        c.compensate(&reading(7000));
        let pal = c.corrected_palette();
        // Red channel (value 2) is boosted and clamps to max.
        assert_eq!(pal[2].r, 255);
        // Blue channel (value 4) should be cut.
        assert!(pal[4].b < 255);
    }

    #[test]
    fn target_preserved() {
        let c = TemperatureCorrector::new(6500);
        assert_eq!(c.target_kelvin(), 6500);
    }
}
