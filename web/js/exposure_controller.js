// pixfer web exposure controller — PID-flavored exposure governor.
// Mirrors `pixfer-core/src/exposure_controller.rs`.

// Create with target luminance + PID gains.
export class ExposureController {
  constructor(targetLuma = 0.5, { kp = 128, ki = 2, kd = 4 } = {}) {
    this.targetLuma = Math.max(0, Math.min(1, targetLuma));
    this.kp = kp;
    this.ki = ki;
    this.kd = kd;
    this.integral = 0;
    this.prevErr = 0;
    this.hasPrev = false;
  }

  // Compute the next exposure value (0..=255) from a reading.
  nextExposure(reading) {
    if (reading.locked) return reading.value;
    const err = this.targetLuma - reading.avgLuma;
    this.integral = Math.max(-10, Math.min(10, this.integral + err));
    const deriv = this.hasPrev ? err - this.prevErr : 0;
    this.prevErr = err;
    this.hasPrev = true;
    const out = this.kp * err + this.ki * this.integral + this.kd * deriv;
    const next = (reading.value + out);
    return Math.round(Math.max(0, Math.min(255, next)));
  }

  // Reset the integral + derivative state (e.g. on link re-acquire).
  reset() {
    this.integral = 0;
    this.prevErr = 0;
    this.hasPrev = false;
  }

  targetLuma() {
    return this.targetLuma;
  }
}

// A convenience: lock exposure to a fixed value.
export function lockedExposure(value) {
  return { value: value | 0, locked: true, avgLuma: 0.5 };
}