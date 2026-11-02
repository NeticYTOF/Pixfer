// pixfer web color palette — the 8-color 3-bit alphabet and its inverse
// mapping. Mirrors `pixfer-core/src/color_palette.rs`.

export const PALETTE = Object.freeze([
  [0, 0, 0],       // 0 - black
  [255, 255, 255], // 1 - white
  [255, 0, 0],     // 2 - red
  [0, 255, 0],     // 3 - green
  [0, 0, 255],     // 4 - blue
  [255, 0, 255],   // 5 - magenta
  [255, 255, 0],   // 6 - yellow
  [128, 128, 128], // 7 - gray
]);

// Convert a 3-bit value (0..=7) to a [r,g,b] triple.
export function colorForValue(v) {
  return PALETTE[v & 0b111];
}

// Convert an [r,g,b] triple back to its 3-bit value, or null if not in the palette.
export function valueForColor(rgb) {
  const idx = PALETTE.findIndex(
    (c) => c[0] === rgb[0] && c[1] === rgb[1] && c[2] === rgb[2]
  );
  return idx >= 0 ? idx & 0b111 : null;
}

// The palette has 8 distinct colors.
export function paletteSize() {
  return PALETTE.length;
}