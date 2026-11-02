// pixfer web transfer heartbeat — keep-alive liveness tracking for a
// transfer session.
// Mirrors `pixfer-core/src/transfer_heartbeat.rs`.

// A heartbeat sample: timestamp (monotonic ms) and a sequence number.
export class HeartbeatSample {
  constructor({ tMs = 0, seq = 0 } = {}) {
    this.tMs = tMs;
    this.seq = seq;
  }
}

// Tracks heartbeat liveness and detects stale peers.
export class TransferHeartbeat {
  constructor(intervalMs, staleAfter) {
    this.intervalMs = intervalMs;
    this.staleAfter = staleAfter;
    this.last = null;
    this.missed = 0;
    this.beats = 0;
  }

  // Record a received heartbeat.
  record(sample) {
    if (this.last !== null) {
      const gap = Math.max(0, sample.tMs - this.last.tMs);
      if (gap > this.intervalMs) {
        this.missed += 1;
      } else {
        this.missed = 0;
      }
    }
    this.last = sample;
    this.beats += 1;
  }

  // The number of heartbeats recorded.
  beatCount() {
    return this.beats;
  }

  // Consecutive missed-interval count.
  missedCount() {
    return this.missed;
  }

  // Whether the peer should be considered stale.
  isStale() {
    return this.missed >= this.staleAfter;
  }

  // The last recorded heartbeat (null if none yet).
  lastSample() {
    return this.last;
  }

  // The configured heartbeat interval.
  interval() {
    return this.intervalMs;
  }
}