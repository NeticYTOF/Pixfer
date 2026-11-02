// pixfer web camera focus controller — keep the receiver camera focused on
// the sender screen across a transfer.
// Mirrors `pixfer-core/src/camera_focus_controller.rs`.

// A focus sample: measured defocus level (0 = sharp, 1 = very blurry).
export class FocusSample {
  constructor({ defocus = 0, tMs = 0 } = {}) {
    this.defocus = defocus;
    this.tMs = tMs;
  }
}

// Tracks focus quality and counts refocus events.
export class CameraFocusController {
  constructor(blurThreshold) {
    this.blurThreshold = blurThreshold;
    this.last = null;
    this.refocuses = 0;
  }

  // Record a focus sample and count a refocus event if too blurry.
  record(sample) {
    if (sample.defocus >= this.blurThreshold) {
      this.refocuses += 1;
    }
    this.last = sample;
  }

  // The number of refocus events.
  refocusCount() {
    return this.refocuses;
  }

  // The last focus sample (null if none yet).
  lastSample() {
    return this.last;
  }

  // Whether the last sample was blurry (needs refocus).
  isBlurry() {
    return this.last !== null && this.last.defocus >= this.blurThreshold;
  }

  // The blur threshold.
  threshold() {
    return this.blurThreshold;
  }
}