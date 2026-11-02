// pixfer web frame scheduler — orders and times the on-screen frames so
// the peer camera reliably captures them.
// Mirrors `pixfer-core/src/frame_scheduler.rs`.

import { FramePacer } from "./frame_pacer.js";

class RingQueue {
  constructor() {
    this.items = [];
  }

  push(item) {
    this.items.push(item);
  }

  peek() {
    return this.items.length > 0 ? this.items[0] : null;
  }

  pop() {
    return this.items.shift() ?? null;
  }

  size() {
    return this.items.length;
  }
}

// A frame scheduler: wraps a FramePacer plus a FIFO queue of pending frames.
export class FrameScheduler {
  constructor(fps) {
    this.pacer = new FramePacer(fps);
    this.queue = new RingQueue();
  }

  // Enqueue a frame (by sequence number) for display.
  enqueue(seq) {
    this.queue.push(seq);
  }

  // The next frame to display (in FIFO order), or null if the queue is empty.
  next() {
    return this.queue.peek();
  }

  // Pop and return the next frame to display.
  popNext() {
    return this.queue.pop();
  }

  // The minimum dwell time (ms) the current frame must stay on screen.
  minDwellMs() {
    return this.pacer.minDwellMs();
  }

  // The target fps.
  targetFps() {
    return this.pacer.targetFps; // FramePacer stores it as a property
  }

  // The number of frames waiting to be displayed.
  pendingCount() {
    return this.queue.size();
  }
}