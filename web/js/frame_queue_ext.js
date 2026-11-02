// pixfer web extended frame queue — a bounded, priority-aware FIFO for the
// TX frame buffer.
// Mirrors `pixfer-core/src/frame_queue_ext.rs`.

// A queued TX frame.
export class QueuedFrame {
  constructor({ seq = 0, data = [], priority = 0 } = {}) {
    this.seq = seq;
    this.data = data;
    this.priority = priority;
  }
}

// A bounded, priority-aware FIFO queue for TX frames.
export class FrameQueueExt {
  constructor(capacity) {
    this.items = [];
    this.capacity = capacity;
  }

  // The queue capacity.
  capacityValue() {
    return this.capacity;
  }

  // The number of queued frames.
  size() {
    return this.items.length;
  }

  isEmpty() {
    return this.items.length === 0;
  }

  // The number of slots remaining.
  available() {
    return Math.max(0, this.capacity - this.items.length);
  }

  // Push a frame; returns false if the queue is full.
  push(frame) {
    if (this.items.length >= this.capacity) return false;
    this.items.push(frame);
    return true;
  }

  // Pop the highest-priority frame (FIFO within equal priority).
  pop() {
    if (this.items.length === 0) return null;
    let bestIdx = 0;
    for (let i = 1; i < this.items.length; i++) {
      if (this.items[i].priority > this.items[bestIdx].priority) bestIdx = i;
    }
    const f = this.items[bestIdx];
    this.items.splice(bestIdx, 1);
    return f;
  }

  // Peek at the highest-priority frame without removing it.
  peek() {
    if (this.items.length === 0) return null;
    let bestIdx = 0;
    for (let i = 1; i < this.items.length; i++) {
      if (this.items[i].priority > this.items[bestIdx].priority) bestIdx = i;
    }
    return this.items[bestIdx];
  }

  // Drop all queued frames.
  clear() {
    this.items.length = 0;
  }
}