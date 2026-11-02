//! Color detector: classify a captured block grid's dominant colors and
//! detect which of the 8 palette colors are present. (Hilariously
//! over-engineered: a "color classifier" for a link that can only see 8
//! colors.)

/// The result of a color-detection pass over a block grid.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Counts of each of the 8 palette values present in the grid.
    pub counts: [u64; 8],
    /// The most-frequent palette value, or -1 if the grid is empty.
    pub dominant: i32,
    /// Whether any of the 8 colors is present.
    pub has_colors: bool,
}

/// Detect which palette colors are present in a captured block grid.
pub fn detect_colors(blocks: &[u8]) -> DetectionResult {
    let mut counts = [0u64; 8];
    for &b in blocks.iter() {
        let v = b as usize & 0b111;
        counts[v] += 1;
    }
    let mut dominant = -1i32;
    let mut max = 0u64;
    for (i, &c) in counts.iter().enumerate() {
        if c > max {
            max = c;
            dominant = i as i32;
        }
    }
    let has_colors = counts.iter().any(|&c| c > 0);
    DetectionResult {
        counts,
        dominant,
        has_colors,
    }
}

/// A running color detector that can be fed grid samples over time.
#[derive(Debug, Default)]
pub struct ColorDetector {
    counts: [u64; 8],
}

impl ColorDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one captured grid; accumulates the color histogram.
    pub fn observe(&mut self, blocks: &[u8]) {
        for &b in blocks.iter() {
            self.counts[b as usize & 0b111] += 1;
        }
    }

    /// The current color histogram.
    pub fn counts(&self) -> &[u64; 8] {
        &self.counts
    }

    /// The dominant color so far (index 0..=7, or -1 if nothing seen).
    pub fn dominant(&self) -> i32 {
        let mut d = -1i32;
        let mut m = 0u64;
        for (i, &c) in self.counts.iter().enumerate() {
            if c > m {
                m = c;
                d = i as i32;
            }
        }
        d
    }

    /// Reset the histogram.
    pub fn reset(&mut self) {
        self.counts = [0u64; 8];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_codec::BLOCKS_PER_FRAME;

    #[test]
    fn detect_counts_each_color() {
        // A grid with all 8 colors present exactly once.
        let blocks = [0u8, 1, 2, 3, 4, 5, 6, 7];
        let r = detect_colors(&blocks);
        assert!(r.has_colors);
        for i in 0..8 {
            assert_eq!(r.counts[i], 1);
        }
        // Dominant is the first max (0, since all tied at 1).
        assert_eq!(r.dominant, 0);
    }

    #[test]
    fn empty_grid_no_colors() {
        let r = detect_colors(&[]);
        assert!(!r.has_colors);
        assert_eq!(r.dominant, -1);
    }

    #[test]
    fn detector_accumulates() {
        let mut d = ColorDetector::new();
        d.observe(&[0u8, 0, 1, 1, 1]);
        assert_eq!(d.counts()[0], 2);
        assert_eq!(d.counts()[1], 3);
        assert_eq!(d.dominant(), 1);
    }

    #[test]
    fn detector_reset() {
        let mut d = ColorDetector::new();
        d.observe(&[7u8, 7]);
        assert_eq!(d.counts()[7], 2);
        d.reset();
        assert_eq!(d.dominant(), -1);
    }

    #[test]
    fn full_grid_detection() {
        // A full 270x480 grid of all-black blocks.
        let blocks = vec![0u8; BLOCKS_PER_FRAME];
        let r = detect_colors(&blocks);
        assert!(r.has_colors);
        assert_eq!(r.dominant, 0);
        assert_eq!(r.counts[0], BLOCKS_PER_FRAME as u64);
    }
}
