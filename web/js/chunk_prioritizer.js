// pixfer web chunk prioritizer — priority queue for pending chunks.
// Mirrors `pixfer-core/src/chunk_prioritizer.rs`.

export class ChunkPrioritizer {
  constructor() {
    this.queue = [];
  }

  push({ chunkIdx, size = 223, priority = 1, queuedMs = 0 } = {}) {
    this.queue.push({ chunkIdx, size, priority: priority | 0, queuedMs });
  }

  pop() {
    if (this.queue.length === 0) return null;
    let best = 0;
    for (let i = 0; i < this.queue.length; i++) {
      const c = this.queue[i];
      const b = this.queue[best];
      if (c.priority > b.priority || (c.priority === b.priority && c.queuedMs < b.queuedMs)) {
        best = i;
      }
    }
    const c = this.queue[best];
    this.queue.splice(best, 1);
    return c;
  }

  len() {
    return this.queue.length;
  }

  isEmpty() {
    return this.queue.length === 0;
  }

  maxPriority() {
    if (this.queue.length === 0) return 0;
    return Math.max(...this.queue.map((c) => c.priority));
  }
}