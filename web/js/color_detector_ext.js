// pixfer web extended color detector — a running color histogram with
// dominant-color tracking over the block grid.
// Mirrors `pixfer-core/src/color_detector_ext.rs`.

// A color-detection result over a block grid.
export class DetectionResult {
  constructor({ hasColors = false, counts = [0, 0, 0, 0, 0, 0, 0, 0], dominant = -1 } = {}) {
    this.hasColors = hasColors;
    this.counts = counts;
    this.dominant = dominant;
  }
}

// A running color detector with a resettable histogram.
export class ColorDetectorExt {
  constructor() {
    this.counts = [0, 0, 0, 0, 0, 0, 0, 0];
  }

  // The per-color counts.
  getCounts() {
    return this.counts.slice();
  }

  // The current dominant color value (-1 if no samples).
  dominant() {
    const maxc = Math.max(0, ...this.counts);
    if (maxc === 0) return -1;
    for (let i = 0; i < 8; i++) {
      if (this.counts[i] === maxc) return i;
    }
    return -1;
  }

  // Observe a set of block values (3-bit 0..7) and update the histogram.
  observe(blocks) {
    for (const b of blocks) {
      this.counts[b & 0b111] += 1;
    }
  }

  // Reset the histogram.
  reset() {
    this.counts.fill(0);
  }

  // The total number of observed blocks.
  total() {
    return this.counts.reduce((a, b) => a + b, 0);
  }
}

// Detect colors in a block grid (one-shot).
export function detectColorsExt(blocks) {
  const counts = [0, 0, 0, 0, 0, 0, 0, 0];
  for (const b of blocks) {
    counts[b & 0b111] += 1;
  }
  const hasColors = counts.some((c) => c > 0);
  const maxc = Math.max(0, ...counts);
  const dominant = maxc === 0 ? -1 : counts.indexOf(maxc);
  return new DetectionResult({ hasColors, counts, dominant });
}