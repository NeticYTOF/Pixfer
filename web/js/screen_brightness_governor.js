// pixfer web screen brightness governor — regulate the sender screen's
// brightness so the receiver camera can capture the 8-color palette.
// Mirrors `pixfer-core/src/screen_brightness_governor.rs`.

// A brightness reading from the sender screen.
export class BrightnessReading {
  constructor({ level = 0, tMs = 0 } = {}) {
    this.level = level;
    this.tMs = tMs;
  }
}

// Regulates screen brightness toward a target so colors stay visible.
export class ScreenBrightnessGovernor {
  constructor(target) {
    this.target = target;
    this.last = null;
    this.adjustments = 0;
  }

  // The target brightness.
  targetValue() {
    return this.target;
  }

  // Record a brightness reading and count an adjustment if off-target.
  record(reading) {
    if (reading.level !== this.target) {
      this.adjustments += 1;
    }
    this.last = reading;
  }

  // The number of brightness adjustments made.
  adjustmentCount() {
    return this.adjustments;
  }

  // The last reading (null if none yet).
  lastReading() {
    return this.last;
  }

  // Whether the last reading matches the target.
  atTarget() {
    return this.last !== null && this.last.level === this.target;
  }
}