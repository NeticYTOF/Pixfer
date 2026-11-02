// pixfer web extended link monitor — a rolling health monitor that tracks
// goodput, latency, and a composite health score.
// Mirrors `pixfer-core/src/link_monitor_ext.rs`.

// A link-monitor sample.
export class MonitorSample {
  constructor({ goodputBps = 0, latencyMs = 0, tMs = 0 } = {}) {
    this.goodputBps = goodputBps;
    this.latencyMs = latencyMs;
    this.tMs = tMs;
  }
}

// A rolling link-health monitor.
export class LinkMonitorExt {
  constructor(windowSize) {
    this.window = Math.max(1, windowSize);
    this.samples = [];
  }

  // Record a sample (ring buffer of `window`).
  record(s) {
    this.samples.push(s);
    while (this.samples.length > this.window) this.samples.shift();
  }

  // The mean goodput (B/s) over the window (0 if no samples).
  meanGoodput() {
    if (this.samples.length === 0) return 0;
    return this.samples.reduce((a, s) => a + s.goodputBps, 0) / this.samples.length;
  }

  // The mean latency (ms) over the window (0 if no samples).
  meanLatency() {
    if (this.samples.length === 0) return 0;
    return this.samples.reduce((a, s) => a + s.latencyMs, 0) / this.samples.length;
  }

  // A composite health score in [0, 100]:
  // - 60% from goodput relative to a 6690 B/s reference
  // - 40% from latency (lower is better).
  healthScore() {
    const goodput = this.meanGoodput();
    const latency = this.meanLatency();
    const goodputScore = Math.min(goodput / 6690, 1.0) * 100;
    const latencyScore = (1 - Math.min(latency / 100, 1.0)) * 100;
    return Math.min(Math.max(goodputScore * 0.6 + latencyScore * 0.4, 0), 100);
  }

  // The number of samples in the window.
  size() {
    return this.samples.length;
  }
}