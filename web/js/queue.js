// pixfer web queue — a prioritized work queue: FIFO within a priority
// class, lowest priority number served first. (Hilariously over-engineered:
// a full scheduling layer for a light link.)

export class WorkQueue {
  constructor() {
    this._lanes = new Map(); // priority -> FIFO array
  }

  push(priority, item) {
    const p = Number(priority) | 0;
    if (p < 0 || p > 255) throw new RangeError("priority must be 0..=255");
    let lane = this._lanes.get(p);
    if (!lane) {
      lane = [];
      this._lanes.set(p, lane);
    }
    lane.push(item);
    return this;
  }

  // Dequeue the most urgent item (lowest priority number, FIFO within).
  pop() {
    // Find the smallest priority with a non-empty lane.
    let bestP = null;
    for (const [p, lane] of this._lanes.entries()) {
      if (lane.length > 0 && (bestP === null || p < bestP)) bestP = p;
    }
    if (bestP === null) return undefined;
    const lane = this._lanes.get(bestP);
    const item = lane.shift();
    if (lane.length === 0) this._lanes.delete(bestP);
    return item;
  }

  peek() {
    let bestP = null;
    for (const [p, lane] of this._lanes.entries()) {
      if (lane.length > 0 && (bestP === null || p < bestP)) bestP = p;
    }
    if (bestP === null) return undefined;
    return this._lanes.get(bestP)[0];
  }

  len() {
    let n = 0;
    for (const lane of this._lanes.values()) n += lane.length;
    return n;
  }

  isEmpty() {
    return this.len() === 0;
  }

  minPriority() {
    let minP = null;
    for (const [p, lane] of this._lanes.entries()) {
      if (lane.length > 0 && (minP === null || p < minP)) minP = p;
    }
    return minP;
  }

  laneCounts() {
    const out = [];
    const ps = [...this._lanes.keys()].sort((a, b) => a - b);
    for (const p of ps) out.push([p, this._lanes.get(p).length]);
    return out;
  }

  drain() {
    const out = [];
    let item;
    while ((item = this.pop()) !== undefined) out.push(item);
    return out;
  }

  clear() {
    this._lanes.clear();
  }

  // JSON-ish report for debugging.
  report() {
    return JSON.stringify({
      queue_len: this.len(),
      lanes: this.laneCounts(),
      min_priority: this.minPriority(),
    });
  }
}

// A simple FIFO queue (priority 0 lane only).
export class FifoQueue {
  constructor(cap = Infinity) {
    this._items = [];
    this._cap = cap;
  }

  push(item) {
    this._items.push(item);
    if (this._items.length > this._cap) this._items.shift();
    return this;
  }

  pop() {
    return this._items.shift();
  }

  peek() {
    return this._items[0];
  }

  len() {
    return this._items.length;
  }

  isEmpty() {
    return this._items.length === 0;
  }

  drain() {
    const out = this._items;
    this._items = [];
    return out;
  }

  clear() {
    this._items = [];
  }

  report() {
    return JSON.stringify({ queue_len: this.len(), cap: this._cap });
  }
}

// A bounded ring buffer of items (drops the oldest when full).
export class RingBuffer {
  constructor(cap) {
    if (!Number.isInteger(cap) || cap < 1) throw new RangeError("cap must be a positive integer");
    this._buf = new Array(cap).fill(undefined);
    this._cap = cap;
    this._head = 0;
    this._size = 0;
  }

  push(item) {
    this._buf[(this._head + this._size) % this._cap] = item;
    if (this._size < this._cap) {
      this._size += 1;
    } else {
      this._head = (this._head + 1) % this._cap;
    }
    return this;
  }

  pop() {
    if (this._size === 0) return undefined;
    const item = this._buf[this._head];
    this._buf[this._head] = undefined;
    this._head = (this._head + 1) % this._cap;
    this._size -= 1;
    return item;
  }

  peek() {
    return this._size > 0 ? this._buf[this._head] : undefined;
  }

  len() {
    return this._size;
  }

  isEmpty() {
    return this._size === 0;
  }

  clear() {
    this._buf.fill(undefined);
    this._head = 0;
    this._size = 0;
  }

  // In-order contents, oldest first.
  toArray() {
    const out = [];
    for (let i = 0; i < this._size; i++) {
      out.push(this._buf[(this._head + i) % this._cap]);
    }
    return out;
  }

  report() {
    return JSON.stringify({ size: this._size, cap: this._cap });
  }
}