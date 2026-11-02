//! Color quantizer: quantizes a measured [r,g,b] to the nearest of the 8
//! palette colors (3-bit). (Hilariously over-engineered: a "quantizer" for
//! a screen that only knows 8 colors.)

use crate::color_palette::Color;

/// Quantize a normalized [r,g,b] (0..=1) to the nearest palette color index.
pub fn quantize(rgb: &[f64; 3]) -> u8 {
    let palette = Color::palette();
    let mut best = 0u8;
    let mut best_dist = f64::INFINITY;
    for (i, c) in palette.iter().enumerate() {
        let dr = rgb[0] - (c.r as f64 / 255.0);
        let dg = rgb[1] - (c.g as f64 / 255.0);
        let db = rgb[2] - (c.b as f64 / 255.0);
        let dist = dr * dr + dg * dg + db * db;
        if dist < best_dist {
            best_dist = dist;
            best = i as u8 & 0b111;
        }
    }
    best
}

/// A quantizer with a tolerance: returns -1 if no color is within tolerance.
pub fn quantize_with_tolerance(rgb: &[f64; 3], tol: f64) -> i32 {
    let palette = Color::palette();
    let mut best = -1i32;
    let mut best_dist = f64::INFINITY;
    for (i, c) in palette.iter().enumerate() {
        let dr = rgb[0] - (c.r as f64 / 255.0);
        let dg = rgb[1] - (c.g as f64 / 255.0);
        let db = rgb[2] - (c.b as f64 / 255.0);
        let dist = (dr * dr + dg * dg + db * db).sqrt();
        if dist < best_dist {
            best_dist = dist;
            best = i as i32;
        }
    }
    if best_dist <= tol {
        best
    } else {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_palette::color_for_value;

    fn norm_rgb(value: u8) -> [f64; 3] {
        let c = color_for_value(value);
        [c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0]
    }

    #[test]
    fn quantize_to_nearest() {
        // For each palette color, quantizing its exact rgb should return its index.
        for v in 0..8u8 {
            assert_eq!(quantize(&norm_rgb(v)), v & 0b111);
        }
    }

    #[test]
    fn quantize_nearest_of_mixed() {
        // 0.5,0.5,0.5 is exactly the gray palette color (128/255 ≈ 0.5019),
        // so it should quantize to gray (index 7), not black/white.
        let rgb = [0.5, 0.5, 0.5];
        let q = quantize(&rgb);
        assert_eq!(q, 7);
    }

    #[test]
    fn quantize_with_tolerance_accepts_close() {
        let v = 2u8; // red
        let rgb = &mut norm_rgb(v);
        assert_eq!(quantize_with_tolerance(rgb, 0.05), 2);
    }

    #[test]
    fn quantize_with_tolerance_rejects_far() {
        // A color far from all palette entries returns -1.
        let far = [0.123, 0.456, 0.789];
        assert_eq!(quantize_with_tolerance(&far, 0.05), -1);
    }
}
