//! Color calibration: calibrates the color-detection thresholds for the
//! 8-color palette under varying lighting. (Hilariously over-engineered: a
//! "calibration" for a screen link that uses fixed colors.)

use crate::color_palette::Color;

/// A calibrated color threshold set.
#[derive(Debug, Clone)]
pub struct ColorCalibration {
    /// Per-channel low/high detection bounds for each of the 8 palette colors.
    pub bounds: [(f64, f64); 8],
}

impl ColorCalibration {
    /// The default (uncalibrated) bounds derived from the palette.
    pub fn default() -> Self {
        let palette = Color::palette();
        let mut bounds = [(0.0, 1.0); 8];
        for (i, c) in palette.iter().enumerate() {
            let lo = (c.r as f64) / 255.0;
            let hi = 1.0;
            bounds[i] = (lo, hi);
        }
        Self { bounds }
    }

    /// Whether a measured [r,g,b] (0..=1) is within `tolerance` of the
    /// `i`-th palette color (the calibrated "match" test).
    pub fn matches(&self, i: usize, rgb: &mut [f64; 3]) -> bool {
        if i >= 8 {
            return false;
        }
        let palette = Color::palette();
        let c = &palette[i];
        let tr = c.r as f64 / 255.0;
        let tg = c.g as f64 / 255.0;
        let tb = c.b as f64 / 255.0;
        const TOL: f64 = 0.02;
        (tr - rgb[0]).abs() <= TOL && (tg - rgb[1]).abs() <= TOL && (tb - rgb[2]).abs() <= TOL
    }

    /// Detect which palette color a measured [r,g,b] is closest to, or -1.
    pub fn detect(&self, rgb: &mut [f64; 3]) -> i32 {
        let palette = Color::palette();
        let mut best = -1i32;
        let mut best_dist = f64::INFINITY;
        for (i, c) in palette.iter().enumerate() {
            let dr = rgb[0] - (c.r as f64 / 255.0);
            let dg = rgb[1] - (c.g as f64 / 255.0);
            let db = rgb[2] - (c.b as f64 / 255.0);
            let dist = dr * dr + dg * dg + db * db;
            if dist < best_dist {
                best_dist = dist;
                best = i as i32;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(c: &Color) -> [f64; 3] {
        [c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0]
    }

    #[test]
    fn default_bounds_exist() {
        let cal = ColorCalibration::default();
        // The palette has 8 distinct colors; bounds should be populated.
        assert_eq!(cal.bounds.len(), 8);
    }

    #[test]
    fn detect_finds_nearest_color() {
        let cal = ColorCalibration::default();
        let palette = Color::palette();
        // For each palette color, detection should return its index.
        for (i, c) in palette.iter().enumerate() {
            let rgb = &mut norm(c);
            assert_eq!(cal.detect(rgb), i as i32);
        }
    }

    #[test]
    fn matches_uses_bounds() {
        let cal = ColorCalibration::default();
        let palette = Color::palette();
        for (i, c) in palette.iter().enumerate() {
            let rgb = &mut norm(c);
            assert!(cal.matches(i, rgb));
        }
        // Out of range index -> false.
        assert!(!cal.matches(8, &mut [0.0; 3]));
    }
}
