// pixfer web chunk reorder buffer — emit chunks in sequence order.
// Mirrors `pixfer-core/src/chunk_reorder.rs`.

export class ChunkReorder {
  constructor(total) {
    this.total = total | 0;
    this.received = [];
    this.emitted = [];
  }

  // Record a received chunk index (duplicates / out-of-range ignored).
  receive(idx) {
    if (idx < this.total && !this.received.includes(idx)) {
      this.received.push(idx);
    }
  }

  // Emit the longest in-order prefix (0, 1, 2, ...).
  drainInOrder() {
    const out = [];
    let nextIdx = this.emitted.length;
    while (nextIdx < this.total && this.received.includes(nextIdx)) {
      out.push(nextIdx);
      this.emitted.push(nextIdx);
      this.received = this.received.filter((i) => i !== nextIdx);
      nextIdx += 1;
    }
    return out;
  }

  receivedCount() {
    return this.received.length;
  }

  emittedCount() {
    return this.emitted.length;
  }

  complete() {
    return this.emitted.length >= this.total && this.total > 0;
  }

  nextMissing() {
    if (this.complete()) return null;
    const emitted = this.emitted.length;
    return emitted < this.total ? emitted : null;
  }
}