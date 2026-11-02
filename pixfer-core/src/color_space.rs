//! Color space: conversions between the 8-color 3-bit palette and RGB, plus
//! a perceptual distance model for the light link. (Hilariously over-
//! engineered: a "color science" module for a screen that can only show 8
//! colors.)

use crate::color_palette::Color;

/// A color-space transformation result.
#[derive(Debug, Clone, Copy)]
pub struct ColorSpaceResult {
    /// The transformed RGB value.
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Converts between RGB and the 3-bit palette value, and measures perceptual
/// distance.
pub struct ColorSpace {
    /// The 8-color palette (index = 3-bit value).
    palette: [Color; 8],
}

impl ColorSpace {
    /// Build a color-space handler with the standard palette.
    pub fn new() -> Self {
        Self {
            palette: Color::palette(),
        }
    }

    /// The palette color for a 3-bit value.
    pub fn color_for_value(&self, v: u8) -> Color {
        self.palette[(v & 0b111) as usize]
    }

    /// The 3-bit value for a palette color (None if not in palette).
    pub fn value_for_color(&self, c: &Color) -> Option<u8> {
        for (i, pc) in self.palette.iter().enumerate() {
            if pc == c {
                return Some(i as u8 & 0b111);
            }
        }
        None
    }

    /// The nearest palette color to an arbitrary RGB, by squared Euclidean
    /// distance in RGB space (lower index breaks ties).
    pub fn nearest_value(&self, c: &Color) -> u8 {
        if let Some(v) = self.value_for_color(c) {
            return v;
        }
        let mut best_v: u8 = 0;
        let mut best_d2 = f64::INFINITY;
        for (i, pc) in self.palette.iter().enumerate() {
            let dr = (c.r as f64 - pc.r as f64);
            let dg = (c.g as f64 - pc.g as f64);
            let db = (c.b as f64 - pc.b as f64);
            let d2 = dr * dr + dg * dg + db * db;
            if d2 < best_d2 {
                best_d2 = d2;
                best_v = i as u8;
            }
        }
        best_v
    }

    /// The perceptual distance (squared Euclidean in RGB) between two colors.
    pub fn distance2(&self, a: &Color, b: &Color) -> f64 {
        let dr = (a.r as f64 - b.r as f64);
        let dg = (a.g as f64 - b.g as f64);
        let db = (a.b as f64 - b.b as f64);
        dr * dr + dg * dg + db * db
    }

    /// Whether two colors are perceptually distinct (distance2 > 0).
    pub fn is_distinct(&self, a: &Color, b: &Color) -> bool {
        self.distance2(a, b) > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_has_eight_distinct_colors() {
        let cs = ColorSpace::new();
        let pal = [
            cs.color_for_value(0),
            cs.color_for_value(1),
            cs.color_for_value(2),
            cs.color_for_value(3),
            cs.color_for_value(4),
            cs.color_for_value(5),
            cs.color_for_value(6),
            cs.color_for_value(7),
        ];
        for (i, c) in pal.iter().enumerate() {
            for (j, d) in pal.iter().skip(i + 1).enumerate() {
                assert!(
                    cs.is_distinct(c, d),
                    "colors {} and {} not distinct",
                    i,
                    i + 1 + j
                );
            }
        }
    }

    #[test]
    fn color_value_roundtrip() {
        let cs = ColorSpace::new();
        for v in 0..8u8 {
            let c = cs.color_for_value(v);
            assert_eq!(cs.value_for_color(&c), Some(v));
        }
    }

    #[test]
    fn nearest_value_for_unknown_color() {
        let cs = ColorSpace::new();
        // (128, 0, 0) is closest to red (255,0,0).
        let c = Color::rgb(128, 0, 0);
        assert_eq!(cs.nearest_value(&c), 2);
        // (128,128,128) is exactly gray (value 7).
        let gray = Color::rgb(128, 128, 128);
        assert_eq!(cs.nearest_value(&gray), 7);
    }

    #[test]
    fn distance_zero_for_same_color() {
        let cs = ColorSpace::new();
        let c = Color::rgb(255, 0, 255);
        assert_eq!(cs.distance2(&c, &c), 0.0);
        let d = Color::rgb(255, 0, 0);
        assert!(cs.is_distinct(&c, &d));
    }

    #[test]
    fn distance_math() {
        let cs = ColorSpace::new();
        // red (255,0,0) vs white (255,255,255): d2 = 0 + 255^2 + 255^2
        let red = Color::rgb(255, 0, 0);
        let white = Color::rgb(255, 255, 255);
        assert!((cs.distance2(&red, &white) - (255.0 * 255.0 * 2.0)).abs() < 1e-6);
    }
}
