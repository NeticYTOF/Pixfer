// pixfer web transfer resume — checkpoint / resume for interrupted
// transfers. Mirrors `pixfer-core/src/transfer_resume.rs`.

// A resume point: the next chunk index to send.
class ResumePoint {
  constructor(nextChunk, totalChunks) {
    this.nextChunk = nextChunk;
    this.totalChunks = totalChunks;
  }
}

// A resume manager for interrupted transfers.
export class TransferResume {
  constructor(totalChunks) {
    this.total = totalChunks | 0;
    this.received = new Set();
    this.emitted = 0; // count of in-order delivered chunks
  }

  // Mark a chunk as delivered (received + verified); advances the in-order
  // resume point.
  markDelivered(idx) {
    if (idx < this.total) this.received.add(idx);
    // Advance the resume point: walk up from the last emitted count while
    // the next index is in `received`.
    while (this.emitted < this.total && this.received.has(this.emitted)) {
      this.received.delete(this.emitted);
      this.emitted++;
    }
  }

  // The resume point: the next in-order chunk to send.
  resumePoint() {
    if (this.total === 0) return null;
    return new ResumePoint(this.emitted, this.total);
  }

  isComplete() {
    return this.emitted >= this.total && this.total > 0;
  }

  deliveredCount() {
    return this.emitted;
  }

  status() {
    const rp = this.resumePoint();
    return rp ? `resume from chunk ${rp.nextChunk}/${rp.totalChunks}` : "no resume point";
  }
}