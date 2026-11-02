// pixfer web extended color quantizer — snap arbitrary RGB to the 8-color
// palette with a configurable tolerance.
// Mirrors `pixfer-core/src/color_quantizer_ext.rs`.

// The 8-color palette, index = 3-bit value (matches color_palette.js).
const PALETTE = [
  [0, 0, 0], // 0 black
  [255, 255, 255], // 1 white
  [255, 0, 0], // 2 red
  [0, 255, 0], // 3 green
  [0, 0, 255], // 4 blue
  [255, 0, 255], // 5 magenta
  [255, 255, 0], // 6 yellow
  [128, 128, 128], // 7 gray
];

// A quantization result.
export class QuantResult {
  constructor({ color = [0, 0, 0], dist2 = 0, value = 0 } = {}) {
    this.color = color;
    this.dist2 = dist2;
    this.value = value;
  }
}

// Quantizes an arbitrary RGB color to the nearest of the 8 palette colors,
// optionally enforcing a tolerance.
export class ColorQuantizerExt {
  constructor(tolerance) {
    this.tol2 = tolerance * tolerance;
  }

  // The tolerance.
  toleranceValue() {
    return Math.sqrt(this.tol2);
  }

  // Quantize `rgb` to the nearest palette color.
  quantize(rgb) {
    const [r, g, b] = rgb;
    let bestI = 0;
    let bestD2 = Infinity;
    for (let i = 0; i < PALETTE.length; i++) {
      const [pr, pg, pb] = PALETTE[i];
      const dr = r - pr;
      const dg = g - pg;
      const db = b - pb;
      const d2 = dr * dr + dg * dg + db * db;
      if (d2 < bestD2) {
        bestD2 = d2;
        bestI = i;
      }
    }
    return new QuantResult({ color: PALETTE[bestI].slice(), dist2: bestD2, value: bestI });
  }

  // Quantize and report whether the nearest color is within tolerance.
  quantizeWithTolerance(rgb) {
    const q = this.quantize(rgb);
    return [q, q.dist2 <= this.tol2];
  }
}