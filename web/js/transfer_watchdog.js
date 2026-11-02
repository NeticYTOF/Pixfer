// pixfer web transfer watchdog — stall detection for in-progress transfers.
// Mirrors `pixfer-core/src/transfer_watchdog.rs`.

// The state of the watchdog.
export const WatchdogState = Object.freeze({
  IDLE: "idle",
  ACTIVE: "active",
  TIMED_OUT: "timed_out",
});

// A watchdog that detects stalled transfers.
export class TransferWatchdog {
  constructor(timeoutMs = 5000) {
    this.timeoutMs = timeoutMs;
    this.lastProgressMs = 0;
    this.active = false;
    this.state = WatchdogState.IDLE;
    this.nowMs = 0;
  }

  // Mark the start of a transfer.
  start() {
    this.active = true;
    this.lastProgressMs = this.nowMs;
    this.state = WatchdogState.ACTIVE;
  }

  // Record progress (a chunk delivered, an ACK received, etc.).
  recordProgress() {
    if (this.active) {
      this.lastProgressMs = this.nowMs;
      this.state = WatchdogState.ACTIVE;
    }
  }

  // Poll the watchdog: returns true if the transfer has timed out.
  check() {
    if (!this.active) {
      this.state = WatchdogState.IDLE;
      return false;
    }
    if (this.nowMs - this.lastProgressMs >= this.timeoutMs) {
      this.state = WatchdogState.TIMED_OUT;
      return true;
    }
    this.state = WatchdogState.ACTIVE;
    return false;
  }

  // End the transfer (success or abort).
  stop() {
    this.active = false;
    this.state = WatchdogState.IDLE;
  }

  // The current state.
  getState() {
    return this.state;
  }

  // The configured timeout (ms).
  timeout() {
    return this.timeoutMs;
  }
}