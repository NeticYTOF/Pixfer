// pixfer web extended transfer resume — a resumable transfer state that
// tracks which chunks have been delivered.
// Mirrors `pixfer-core/src/transfer_resume_ext.rs`.

// A resume point: which chunk index to continue from.
export class ResumePointExt {
  constructor({ nextChunk = 0, totalChunks = 0, delivered = 0 } = {}) {
    this.nextChunk = nextChunk;
    this.totalChunks = totalChunks;
    this.delivered = delivered;
  }
}

// Tracks resumable transfer state.
export class TransferResumeExt {
  constructor(totalChunks) {
    this.nextChunk = 0;
    this.totalChunks = totalChunks;
    this.delivered = 0;
  }

  // Mark chunk `idx` as delivered and advance the resume point.
  markDelivered(idx) {
    if (idx < this.totalChunks && idx === this.nextChunk) {
      this.delivered += 1;
      this.nextChunk += 1;
    }
  }

  // The current resume point.
  resumePoint() {
    return new ResumePointExt({ nextChunk: this.nextChunk, totalChunks: this.totalChunks, delivered: this.delivered });
  }

  // Whether the transfer is complete.
  isComplete() {
    return this.nextChunk >= this.totalChunks;
  }

  // The number of chunks remaining.
  remaining() {
    return Math.max(0, this.totalChunks - this.nextChunk);
  }

  // The status string.
  status() {
    if (this.totalChunks === 0) return "empty";
    if (this.isComplete()) return "complete";
    return "in_progress";
  }
}