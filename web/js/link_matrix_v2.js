// pixfer web link matrix v2 — a per-link, per-metric rolling store with
// trend detection.
// Mirrors `pixfer-core/src/link_matrix_v2.rs`.

// A rolling-window metric sample.
export class MetricSample {
  constructor({ metric = "", value = 0, tMs = 0 } = {}) {
    this.metric = metric;
    this.value = value;
    this.tMs = tMs;
  }
}

// A per-link, per-metric rolling store with trend detection.
export class LinkMatrixV2 {
  constructor(windowSize) {
    this.rings = new Map(); // fingerprint -> (metric -> ring[])
    this.window = Math.max(1, windowSize);
  }

  // The window size.
  windowSize() {
    return this.window;
  }

  // The number of links tracked.
  linkCount() {
    return this.rings.size;
  }

  // Record a metric sample for a link (ring buffer of `window`).
  record(fingerprint, sample) {
    if (!this.rings.has(fingerprint)) this.rings.set(fingerprint, new Map());
    const metrics = this.rings.get(fingerprint);
    if (!metrics.has(sample.metric)) metrics.set(sample.metric, []);
    const ring = metrics.get(sample.metric);
    ring.push(sample.value);
    while (ring.length > this.window) ring.shift();
  }

  // The most recent value for a metric on a link (null if absent).
  latest(fingerprint, metric) {
    const metrics = this.rings.get(fingerprint);
    if (!metrics || !metrics.has(metric)) return null;
    const ring = metrics.get(metric);
    return ring.length ? ring[ring.length - 1] : null;
  }

  // The rolling mean of a metric for a link (null if no samples).
  rollingMean(fingerprint, metric) {
    const metrics = this.rings.get(fingerprint);
    if (!metrics || !metrics.has(metric)) return null;
    const ring = metrics.get(metric);
    if (!ring || ring.length === 0) return null;
    return ring.reduce((a, b) => a + b, 0) / ring.length;
  }

  // A simple linear trend: (last - first) over the window (null if <2 samples).
  trend(fingerprint, metric) {
    const metrics = this.rings.get(fingerprint);
    if (!metrics || !metrics.has(metric)) return null;
    const ring = metrics.get(metric);
    if (!ring || ring.length < 2) return null;
    return ring[ring.length - 1] - ring[0];
  }

  // Remove a link's metrics.
  dropLink(fingerprint) {
    this.rings.delete(fingerprint);
  }
}