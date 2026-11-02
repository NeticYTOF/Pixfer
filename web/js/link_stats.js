// pixfer web link stats — per-link aggregate statistics: bytes, chunks,
// retries, and throughput. Mirrors `pixfer-core/src/link_stats.rs`.

export class LinkStats {
  constructor() {
    this.bytes_sent = 0;
    this.bytes_received = 0;
    this.chunks_total = 0;
    this.chunks_delivered = 0;
    this.retries = 0;
    this.elapsed_ms = 0;
  }

  addSentBytes(n) {
    this.bytes_sent += n;
    return this;
  }

  addReceivedBytes(n) {
    this.bytes_received += n;
    return this;
  }

  setElapsed(ms) {
    this.elapsed_ms = ms;
    return this;
  }

  // Effective MB/s for the sent direction (0 if no elapsed time).
  sentMbps() {
    if (this.elapsed_ms === 0) return 0;
    return this.bytes_sent / (this.elapsed_ms / 1000) / 1_048_576;
  }

  receivedMbps() {
    if (this.elapsed_ms === 0) return 0;
    return this.bytes_received / (this.elapsed_ms / 1000) / 1_048_576;
  }

  deliveryRatio() {
    if (this.chunks_total === 0) return 0;
    return this.chunks_delivered / this.chunks_total;
  }

  report() {
    return (
      `link_stats: sent=${this.bytes_sent}B recv=${this.bytes_received}B ` +
      `chunks=${this.chunks_delivered}/${this.chunks_total} ` +
      `retries=${this.retries} elapsed=${this.elapsed_ms}ms ` +
      `sent_mbps=${this.sentMbps().toFixed(3)} recv_mbps=${this.receivedMbps().toFixed(3)}`
    );
  }
}