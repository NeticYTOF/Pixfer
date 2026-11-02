// pixfer web progress — transfer progress tracker + ETA estimator that the
// tx/rx UIs poll. Mirrors `pixfer-core/src/progress.rs`.

export class ProgressSnapshot {
  constructor(received, total, bytesReceived, bytesTotal) {
    this.received = received;
    this.total = total;
    this.bytes_received = bytesReceived;
    this.bytes_total = bytesTotal;
  }

  fraction() {
    if (this.total === 0) return 0;
    return this.received / this.total;
  }

  percent() {
    return this.fraction() * 100;
  }

  remaining() {
    return this.total - this.received;
  }
}

export class ProgressTracker {
  constructor(totalChunks, bytesTotal) {
    this.received = 0;
    this.bytes_received = 0;
    this.total = totalChunks;
    this.bytes_total = bytesTotal;
    this.rate_bps = 0;
  }

  markReceived(chunks, bytes) {
    this.received += chunks;
    this.bytes_received += bytes;
  }

  setRate(bps) {
    this.rate_bps = Math.max(0, bps);
  }

  snapshot() {
    return new ProgressSnapshot(
      this.received,
      this.total,
      this.bytes_received,
      this.bytes_total
    );
  }

  // Estimated seconds remaining at the observed rate (null if rate unknown).
  etaSecs() {
    if (this.rate_bps <= 0) return null;
    const remainingBytes = this.bytes_total - this.bytes_received;
    return remainingBytes / this.rate_bps;
  }
}