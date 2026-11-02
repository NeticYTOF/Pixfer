//! Adaptive color calibration: the sender's screen colors drift over
//! time and temperature; this module re-calibrates the palette against
//! the receiver's camera readings. (Hilariously over-engineered: a
//! "self-tuning" color system for a link that only knows 8 colors.)

use crate::color_palette::{color_for_value, Color};

/// A measured color reading from the camera.
#[derive(Debug, Clone, Copy)]
pub struct MeasuredColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

/// An adaptive color calibration table: maps each of the 8 palette
/// values to a measured, device-specific RGB that the camera actually
/// sees (accounting for screen aging, temperature drift, and
/// camera white-balance).
#[derive(Debug, Clone)]
pub struct AdaptivePalette {
    /// The 8 calibrated colors, indexed by 3-bit value.
    colors: [Color; 8],
}

impl AdaptivePalette {
    /// Build an adaptive palette from a set of measured colors (one per
    /// 3-bit value, in order 0..=7).
    pub fn from_measurements(measured: &[MeasuredColor]) -> Self {
        let mut colors = [Color::rgb(0, 0, 0); 8];
        for (i, m) in measured.iter().enumerate().take(8) {
            colors[i] = Color::rgb(
                (m.r * 255.0).round().clamp(0.0, 255.0) as u8,
                (m.g * 255.0).round().clamp(0.0, 255.0) as u8,
                (m.b * 255.0).round().clamp(0.0, 255.0) as u8,
            );
        }
        Self { colors }
    }

    /// The calibrated color for a 3-bit value.
    pub fn color_for_value(&self, v: u8) -> Color {
        self.colors[v as usize & 0b111]
    }

    /// Whether the palette matches the default 8-color palette.
    pub fn is_default(&self) -> bool {
        for v in 0..8u8 {
            if self.colors[v as usize] != color_for_value(v) {
                return false;
            }
        }
        true
    }
}

/// Re-calibrate the sender's palette from camera measurements.
pub fn recalibrate(measured: &[MeasuredColor]) -> AdaptivePalette {
    AdaptivePalette::from_measurements(measured)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(r: f64, g: f64, b: f64) -> MeasuredColor {
        MeasuredColor { r, g, b }
    }

    #[test]
    fn from_measurements_builds_palette() {
        let measured = vec![
            m(0.0, 0.0, 0.0),
            m(1.0, 1.0, 1.0),
            m(1.0, 0.0, 0.0),
            m(0.0, 1.0, 0.0),
            m(0.0, 0.0, 1.0),
            m(1.0, 0.0, 1.0),
            m(1.0, 1.0, 0.0),
            m(0.5, 0.5, 0.5),
        ];
        let p = AdaptivePalette::from_measurements(&measured);
        assert!(p.is_default());
    }

    #[test]
    fn drifted_measurements_differ() {
        let measured = vec![
            m(0.05, 0.05, 0.05), // black slightly lifted
            m(0.9, 0.9, 0.9),    // white slightly dimmed
            m(1.0, 0.1, 0.0),    // red shifted toward orange
            m(0.0, 1.0, 0.1),
            m(0.0, 0.1, 1.0),
            m(1.0, 0.1, 1.0),
            m(1.0, 1.0, 0.1),
            m(0.5, 0.5, 0.5),
        ];
        let p = AdaptivePalette::from_measurements(&measured);
        assert!(!p.is_default());
        // The red channel is shifted.
        let red = p.color_for_value(2);
        assert_eq!(red.g, 26); // 0.1 * 255 ≈ 25.5 -> 26
    }

    #[test]
    fn color_for_value_is_indexed() {
        let measured = vec![
            m(1.0, 0.0, 0.0),
            m(0.0, 1.0, 0.0),
            m(0.0, 0.0, 1.0),
            m(1.0, 0.0, 1.0),
            m(1.0, 1.0, 0.0),
            m(0.5, 0.5, 0.5),
            m(0.2, 0.2, 0.2),
            m(0.8, 0.8, 0.8),
        ];
        let p = AdaptivePalette::from_measurements(&measured);
        assert_eq!(p.color_for_value(0), Color::rgb(255, 0, 0));
        assert_eq!(p.color_for_value(7), Color::rgb(204, 204, 204));
    }

    #[test]
    fn recalibrate_is_alias() {
        let measured = vec![m(0.5, 0.5, 0.5)];
        let p = recalibrate(&measured);
        assert!(!p.is_default());
    }

    #[test]
    fn insufficient_measurements_pads_with_black() {
        let measured = vec![m(1.0, 1.0, 1.0)];
        let p = AdaptivePalette::from_measurements(&measured);
        // Only 1 measurement: value 0 = white; values 1..=7 stay black.
        assert_eq!(p.color_for_value(0), Color::rgb(255, 255, 255));
        assert_eq!(p.color_for_value(1), Color::rgb(0, 0, 0));
        assert_eq!(p.color_for_value(7), Color::rgb(0, 0, 0));
    }
}
