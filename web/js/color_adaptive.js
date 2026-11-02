// pixfer web adaptive color calibration — self-tuning 8-color palette.
// Mirrors `pixfer-core/src/color_adaptive.rs`.

// A measured color reading (normalized 0..=1).
// { r, g, b }

// An adaptive color calibration table: maps each 3-bit value to a
// device-specific calibrated RGB.
export class AdaptivePalette {
  // Build from an array of 8 measured colors (one per 3-bit value 0..=7).
  static fromMeasurements(measured) {
    const colors = new Array(8).fill(null).map(() => ({ r: 0, g: 0, b: 0 }));
    for (let i = 0; i < 8 && i < measured.length; i++) {
      const m = measured[i];
      colors[i] = {
        r: clampByte(m.r * 255),
        g: clampByte(m.g * 255),
        b: clampByte(m.b * 255),
      };
    }
    return new AdaptivePalette(colors);
  }

  constructor(colors) {
    this.colors = colors;
  }

  // The calibrated color for a 3-bit value.
  colorForValue(v) {
    return this.colors[v & 0b111];
  }

  // Whether the palette matches the default 8-color palette.
  isDefault() {
    const defaults = [
      { r: 0, g: 0, b: 0 },
      { r: 255, g: 255, b: 255 },
      { r: 255, g: 0, b: 0 },
      { r: 0, g: 255, b: 0 },
      { r: 0, g: 0, b: 255 },
      { r: 255, g: 0, b: 255 },
      { r: 255, g: 255, b: 0 },
      { r: 128, g: 128, b: 128 },
    ];
    for (let v = 0; v < 8; v++) {
      const c = this.colors[v];
      const d = defaults[v];
      if (c.r !== d.r || c.g !== d.g || c.b !== d.b) {
        return false;
      }
    }
    return true;
  }
}

function clampByte(x) {
  return Math.round(Math.max(0, Math.min(255, x)));
}

// Re-calibrate the sender's palette from camera measurements.
export function recalibrate(measured) {
  return AdaptivePalette.fromMeasurements(measured);
}