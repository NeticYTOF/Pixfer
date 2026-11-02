// pixfer web link matrix — a per-link, per-metric scorecard with JSON
// export. Mirrors `pixfer-core/src/link_matrix_ext.rs`.

// A single metric cell in the link matrix.
export class MetricCellExt {
  constructor({ value = 0, tMs = 0 } = {}) {
    this.value = value;
    this.tMs = tMs;
  }
}

// A per-link scorecard of metrics (throughput, latency, goodput, ...).
export class LinkMatrixExt {
  constructor() {
    this.byLink = new Map();
  }

  // The number of links tracked.
  linkCount() {
    return this.byLink.size;
  }

  // Record a metric value for a link.
  set(fingerprint, metric, value, tMs) {
    if (!this.byLink.has(fingerprint)) this.byLink.set(fingerprint, new Map());
    this.byLink.get(fingerprint).set(metric, new MetricCellExt({ value, tMs }));
  }

  // Retrieve a metric value for a link (null if absent).
  get(fingerprint, metric) {
    const m = this.byLink.get(fingerprint);
    return m && m.has(metric) ? m.get(metric) : null;
  }

  // All metrics recorded for a link (null if the link is unknown).
  metricsFor(fingerprint) {
    return this.byLink.get(fingerprint) || null;
  }

  // A JSON export of the whole matrix.
  toJson() {
    const parts = [];
    for (const [fp, metrics] of this.byLink.entries()) {
      const m = [];
      for (const [name, cell] of metrics.entries()) {
        const v = Number.isInteger(cell.value)
          ? String(cell.value)
          : cell.value.toFixed(4);
        m.push(`"${name}":{"value":${v},"t_ms":${cell.tMs}}`);
      }
      parts.push(`"${fp}":{${m.join(",")}`);
    }
    return `[${parts.join(",")}]`;
  }
}