// pixfer web color lookup table — fast RGB → 3-bit value mapping for the
// 8-color palette.
// Mirrors `pixfer-core/src/color_lut.rs`.

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

// A lookup table mapping an RGB color to its 3-bit value.
export class ColorLut {
  // Build a LUT from the standard 8-color palette.
  constructor() {
    this.palette = PALETTE;
  }

  // The number of colors the LUT covers.
  size() {
    return this.palette.length;
  }

  // The color at palette index `v`.
  colorFor(v) {
    return this.palette[v & 0b111].slice();
  }

  // Map an RGB color to the nearest palette 3-bit value.
  valueFor(rgb) {
    const [r, g, b] = rgb;
    // Exact match?
    for (let i = 0; i < this.palette.length; i++) {
      const [pr, pg, pb] = this.palette[i];
      if (pr === r && pg === g && pb === b) return i;
    }
    // Nearest by squared Euclidean distance (lower index breaks ties).
    let bestV = 0;
    let bestD2 = Infinity;
    for (let i = 0; i < this.palette.length; i++) {
      const [pr, pg, pb] = this.palette[i];
      const dr = r - pr;
      const dg = g - pg;
      const db = b - pb;
      const d2 = dr * dr + dg * dg + db * db;
      if (d2 < bestD2) {
        bestD2 = d2;
        bestV = i;
      }
    }
    return bestV;
  }

  // The palette color for a 3-bit value.
  toColor(v) {
    return this.colorFor(v);
  }
}

// Quantize an arbitrary RGB triple to the nearest of the 8 palette colors.
export function quantize(rgb) {
  const lut = new ColorLut();
  return lut.toColor(lut.valueFor(rgb));
}