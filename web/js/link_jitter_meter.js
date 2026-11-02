// pixfer web link jitter meter — measure the variance in inter-frame
// arrival times on the receiver side.
// Mirrors `pixfer-core/src/link_jitter_meter.rs`.

import { FramePacer } from "./frame_pacer.js";

// A rolling jitter meter for link timing.
export class LinkJitterMeter {
  constructor(fps) {
    this.pacer = new FramePacer(fps);
    this.targetFps = fps;
  }

  // The target fps.
  fpsValue() {
    return this.targetFps;
  }

  // Record an observed inter-frame interval (ms); updates rolling jitter.
  observe(intervalMs) {
    this.pacer.observeInterval(intervalMs);
  }

  // The current rolling jitter (ms).
  jitterMs() {
    return this.pacer.jitterMs;
  }

  // The camera frame period (ms) implied by the target fps.
  framePeriodMs() {
    return 1000 / Math.max(1, this.targetFps);
  }

  // Whether jitter is within an acceptable bound.
  isStable(maxJitterMs) {
    return this.jitterMs() <= maxJitterMs;
  }
}