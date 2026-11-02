//! Extended color quantizer: snap arbitrary RGB to the 8-color palette with
//! a configurable tolerance. (Hilariously over-engineered: a "quantizer"
//! for a screen that can only show 8 colors.)

use crate::color_palette::Color;

/// A quantization result.
#[derive(Debug, Clone, Copy)]
pub struct QuantResult {
    /// The snapped palette color.
    pub color: Color,
    /// The squared distance to the nearest palette color.
    pub dist2: f64,
    /// The 3-bit palette index.
    pub value: u8,
}

/// Quantizes an arbitrary RGB color to the nearest of the 8 palette colors,
/// optionally enforcing a tolerance.
pub struct ColorQuantizerExt {
    /// The squared-distance tolerance; colors farther than this are still
    /// snapped to the nearest (the tolerance gates acceptance).
    pub tol2: f64,
}

impl ColorQuantizerExt {
    pub fn new(tolerance: f64) -> Self {
        Self {
            tol2: tolerance * tolerance,
        }
    }

    /// The tolerance.
    pub fn tolerance(&self) -> f64 {
        self.tol2.sqrt()
    }

    /// Quantize `rgb` to the nearest palette color, returning the snapped
    /// color, its 3-bit value, and the squared distance.
    pub fn quantize(&self, rgb: (u8, u8, u8)) -> QuantResult {
        let palette = Color::palette();
        let (r, g, b) = rgb;
        let mut best_i = 0usize;
        let mut best_d2 = f64::INFINITY;
        for (i, pc) in palette.iter().enumerate() {
            let dr = r as f64 - pc.r as f64;
            let dg = g as f64 - pc.g as f64;
            let db = b as f64 - pc.b as f64;
            let d2 = dr * dr + dg * dg + db * db;
            if d2 < best_d2 {
                best_d2 = d2;
                best_i = i;
            }
        }
        let color = palette[best_i];
        let value = best_i as u8 & 0b111;
        QuantResult {
            color,
            dist2: best_d2,
            value,
        }
    }

    /// Quantize and report whether the nearest color is within tolerance.
    pub fn quantize_with_tolerance(&self, rgb: (u8, u8, u8)) -> (QuantResult, bool) {
        let q = self.quantize(rgb);
        (q, q.dist2 <= self.tol2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_palette_color_zero_distance() {
        let q = ColorQuantizerExt::new(0.0);
        // Gray (128,128,128) is exactly palette index 7.
        let r = q.quantize((128, 128, 128));
        assert_eq!(r.value, 7);
        assert_eq!(r.dist2, 0.0);
        assert_eq!(r.color, Color::rgb(128, 128, 128));
    }

    #[test]
    fn off_palette_snaps_to_nearest() {
        let q = ColorQuantizerExt::new(0.0);
        // (200,30,250) is closest to magenta (255,0,255)? distances:
        // magenta d2 = 55^2 + 30^2 + 5^2 = 3025+900+25 = 3950
        // yellow (255,255,0): 55^2+225^2+250^2 = 3025+50625+62500 = 116150
        // So magenta wins -> value 5.
        let r = q.quantize((200, 30, 250));
        assert_eq!(r.value, 5);
        assert!((r.dist2 - 3950.0).abs() < 1e-6);
    }

    #[test]
    fn tolerance_gates_acceptance() {
        let q = ColorQuantizerExt::new(10.0); // tol2 = 100
        let (r, ok) = q.quantize_with_tolerance((128, 128, 128));
        assert!(ok);
        assert_eq!(r.value, 7);
        // A far color: (10,10,10) vs black d2 = 100+100+100 = 300 > 100.
        let (_, ok2) = q.quantize_with_tolerance((10, 10, 10));
        assert!(!ok2);
    }

    #[test]
    fn zero_tolerance_always_accepts_nearest() {
        let q = ColorQuantizerExt::new(0.0);
        let (r, _) = q.quantize_with_tolerance((200, 30, 250));
        assert_eq!(r.value, 5);
    }

    #[test]
    fn tie_break_prefers_lower_index() {
        let q = ColorQuantizerExt::new(0.0);
        // (127,0,0): black d2 = 127^2 = 16129; red d2 = 128^2 = 16384.
        // Black (index 0) is slightly closer -> wins the tie-ish case.
        let r = q.quantize((127, 0, 0));
        assert_eq!(r.value, 0);
    }
}
