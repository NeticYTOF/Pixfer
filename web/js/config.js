// pixfer web config — a Config value object, named ConfigProfiles, a
// ConfigBuilder, and a feature registry. Mirrors `pixfer-core/src/config.rs`
// and `pixfer-core/src/features.rs`.

export const DEFAULT_THROUGHPUT = 2_000_000; // 2 MB/s
export const ZIP_THRESHOLD_S = 30; // auto-zip boundary (60 MB at default)

export const ConfigProfile = Object.freeze({
  LOW_POWER: "low_power",
  BALANCED: "balanced",
  MAX_THROUGHPUT: "max_throughput",
});

export class Config {
  constructor() {
    this.profile = ConfigProfile.BALANCED;
    this.defaultThroughput = DEFAULT_THROUGHPUT;
    this.zipThresholdS = ZIP_THRESHOLD_S;
    this.maxRetries = 3;
    this.ackTimeoutMs = 2500;
    this.blockPx = 4;
    this.lockExposure = false;
    this.throughputFloorMbps = 1.0;
    this.cameraWidth = 640;
    this.cameraHeight = 480;
  }

  // Build a config from a named profile (mirrors Config::for_profile).
  static forProfile(profile) {
    const c = new Config();
    c.profile = profile;
    switch (profile) {
      case ConfigProfile.LOW_POWER:
        c.defaultThroughput = 1_000_000;
        c.maxRetries = 5;
        c.ackTimeoutMs = 4000;
        c.throughputFloorMbps = 0.5;
        break;
      case ConfigProfile.MAX_THROUGHPUT:
        c.defaultThroughput = 4_000_000;
        c.maxRetries = 2;
        c.ackTimeoutMs = 1500;
        c.throughputFloorMbps = 2.0;
        c.lockExposure = true;
        break;
      case ConfigProfile.BALANCED:
      default:
        break;
    }
    return c;
  }

  // Strict validation; returns an array of problems (empty = valid).
  validate() {
    const problems = [];
    if (this.defaultThroughput <= 0) problems.push("defaultThroughput must be > 0");
    if (this.zipThresholdS <= 0) problems.push("zipThresholdS must be > 0");
    if (this.maxRetries < 1) problems.push("maxRetries must be >= 1");
    if (this.ackTimeoutMs <= 0) problems.push("ackTimeoutMs must be > 0");
    if (this.blockPx <= 0) problems.push("blockPx must be > 0");
    if (this.cameraWidth <= 0 || this.cameraHeight <= 0) {
      problems.push("camera dimensions must be > 0");
    }
    return problems;
  }

  // Effective needsZip using this config's throughput + threshold.
  needsZip(size) {
    const est = size / this.defaultThroughput;
    return est > this.zipThresholdS;
  }

  estimatedSeconds(size) {
    return size / this.defaultThroughput;
  }

  // JSON-ish report.
  report() {
    return JSON.stringify({
      profile: this.profile,
      default_throughput: this.defaultThroughput,
      zip_threshold_s: this.zipThresholdS,
      max_retries: this.maxRetries,
      ack_timeout_ms: this.ackTimeoutMs,
      block_px: this.blockPx,
      lock_exposure: this.lockExposure,
      throughput_floor_mbps: this.throughputFloorMbps,
      camera: [this.cameraWidth, this.cameraHeight],
    });
  }
}

// Fluent builder mirroring the profile presets but allowing per-field
// overrides — the classic "I added a builder for a struct with a default"
// flavor of over-engineering.
export class ConfigBuilder {
  constructor() {
    this.profile = null;
    this.defaultThroughput = null;
    this.zipThresholdS = null;
    this.maxRetries = null;
    this.ackTimeoutMs = null;
    this.blockPx = null;
    this.lockExposure = null;
    this.throughputFloorMbps = null;
    this.cameraWidth = null;
    this.cameraHeight = null;
  }

  withProfile(p) {
    this.profile = p;
    return this;
  }

  throughput(v) {
    this.defaultThroughput = v;
    return this;
  }

  zipThreshold(v) {
    this.zipThresholdS = v;
    return this;
  }

  maxRetries(v) {
    this.maxRetries = v;
    return this;
  }

  ackTimeout(v) {
    this.ackTimeoutMs = v;
    return this;
  }

  blockPx(v) {
    this.blockPx = v;
    return this;
  }

  lockExposure(v) {
    this.lockExposure = v;
    return this;
  }

  throughputFloor(v) {
    this.throughputFloorMbps = v;
    return this;
  }

  cameraResolution(w, h) {
    this.cameraWidth = w;
    this.cameraHeight = h;
    return this;
  }

  build() {
    const base = this.profile ? Config.forProfile(this.profile) : new Config();
    return {
      profile: this.profile || base.profile,
      defaultThroughput: this.defaultThroughput ?? base.defaultThroughput,
      zipThresholdS: this.zipThresholdS ?? base.zipThresholdS,
      maxRetries: this.maxRetries ?? base.maxRetries,
      ackTimeoutMs: this.ackTimeoutMs ?? base.ackTimeoutMs,
      blockPx: this.blockPx ?? base.blockPx,
      lockExposure: this.lockExposure ?? base.lockExposure,
      throughputFloorMbps: this.throughputFloorMbps ?? base.throughputFloorMbps,
      cameraWidth: this.cameraWidth ?? base.cameraWidth,
      cameraHeight: this.cameraHeight ?? base.cameraHeight,
    };
  }
}

// ---------- feature registry (mirrors features.rs) ----------

const FEATURES = [
  { name: "auto_zip", desc: "auto zip/unzip when estimated time exceeds the threshold", enabled: true, def: true, risk: 1 },
  { name: "live_speed_readout", desc: "live MB/s readouts on the tx/rx views", enabled: true, def: true, risk: 0 },
  { name: "exposure_lock", desc: "attempt a manual exposure lock on camera open", enabled: false, def: false, risk: 1 },
  { name: "rs_fec", desc: "Reed-Solomon(255,223,16) forward error correction", enabled: true, def: true, risk: 2 },
  { name: "crc32", desc: "per-chunk CRC32 integrity", enabled: true, def: true, risk: 1 },
  { name: "multicolor", desc: "8-color 3-bit/block grid (vs 1-bit OOK)", enabled: true, def: true, risk: 2 },
];

export class FeatureRegistry {
  constructor() {
    this.features = new Map();
    for (const f of FEATURES) this.features.set(f.name, { ...f });
  }

  get(name) {
    return this.features.get(name) || null;
  }

  isEnabled(name) {
    const f = this.features.get(name);
    return f ? f.enabled : false;
  }

  enable(name) {
    const f = this.features.get(name);
    if (f) f.enabled = true;
  }

  disable(name) {
    const f = this.features.get(name);
    if (f) f.enabled = false;
  }

  reset(name) {
    const f = this.features.get(name);
    if (f) f.enabled = f.def;
  }

  enabledCount() {
    let n = 0;
    for (const f of this.features.values()) if (f.enabled) n += 1;
    return n;
  }

  enabledFeatures() {
    const out = [];
    for (const f of this.features.values()) if (f.enabled) out.push(f);
    return out;
  }

  summary() {
    const names = [...this.features.entries()]
      .filter(([, f]) => f.enabled)
      .map(([n]) => n);
    return names.length ? `features: ${names.join(", ")}` : "features: (none)";
  }

  toJson() {
    const lines = [];
    for (const [n, f] of this.features.entries()) {
      const obj = JSON.stringify({ enabled: f.enabled, risk: f.risk, min_version: 1 });
      lines.push(`  "${n}": ${obj}`);
    }
    return `{\n${lines.join(",\n")}\n}`;
  }
}

export function standardFeatures() {
  return new FeatureRegistry();
}