// pixfer web color detector — classify which of the 8 palette colors are
// present in a captured block grid. Mirrors `pixfer-core/src/color_detector.rs`.

// The result of a color-detection pass over a block grid.
class DetectionResult {
  constructor(counts, dominant, hasColors) {
    this.counts = counts; // array of 8 counts
    this.dominant = dominant; // 0..=7 or -1
    this.hasColors = hasColors;
  }
}

// Detect which palette colors are present in a captured block grid.
export function detectColors(blocks) {
  const counts = new Array(8).fill(0);
  for (const b of blocks) {
    counts[b & 0b111] += 1;
  }
  let dominant = -1;
  let max = 0;
  for (let i = 0; i < 8; i++) {
    if (counts[i] > max) {
      max = counts[i];
      dominant = i;
    }
  }
  const hasColors = counts.some((c) => c > 0);
  return new DetectionResult(counts, dominant, hasColors);
}

// A running color detector that can be fed grid samples over time.
export class ColorDetector {
  constructor() {
    this.counts = new Array(8).fill(0);
  }

  // Feed one captured grid; accumulates the color histogram.
  observe(blocks) {
    for (const b of blocks) {
      this.counts[b & 0b111] += 1;
    }
  }

  // The current color histogram.
  getCounts() {
    return this.counts;
  }

  // The dominant color so far (index 0..=7, or -1 if nothing seen).
  dominant() {
    let d = -1;
    let m = 0;
    for (let i = 0; i < 8; i++) {
      if (this.counts[i] > m) {
        m = this.counts[i];
        d = i;
      }
    }
    return d;
  }

  // Reset the histogram.
  reset() {
    this.counts = new Array(8).fill(0);
  }
}