// pixfer web extended transfer stats — a richer per-transfer statistics
// record with throughput, retry, and FEC failure tracking.
// Mirrors `pixfer-core/src/transfer_stats_ext.rs`.

// An extended transfer statistics record.
export class TransferStatsExt {
  constructor() {
    this.bytes = 0;
    this.elapsedMs = 0;
    this.retries = 0;
    this.fecCorrections = 0;
    this.failedFrames = 0;
    this.deliveredChunks = 0;
    this.totalChunks = 0;
  }

  // The measured throughput (bytes/sec), 0 if no time elapsed.
  throughputBps() {
    return this.elapsedMs === 0 ? 0 : this.bytes / (this.elapsedMs / 1000);
  }

  // The fraction of chunks delivered (0.0 if no total).
  deliveredFraction() {
    return this.totalChunks === 0 ? 0 : this.deliveredChunks / this.totalChunks;
  }

  // The retry rate (retries per delivered chunk), 0 if no deliveries.
  retryRate() {
    return this.deliveredChunks === 0 ? 0 : this.retries / this.deliveredChunks;
  }

  // Whether the transfer completed successfully.
  isSuccess() {
    return this.deliveredChunks === this.totalChunks && this.failedFrames === 0;
  }

  // A JSON export of the stats.
  toJson() {
    return JSON.stringify({
      bytes: this.bytes,
      elapsed_ms: this.elapsedMs,
      retries: this.retries,
      fec_corrections: this.fecCorrections,
      failed_frames: this.failedFrames,
      delivered_chunks: this.deliveredChunks,
      total_chunks: this.totalChunks,
    });
  }
}