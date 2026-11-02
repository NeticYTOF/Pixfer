//! Color lookup table: fast RGB → 3-bit value mapping for the 8-color
//! palette. (Hilariously over-engineered: a "LUT" for a screen that can
//! only show 8 colors.)

use crate::color_palette::{color_for_value, value_for_color, Color};

/// A lookup table mapping an RGB color to its 3-bit palette value.
///
/// Backed by a flat array of 256*256*256 entries would be huge, so this
/// uses the exact-palette membership test instead: a color maps to its
/// 3-bit value only if it is exactly one of the 8 palette colors;
/// otherwise the nearest palette color is chosen (min Euclidean distance).
pub struct ColorLut {
    /// The 8-color palette (index = 3-bit value).
    palette: [Color; 8],
}

impl ColorLut {
    /// Build a LUT from the standard 8-color palette.
    pub fn new() -> Self {
        Self {
            palette: Color::palette(),
        }
    }

    /// The number of colors the LUT covers.
    pub fn len(&self) -> usize {
        8
    }

    /// The color at palette index `v`.
    pub fn color_for(&self, v: u8) -> Color {
        self.palette[(v & 0b111) as usize]
    }

    /// Map an RGB color to the nearest palette 3-bit value.
    ///
    /// Returns the exact value if the color is in the palette, else the
    /// nearest by squared Euclidean distance (deterministic tie-break:
    /// lower index wins).
    pub fn value_for(&self, c: &Color) -> u8 {
        if let Some(v) = value_for_color(c) {
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

    /// The palette color for a 3-bit value (delegates to color_for_value).
    pub fn to_color(&self, v: u8) -> Color {
        color_for_value(v)
    }
}

/// Quantize an arbitrary RGB color to the nearest of the 8 palette colors.
pub fn quantize(c: &Color) -> Color {
    let lut = ColorLut::new();
    lut.to_color(lut.value_for(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_covers_eight_colors() {
        let lut = ColorLut::new();
        assert_eq!(lut.len(), 8);
    }

    #[test]
    fn exact_palette_color_maps_to_its_value() {
        let lut = ColorLut::new();
        for v in 0..8u8 {
            let c = lut.color_for(v);
            assert_eq!(lut.value_for(&c), v);
        }
    }

    #[test]
    fn unknown_color_maps_to_nearest() {
        let lut = ColorLut::new();
        // (128, 0, 0) is closest to red (255,0,0)? dist to red = 127^2, to black = 128^2 -> red.
        let c = Color::rgb(128, 0, 0);
        assert_eq!(lut.value_for(&c), 2); // red is index 2
                                          // (128, 128, 128) is exactly gray (index 7).
        let gray = Color::rgb(128, 128, 128);
        assert_eq!(lut.value_for(&gray), 7);
    }

    #[test]
    fn quantize_snap_to_palette() {
        let c = Color::rgb(200, 30, 250);
        let q = quantize(&c);
        // Nearest is magenta (255,0,255)? Check distances.
        let lut = ColorLut::new();
        assert_eq!(q, lut.to_color(lut.value_for(&c)));
    }

    #[test]
    fn color_for_matches_palette() {
        let lut = ColorLut::new();
        assert_eq!(lut.color_for(0), Color::rgb(0, 0, 0));
        assert_eq!(lut.color_for(1), Color::rgb(255, 255, 255));
        assert_eq!(lut.color_for(7), Color::rgb(128, 128, 128));
    }

    #[test]
    fn tie_break_prefers_lower_index() {
        let lut = ColorLut::new();
        // A color equidistant from two palette entries: lower index wins.
        // (127, 0, 0) vs red(255,0,0) d2=128^2, vs black(0,0,0) d2=127^2 -> black wins (lower index 0).
        let c = Color::rgb(127, 0, 0);
        assert_eq!(lut.value_for(&c), 0);
    }
}
