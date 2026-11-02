// pixfer web throughput meter — a windowed MB/s meter for live speed
// readouts. Mirrors `pixfer-core/src/throughput_meter.rs`.

export class ThroughputMeter2 {
  constructor(windowMs = 5000) {
    this.bytes = 0;
    this.windowStartMs = 0;
    this.lastMbps = 0;
    this.windowMs = windowMs;
  }

  add(n, nowMs) {
    this.bytes += n;
    if (nowMs - this.windowStartMs >= this.windowMs) {
      const secs = (nowMs - this.windowStartMs) / 1000;
      if (secs > 0) this.lastMbps = this.bytes / secs / 1_048_576;
      this.bytes = 0;
      this.windowStartMs = nowMs;
    }
    return this;
  }

  mbps() {
    return this.lastMbps;
  }

  reset(nowMs) {
    this.bytes = 0;
    this.windowStartMs = nowMs;
    this.lastMbps = 0;
    return this;
  }
}