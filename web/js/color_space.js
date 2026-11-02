// pixfer web color space — conversions between the 8-color 3-bit palette
// and RGB, plus a perceptual distance model.
// Mirrors `pixfer-core/src/color_space.rs`.

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

// Converts between RGB and the 3-bit palette value, and measures
// perceptual distance.
export class ColorSpace {
  constructor() {
    this.palette = PALETTE;
  }

  // The palette color for a 3-bit value.
  colorForValue(v) {
    return this.palette[v & 0b111].slice();
  }

  // The 3-bit value for a palette color (null if not in palette).
  valueForColor(rgb) {
    const [r, g, b] = rgb;
    for (let i = 0; i < this.palette.length; i++) {
      const [pr, pg, pb] = this.palette[i];
      if (pr === r && pg === g && pb === b) return i;
    }
    return null;
  }

  // The nearest palette color to an arbitrary RGB, by squared Euclidean
  // distance (lower index breaks ties).
  nearestValue(rgb) {
    const v = this.valueForColor(rgb);
    if (v !== null) return v;
    const [r, g, b] = rgb;
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

  // The squared Euclidean distance in RGB space between two colors.
  distance2(rgbA, rgbB) {
    const [ar, ag, ab] = rgbA;
    const [br, bg, bb] = rgbB;
    const dr = ar - br;
    const dg = ag - bg;
    const db = ab - bb;
    return dr * dr + dg * dg + db * db;
  }

  // Whether two colors are perceptually distinct.
  isDistinct(rgbA, rgbB) {
    return this.distance2(rgbA, rgbB) > 0;
  }
}