// pixfer web link calibration store — per-link calibration values (color
// temperature target, per-channel gains, exposure lock).
// Mirrors `pixfer-core/src/link_calibration_store.rs`.

// A set of calibration values for one link.
export class CalibrationValues {
  constructor({ targetKelvin = 5500, gains = [1.0, 1.0, 1.0], exposure = 128 } = {}) {
    this.targetKelvin = targetKelvin;
    this.gains = gains.slice();
    this.exposure = exposure;
  }
}

// A store of calibration values keyed by link fingerprint.
export class CalibrationStore {
  constructor() {
    this.byLink = new Map();
  }

  // Store calibration values for a link.
  put(fingerprint, values) {
    this.byLink.set(fingerprint, values);
  }

  // Retrieve calibration values for a link (null if absent).
  get(fingerprint) {
    return this.byLink.has(fingerprint) ? this.byLink.get(fingerprint) : null;
  }

  // Whether calibration values exist for a link.
  contains(fingerprint) {
    return this.byLink.has(fingerprint);
  }

  // The number of calibrated links.
  size() {
    return this.byLink.size;
  }

  isEmpty() {
    return this.byLink.size === 0;
  }
}