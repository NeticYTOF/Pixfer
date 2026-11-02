//! Extended color detector: a running color histogram with dominant-color
//! tracking over the 270x480 block grid. (Hilariously over-engineered: a
//! "histogram engine" for 8 colors on a phone screen.)

/// A color-detection result over a block grid.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Whether any palette color was detected.
    pub has_colors: bool,
    /// Per-color counts (index = 3-bit value).
    pub counts: [u64; 8],
    /// The dominant color value (lowest index among max counts), -1 if none.
    pub dominant: i32,
}

/// A running color detector with a resettable histogram.
#[derive(Debug, Default)]
pub struct ColorDetectorExt {
    counts: [u64; 8],
}

impl ColorDetectorExt {
    pub fn new() -> Self {
        Self::default()
    }

    /// The per-color counts.
    pub fn get_counts(&self) -> [u64; 8] {
        self.counts
    }

    /// The current dominant color value (-1 if no samples).
    pub fn dominant(&self) -> i32 {
        let maxc = self.counts.iter().max().copied().unwrap_or(0);
        if maxc == 0 {
            return -1;
        }
        // First index (lowest) whose count equals max.
        for i in 0..8 {
            if self.counts[i] == maxc {
                return i as i32;
            }
        }
        -1
    }

    /// Observe a set of block values (3-bit 0..7) and update the histogram.
    pub fn observe(&mut self, blocks: &[u8]) {
        for &b in blocks {
            let idx = (b & 0b111) as usize;
            self.counts[idx] += 1;
        }
    }

    /// Reset the histogram.
    pub fn reset(&mut self) {
        self.counts = [0; 8];
    }

    /// The total number of observed blocks.
    pub fn total(&self) -> u64 {
        self.counts.iter().sum()
    }
}

/// Detect colors in a block grid (one-shot).
pub fn detect_colors_ext(blocks: &[u8]) -> DetectionResult {
    let mut counts = [0u64; 8];
    for &b in blocks {
        counts[(b & 0b111) as usize] += 1;
    }
    let has_colors = counts.iter().any(|&c| c > 0);
    let maxc = counts.iter().max().copied().unwrap_or(0);
    let dominant = if maxc == 0 {
        -1
    } else {
        (0..8).find(|&i| counts[i] == maxc).unwrap() as i32
    };
    DetectionResult {
        has_colors,
        counts,
        dominant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_grid_no_colors() {
        let r = detect_colors_ext(&[]);
        assert!(!r.has_colors);
        assert_eq!(r.dominant, -1);
        assert_eq!(r.counts, [0; 8]);
    }

    #[test]
    fn all_eight_colors_counted_once() {
        let blocks = [0u8, 1, 2, 3, 4, 5, 6, 7];
        let r = detect_colors_ext(&blocks);
        assert!(r.has_colors);
        assert!(r.counts.iter().all(|&c| c == 1));
        assert_eq!(r.dominant, 0); // first max (all tied -> lowest index)
    }

    #[test]
    fn full_black_grid_dominant_zero() {
        let blocks = vec![0u8; 270 * 480];
        let r = detect_colors_ext(&blocks);
        assert!(r.has_colors);
        assert_eq!(r.dominant, 0);
        assert_eq!(r.counts[0], 270 * 480);
    }

    #[test]
    fn detector_accumulates() {
        let mut d = ColorDetectorExt::new();
        d.observe(&[0, 0, 1, 1, 1]);
        let c = d.get_counts();
        assert_eq!(c[0], 2);
        assert_eq!(c[1], 3);
        assert_eq!(d.dominant(), 1);
        assert_eq!(d.total(), 5);
    }

    #[test]
    fn detector_reset_clears() {
        let mut d = ColorDetectorExt::new();
        d.observe(&[7, 7]);
        assert_eq!(d.get_counts()[7], 2);
        d.reset();
        assert_eq!(d.dominant(), -1);
        assert_eq!(d.total(), 0);
    }

    #[test]
    fn value_wraps_to_three_bits() {
        let mut d = ColorDetectorExt::new();
        // 9 & 0b111 = 1, so it counts into slot 1.
        d.observe(&[9]);
        assert_eq!(d.get_counts()[1], 1);
    }
}
