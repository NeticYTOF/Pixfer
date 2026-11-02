// pixfer web telemetry — typed events + an in-memory pub/sub bus (built on
// the events emitter) + a counting recorder. Mirrors
// `pixfer-core/src/telemetry.rs`.

import { Emitter } from "./events.js";

// A telemetry event type (name + payload).
export class TelemetryEvent {
  constructor(name, data = {}) {
    this.name = name;
    this.data = data;
    this.ts = Date.now();
  }
}

// The standard event names (stable contract for dashboards).
export const TELEMETRY_NAMES = Object.freeze([
  "transfer.started",
  "transfer.completed",
  "transfer.failed",
  "chunk.sent",
  "chunk.acked",
  "chunk.received",
  "chunk.retry",
  "ack.timeout",
  "exposure.locked",
]);

// An in-memory pub/sub bus.
export class TelemetryBus {
  constructor() {
    this._emitter = new Emitter();
    this._subscribers = 0;
  }

  subscribe(fn) {
    this._subscribers += 1;
    this._emitter.on("*", (type, ...rest) => {
      const ev = new TelemetryEvent(type, rest[0] || {});
      fn(ev);
    });
    return this;
  }

  emit(name, data = {}) {
    this._emitter.emit(name, data);
    return this;
  }

  subscriberCount() {
    return this._subscribers;
  }
}

// A recorder that counts events by name and keeps the last event.
export class TelemetryRecorder {
  constructor() {
    this.counts = new Map();
    this.last = null;
  }

  record(ev) {
    this.counts.set(ev.name, (this.counts.get(ev.name) || 0) + 1);
    this.last = ev;
  }

  count(name) {
    return this.counts.get(name) || 0;
  }

  // Wire a recorder into a bus.
  attach(bus) {
    bus.subscribe((ev) => this.record(ev));
    return this;
  }

  report() {
    return JSON.stringify({
      counts: Object.fromEntries([...this.counts.entries()]),
      last: this.last ? this.last.name : null,
    });
  }
}