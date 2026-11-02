// pixfer web metrics — counters, gauges, histograms (p50/p95/p99), and a
// throughput meter (MB/s). Mirrors `pixfer-core/src/metrics.rs`.

export class Counter {
  constructor() {
    this.value = 0;
  }

  inc(n = 1) {
    this.value += n;
    return this;
  }

  dec(n = 1) {
    this.value = Math.max(0, this.value - n);
    return this;
  }

  val() {
    return this.value;
  }
}

export class Gauge {
  constructor(initial = 0) {
    this.value = initial;
  }

  set(v) {
    this.value = v;
    return this;
  }

  add(delta) {
    this.value += delta;
    return this;
  }

  val() {
    return this.value;
  }
}

// Histogram with fixed edges; quantile estimate by walking the CDF.
export class Histogram {
  constructor(edges) {
    if (!Array.isArray(edges) || edges.length < 2) {
      throw new RangeError("histogram needs at least 2 edges");
    }
    this.edges = [...edges].sort((a, b) => a - b);
    this.buckets = new Array(this.edges.length - 1).fill(0);
    this.count = 0;
    this.sum = 0;
  }

  record(v) {
    this.count += 1;
    this.sum += v;
    const i = this.edges.findIndex((e) => v <= e);
    if (i >= 0) {
      this.buckets[i] += 1;
    } else {
      // Above the max edge -> overflow bucket.
      this.buckets[this.buckets.length - 1] += 1;
    }
  }

  quantile(q) {
    if (this.count === 0) return 0;
    const target = Math.max(0, Math.min(1, q)) * this.count;
    let seen = 0;
    for (let i = 0; i < this.buckets.length; i++) {
      seen += this.buckets[i];
      if (seen >= target) {
        // Return the upper edge of this bucket as the estimate.
        return this.edges[i + 1];
      }
    }
    return this.edges[this.edges.length - 1];
  }

  p50() {
    return this.quantile(0.5);
  }

  p95() {
    return this.quantile(0.95);
  }

  p99() {
    return this.quantile(0.99);
  }

  mean() {
    return this.count === 0 ? 0 : this.sum / this.count;
  }

  report() {
    return JSON.stringify({
      count: this.count,
      p50: this.p50(),
      p95: this.p95(),
      p99: this.p99(),
      mean: this.mean(),
    });
  }
}

// Throughput meter: bytes over a sliding time window -> MB/s.
export class ThroughputMeter {
  constructor() {
    this.bytes = 0;
    this.windowStart = Date.now();
    this._lastTs = this.windowStart;
  }

  add(bytes, tsMs = Date.now()) {
    // Keep a simple window: if the window is > 5s, compute + reset.
    this.bytes += bytes;
    this._lastTs = tsMs;
    if (tsMs - this.windowStart >= 5000) {
      const secs = (tsMs - this.windowStart) / 1000;
      if (secs > 0) {
        this._lastMbps = this.bytes / secs / 1_000_000;
      }
      this.bytes = 0;
      this.windowStart = tsMs;
      this._lastTs = tsMs;
    }
    return this;
  }

  mbps() {
    if (this.bytes === 0) return this._lastMbps || 0;
    // Use the last add timestamp for determinism (not a fresh Date.now()).
    const secs = (this._lastTs - this.windowStart) / 1000;
    if (secs <= 0) return this._lastMbps || 0;
    return this.bytes / secs / 1_000_000;
  }
}

// Facade tying the metric families together.
export class Metrics {
  constructor() {
    this.counters = new Map();
    this.gauges = new Map();
    this.histogram = new Histogram([1, 5, 10, 25, 50, 100, 250, 500, 1000, 5000]);
    this.throughput = new ThroughputMeter();
  }

  counter(name) {
    if (!this.counters.has(name)) this.counters.set(name, new Counter());
    return this.counters.get(name);
  }

  gauge(name) {
    if (!this.gauges.has(name)) this.gauges.set(name, new Gauge());
    return this.gauges.get(name);
  }

  recordLatency(ms) {
    this.histogram.record(ms);
  }

  recordBytes(n) {
    this.throughput.add(n);
  }

  // JSON-ish report of all metrics.
  report() {
    return JSON.stringify({
      counters: Object.fromEntries([...this.counters.entries()].map(([n, c]) => [n, c.val()])),
      gauges: Object.fromEntries([...this.gauges.entries()].map(([n, g]) => [n, g.val()])),
      histogram: JSON.parse(this.histogram.report()),
      throughput_mbps: this.throughput.mbps(),
    });
  }
}