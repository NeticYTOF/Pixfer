// pixfer web color drift monitor — track how measured on-screen colors
// deviate from the expected 8-color palette over time.
// Mirrors `pixfer-core/src/color_drift_monitor.rs`.

// One color-drift sample.
export class DriftSample {
  constructor({ colorIndex = 0, r = 0, g = 0, b = 0, tMs = 0 } = {}) {
    this.colorIndex = colorIndex;
    this.r = r;
    this.g = g;
    this.b = b;
    this.tMs = tMs;
  }
}

// A monitor that tracks per-color drift against the expected palette.
export class ColorDriftMonitor {
  constructor() {
    this.samples = [];
    this.meanDrift = new Array(8).fill(0.0);
    this.counts = new Array(8).fill(0);
  }

  // Record a measured color sample. `expected` is the flat palette [r,g,b x 8].
  record(sample, expected) {
    const idx = sample.colorIndex % 8;
    const er = expected[idx * 3];
    const eg = expected[idx * 3 + 1];
    const eb = expected[idx * 3 + 2];
    const drift = (
      Math.abs(sample.r - er) + Math.abs(sample.g - eg) + Math.abs(sample.b - eb)
    ) / 3;
    // Rolling mean update (numerically safe).
    const n = this.counts[idx] + 1;
    this.meanDrift[idx] += (drift - this.meanDrift[idx]) / n;
    this.counts[idx] += 1;
    this.samples.push(sample);
    if (this.samples.length > 256) this.samples.shift();
  }

  // The rolling mean drift magnitude for a color (0 if no samples).
  meanDriftOf(colorIndex) {
    const idx = colorIndex % 8;
    return this.counts[idx] === 0 ? 0 : this.meanDrift[idx];
  }

  // Whether drift for a color exceeds a threshold.
  exceeds(colorIndex, threshold) {
    return this.meanDriftOf(colorIndex) > threshold;
  }

  // The number of samples recorded for a color.
  sampleCount(colorIndex) {
    return this.counts[colorIndex % 8];
  }

  // The worst (largest mean drift) color, or null if no samples.
  worstColor() {
    let worst = null;
    for (let i = 0; i < 8; i++) {
      if (this.counts[i] === 0) continue;
      if (worst === null || this.meanDrift[i] > worst.drift) {
        worst = { colorIndex: i, drift: this.meanDrift[i] };
      }
    }
    return worst;
  }

  // Reset all drift state.
  reset() {
    this.samples.length = 0;
    this.meanDrift.fill(0.0);
    this.counts.fill(0);
  }

  // The total number of samples recorded (capped ring length).
  length() {
    return this.samples.length;
  }
}

// The expected 8-color palette as a flat [r,g,b; 8] array (matches
// color_palette.js / Color::palette()).
export function expectedPalette() {
  return [
    0, 0, 0, // 0 black
    255, 255, 255, // 1 white
    255, 0, 0, // 2 red
    0, 255, 0, // 3 green
    0, 0, 255, // 4 blue
    255, 0, 255, // 5 magenta
    255, 255, 0, // 6 yellow
    128, 128, 128, // 7 gray
  ];
}