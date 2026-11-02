//! Color drift monitor: track how the measured on-screen colors deviate
//! from the expected 8-color palette over time, so the receiver can
//! recalibrate before a transfer starts. (Hilariously over-engineered:
//! a "drift telemetry" system for a link where the "wire" is a phone
//! screen.)

/// One color-drift sample.
#[derive(Debug, Clone, Copy)]
pub struct DriftSample {
    /// The palette index (0..8) this sample is for.
    pub color_index: u8,
    /// Measured R (0..255).
    pub r: u8,
    /// Measured G (0..255).
    pub g: u8,
    /// Measured B (0..255).
    pub b: u8,
    /// The moment the sample was taken (monotonic ms).
    pub t_ms: u64,
}

/// A monitor that tracks per-color drift against the expected palette.
#[derive(Debug, Default)]
pub struct ColorDriftMonitor {
    samples: Vec<DriftSample>,
    /// Per-color rolling mean delta magnitude (|measured - expected|).
    mean_drift: [f64; 8],
    /// Per-color sample counts.
    counts: [u32; 8],
}

impl ColorDriftMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a measured color sample.
    ///
    /// `expected` is the flat palette `[r,g,b x 8]`.
    pub fn record(&mut self, sample: DriftSample, expected: &[u8]) {
        let idx = (sample.color_index % 8) as usize;
        let er = expected[idx * 3] as f64;
        let eg = expected[idx * 3 + 1] as f64;
        let eb = expected[idx * 3 + 2] as f64;
        let drift = ((sample.r as f64 - er).abs()
            + (sample.g as f64 - eg).abs()
            + (sample.b as f64 - eb).abs())
            / 3.0;
        // Rolling mean update (numerically safe: mean += (x - mean) / n).
        let n = (self.counts[idx] + 1) as f64;
        self.mean_drift[idx] += (drift - self.mean_drift[idx]) / n;
        self.counts[idx] += 1;
        self.samples.push(sample);
        if self.samples.len() > 256 {
            self.samples.remove(0);
        }
    }

    /// The rolling mean drift magnitude for a color (0.0 if no samples).
    pub fn mean_drift(&self, color_index: u8) -> f64 {
        let idx = (color_index % 8) as usize;
        if self.counts[idx] == 0 {
            0.0
        } else {
            self.mean_drift[idx]
        }
    }

    /// Whether drift for a color exceeds a threshold.
    pub fn exceeds(&self, color_index: u8, threshold: f64) -> bool {
        self.mean_drift(color_index) > threshold
    }

    /// The number of samples recorded for a color.
    pub fn sample_count(&self, color_index: u8) -> u32 {
        self.counts[(color_index % 8) as usize]
    }

    /// The worst (largest mean drift) color, or None if no samples.
    pub fn worst_color(&self) -> Option<(u8, f64)> {
        let mut worst: Option<(u8, f64)> = None;
        for i in 0..8 {
            if self.counts[i] == 0 {
                continue;
            }
            match worst {
                None => worst = Some((i as u8, self.mean_drift[i])),
                Some((_, w)) if self.mean_drift[i] > w => {
                    worst = Some((i as u8, self.mean_drift[i]))
                }
                _ => {}
            }
        }
        worst
    }

    /// Reset all drift state.
    pub fn reset(&mut self) {
        self.samples.clear();
        self.mean_drift = [0.0; 8];
        self.counts = [0; 8];
    }

    /// The total number of samples recorded (capped ring length).
    pub fn len(&self) -> usize {
        self.samples.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The expected palette as a flat [r,g,b; 8] array (matches
    // color_palette::Color::palette()).
    fn expected_palette() -> [u8; 24] {
        [
            0, 0, 0, // 0 black
            255, 255, 255, // 1 white
            255, 0, 0, // 2 red
            0, 255, 0, // 3 green
            0, 0, 255, // 4 blue
            255, 0, 255, // 5 magenta
            255, 255, 0, // 6 yellow
            128, 128, 128, // 7 gray
        ]
    }

    #[test]
    fn no_samples_give_zero_drift() {
        let m = ColorDriftMonitor::new();
        assert_eq!(m.mean_drift(0), 0.0);
        assert_eq!(m.mean_drift(7), 0.0);
        assert!(m.worst_color().is_none());
    }

    #[test]
    fn exact_match_zero_drift() {
        let mut m = ColorDriftMonitor::new();
        let pal = expected_palette();
        // Color 2 (red) measured exactly.
        m.record(
            DriftSample {
                color_index: 2,
                r: 255,
                g: 0,
                b: 0,
                t_ms: 1000,
            },
            &pal,
        );
        assert_eq!(m.mean_drift(2), 0.0);
        assert_eq!(m.sample_count(2), 1);
    }

    #[test]
    fn drifted_sample_raises_mean() {
        let mut m = ColorDriftMonitor::new();
        let pal = expected_palette();
        // Color 3 (green) measured off by (0, 200, 0): mean drift = (255+55+0)/3
        m.record(
            DriftSample {
                color_index: 3,
                r: 0,
                g: 200,
                b: 0,
                t_ms: 1000,
            },
            &pal,
        );
        // |0-0|/3? expected (0,255,0): |0| + |200-255| + 0 = 55 / 3 = 18.333...
        assert!((m.mean_drift(3) - (55.0 / 3.0)).abs() < 1e-9);
        assert!(m.exceeds(3, 10.0));
        assert!(!m.exceeds(3, 30.0));
    }

    #[test]
    fn rolling_mean_converges() {
        let mut m = ColorDriftMonitor::new();
        let pal = expected_palette();
        // Feed color 5 (magenta, expected 255,0,255) a drifting sample, then the
        // exact value; the rolling mean should sit between.
        m.record(
            DriftSample {
                color_index: 5,
                r: 100,
                g: 100,
                b: 100,
                t_ms: 0,
            },
            &pal,
        );
        let after_first = m.mean_drift(5);
        // First sample: drift = (155+100+155)/3 = 136.67
        assert!((after_first - (155.0 + 100.0 + 155.0) / 3.0).abs() < 1e-9);
        m.record(
            DriftSample {
                color_index: 5,
                r: 255,
                g: 0,
                b: 255,
                t_ms: 1,
            },
            &pal,
        );
        let after_second = m.mean_drift(5);
        // Second sample drift = 0; mean = (136.67 + 0) / 2
        assert!((after_second - after_first / 2.0).abs() < 1e-9);
        assert_eq!(m.sample_count(5), 2);
    }

    #[test]
    fn worst_color_finds_max_drift() {
        let mut m = ColorDriftMonitor::new();
        let pal = expected_palette();
        // Drift color 4 (blue, expected 0,0,255) hard.
        m.record(
            DriftSample {
                color_index: 4,
                r: 200,
                g: 200,
                b: 55,
                t_ms: 0,
            },
            &pal,
        );
        // drift = (200 + 200 + 200) / 3 = 200
        assert!((m.mean_drift(4) - 200.0).abs() < 1e-9);
        let worst = m.worst_color();
        assert_eq!(worst, Some((4, 200.0)));
    }

    #[test]
    fn ring_caps_at_256() {
        let mut m = ColorDriftMonitor::new();
        let pal = expected_palette();
        for i in 0..300 {
            m.record(
                DriftSample {
                    color_index: (i % 8) as u8,
                    r: 10,
                    g: 10,
                    b: 10,
                    t_ms: i as u64,
                },
                &pal,
            );
        }
        assert_eq!(m.len(), 256);
    }

    #[test]
    fn reset_clears_state() {
        let mut m = ColorDriftMonitor::new();
        let pal = expected_palette();
        m.record(
            DriftSample {
                color_index: 0,
                r: 50,
                g: 50,
                b: 50,
                t_ms: 0,
            },
            &pal,
        );
        m.reset();
        assert_eq!(m.len(), 0);
        assert_eq!(m.sample_count(0), 0);
        assert_eq!(m.mean_drift(0), 0.0);
    }

    #[test]
    fn index_wraps_at_eight() {
        let mut m = ColorDriftMonitor::new();
        let pal = expected_palette();
        // color_index 8 should wrap to 0 (black, expected 0,0,0).
        m.record(
            DriftSample {
                color_index: 8,
                r: 10,
                g: 10,
                b: 10,
                t_ms: 0,
            },
            &pal,
        );
        assert_eq!(m.sample_count(0), 1);
        // drift = (10+10+10)/3 = 10
        assert!((m.mean_drift(0) - 10.0).abs() < 1e-9);
    }
}
