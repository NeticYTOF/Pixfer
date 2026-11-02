// pixfer web reassembly ext — tracks chunk arrival order and detects gaps
// in the received-chunk sequence. Mirrors `pixfer-core/src/reassembly_ext.rs`.

export class ReassemblyExt {
  constructor(total) {
    this.total = total;
    this.received = new Set();
  }

  markReceived(idx) {
    if (idx < this.total) this.received.add(idx);
    return this;
  }

  isComplete() {
    return this.received.size >= this.total;
  }

  // Smallest missing chunk index, or null if complete.
  // JS Sets iterate in insertion order, not sorted like a Rust BTreeSet,
  // so we sort the received indices to find the first gap.
  nextMissing() {
    if (this.isComplete()) return null;
    const sorted = [...this.received].sort((a, b) => a - b);
    let expected = 0;
    for (const have of sorted) {
      if (have > expected) return expected;
      expected = have + 1;
    }
    return expected < this.total ? expected : null;
  }

  receivedCount() {
    return this.received.size;
  }
}