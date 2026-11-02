// pixfer web frame dedup — avoid retransmitting frames the receiver
// already has, keyed by frame sequence number.
// Mirrors `pixfer-core/src/frame_dedup.rs`.

// Tracks which frames have already been delivered, so the sender can skip
// retransmitting them.
export class FrameDedup {
  constructor() {
    this.delivered = new Set();
  }

  // Record that frame `seq` was delivered.
  markDelivered(seq) {
    this.delivered.add(seq);
  }

  // Whether frame `seq` was already delivered.
  isDelivered(seq) {
    return this.delivered.has(seq);
  }

  // The number of unique frames delivered.
  size() {
    return this.delivered.size;
  }

  isEmpty() {
    return this.delivered.size === 0;
  }

  // Clear the dedup set.
  clear() {
    this.delivered.clear();
  }
}