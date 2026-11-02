// pixfer web color-temperature correction — white-balance compensation
// for the 8-color palette. Mirrors `pixfer-core/src/color_temperature.rs`.

// A color-temperature reading (Kelvin).
// { kelvin, valid }

// A white-balance corrector that shifts the 8-color palette to
// compensate for a color-temperature offset.
export class TemperatureCorrector {
  constructor(targetKelvin = 5500) {
    this.targetKelvin = targetKelvin;
    this.gainR = 1.0;
    this.gainG = 1.0;
    this.gainB = 1.0;
  }

  // Adjust the per-channel gains to compensate for a measured temperature.
  compensate(reading) {
    if (!reading.valid || reading.kelvin === 0) return;
    const dev = (reading.kelvin - this.targetKelvin) / 1000;
    const d = Math.max(-1, Math.min(1, dev));
    this.gainR = clampGain(1.0 + 0.2 * d);
    this.gainG = 1.0;
    this.gainB = clampGain(1.0 - 0.2 * d);
  }

  // The corrected 8-color palette after compensation.
  correctedPalette() {
    const defaults = [
      [0, 0, 0], [255, 255, 255], [255, 0, 0], [0, 255, 0],
      [0, 0, 255], [255, 0, 255], [255, 255, 0], [128, 128, 128],
    ];
    return defaults.map(([r, g, b]) => ({
      r: clampByte(r * this.gainR),
      g: clampByte(g * this.gainG),
      b: clampByte(b * this.gainB),
    }));
  }

  // The current per-channel gains.
  gains() {
    return { r: this.gainR, g: this.gainG, b: this.gainB };
  }

  target() {
    return this.targetKelvin;
  }
}

function clampGain(x) {
  return Math.max(0.5, Math.min(1.5, x));
}

function clampByte(x) {
  return Math.round(Math.max(0, Math.min(255, x)));
}