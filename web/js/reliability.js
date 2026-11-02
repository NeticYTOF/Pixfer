// pixfer web reliability — goodput, loss rate, and a link-quality score that
// blends them. Mirrors `pixfer-core/src/reliability.rs`.

export class ReliabilitySnapshot {
  constructor(totalChunks, delivered, retries, naks) {
    this.total_chunks = totalChunks;
    this.delivered = delivered;
    this.retries = retries;
    this.naks = naks;
  }

  goodputFraction() {
    if (this.total_chunks === 0) return 0;
    return this.delivered / this.total_chunks;
  }

  retryFraction() {
    if (this.total_chunks === 0) return 0;
    return this.retries / this.total_chunks;
  }

  // A 0..1 link-quality score: goodput up, retries/NAKs down.
  qualityScore() {
    const goodput = this.goodputFraction();
    const penalty = this.retryFraction() * 0.5 + this.naks * 0.01;
    return Math.min(1, Math.max(0, goodput - penalty));
  }
}

export class ReliabilityTracker {
  constructor() {
    this.total_chunks = 0;
    this.delivered = 0;
    this.retries = 0;
    this.naks = 0;
  }

  markDelivered(totalChunks) {
    this.total_chunks = totalChunks;
    this.delivered = totalChunks;
  }

  markRetry() {
    this.retries += 1;
  }

  markNak() {
    this.naks += 1;
  }

  snapshot() {
    return new ReliabilitySnapshot(
      this.total_chunks,
      this.delivered,
      this.retries,
      this.naks
    );
  }

  qualityScore() {
    return this.snapshot().qualityScore();
  }
}