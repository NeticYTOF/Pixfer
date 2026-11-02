// pixfer web extended transfer progress — a fine-grained progress tracker
// with per-chunk granularity and ETA estimation.
// Mirrors `pixfer-core/src/transfer_progress_ext.rs`.

// A fine-grained progress snapshot.
export class ProgressSnapshotExt {
  constructor({ delivered = 0, total = 0, elapsedMs = 0 } = {}) {
    this.delivered = delivered;
    this.total = total;
    this.elapsedMs = elapsedMs;
  }
}

// Tracks transfer progress at chunk granularity with ETA.
export class TransferProgressExt {
  constructor(totalChunks) {
    this.delivered = 0;
    this.total = totalChunks;
    this.elapsedMs = 0;
  }

  // Mark a chunk as delivered.
  markDelivered() {
    if (this.delivered < this.total) this.delivered += 1;
  }

  // The current progress fraction (0.0..=1.0).
  fraction() {
    return this.total === 0 ? 0 : this.delivered / this.total;
  }

  // The current progress snapshot.
  snapshot() {
    return new ProgressSnapshotExt({
      delivered: this.delivered,
      total: this.total,
      elapsedMs: this.elapsedMs,
    });
  }

  // The estimated time remaining (ms), based on average per-chunk time.
  etaMs() {
    if (this.delivered === 0 || this.total === 0) return 0;
    const perChunk = Math.floor(this.elapsedMs / this.delivered);
    const remaining = this.total - this.delivered;
    return perChunk * remaining;
  }

  // Whether the transfer is complete.
  isComplete() {
    return this.delivered >= this.total;
  }

  // The total chunk count.
  totalValue() {
    return this.total;
  }

  // The delivered chunk count.
  deliveredCount() {
    return this.delivered;
  }

  // Set the elapsed time (ms).
  setElapsedMs(ms) {
    this.elapsedMs = ms;
  }
}