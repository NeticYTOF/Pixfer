// pixfer web RX exposure lock — keep the receiver camera's exposure steady
// during a transfer so the 8-color palette stays stable.
// Mirrors `pixfer-core/src/rx_exposure_lock.rs`.

// Locks the receiver camera's exposure to a stable value for the duration
// of a transfer.
export class RxCameraExposureLock {
  constructor() {
    this.locked = false;
    this.target = 128;
  }

  // Lock exposure to `target`.
  lock(target) {
    this.target = target;
    this.locked = true;
  }

  // Release the lock (camera returns to auto-exposure).
  unlock() {
    this.locked = false;
  }

  // Whether the lock is active.
  isLocked() {
    return this.locked;
  }

  // The locked exposure value.
  targetValue() {
    return this.target;
  }
}

// Compute a target exposure from a measured scene luminance.
// The model: exposure inversely proportional to scene brightness, clamped
// to 0..=255.
export function suggestExposure(sceneLuma) {
  const luma = Math.min(Math.max(sceneLuma, 1), 255);
  const raw = 40000 / luma;
  return Math.min(Math.max(Math.floor(raw), 0), 255);
}