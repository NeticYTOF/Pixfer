// pixfer web extended transfer watchdog — a staler watchdog with
// configurable grace periods and stall detection.
// Mirrors `pixfer-core/src/transfer_watchdog_ext.rs`.

// The watchdog state (mirrors transfer_watchdog.js states).
export const WatchdogState = Object.freeze({
  IDLE: "idle",
  ACTIVE: "active",
  TIMED_OUT: "timed_out",
});

// An extended watchdog with grace periods and stall detection.
export class TransferWatchdogExt {
  constructor(timeoutMs, graceMs) {
    this.timeoutMs = timeoutMs;
    this.graceMs = graceMs;
    this.fired = 0;
    this.stalled = false;
    this.state = WatchdogState.ACTIVE;
  }

  // The effective timeout including grace (ms).
  effectiveTimeoutMs() {
    return this.timeoutMs + this.graceMs;
  }

  // The number of times the watchdog has fired.
  fireCount() {
    return this.fired;
  }

  // Whether the transfer is stalled.
  isStalled() {
    return this.stalled;
  }

  // The current watchdog state.
  stateValue() {
    return this.state;
  }

  // Mark progress (resets the stall timer).
  noteProgress() {
    this.stalled = false;
    this.state = WatchdogState.ACTIVE;
  }

  // Mark a stall (no progress beyond the grace period).
  noteStall() {
    this.stalled = true;
    this.state = WatchdogState.TIMED_OUT;
    this.fired += 1;
  }

  // Reset the watchdog.
  reset() {
    this.fired = 0;
    this.stalled = false;
    this.state = WatchdogState.ACTIVE;
  }
}