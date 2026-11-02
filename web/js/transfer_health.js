// pixfer web transfer health — EMA-based throughput/retry/fec monitor.
// Mirrors `pixfer-core/src/transfer_health.rs`.

// Tier names.
export const HealthTier = Object.freeze({
  HEALTHY: "healthy",
  DEGRADED: "degraded",
  UNHEALTHY: "unhealthy",
  UNKNOWN: "unknown",
});

// Feed a { throughputMbps, retries, fecFailed } sample.
export class TransferHealth {
  constructor(alpha = 0.3) {
    this.alpha = Math.max(0, Math.min(1, alpha));
    this.emaTp = 0;
    this.emaRetries = 0;
    this.emaFecFail = 0;
    this.samples = 0;
  }

  observe({ throughputMbps = 0, retries = 0, fecFailed = 0 } = {}) {
    this.emaTp = this.alpha * throughputMbps + (1 - this.alpha) * this.emaTp;
    this.emaRetries = this.alpha * retries + (1 - this.alpha) * this.emaRetries;
    this.emaFecFail = this.alpha * fecFailed + (1 - this.alpha) * this.emaFecFail;
    this.samples = Math.min(this.samples + 1, 0x7fffffff);
  }

  // Tier from the EMAs + a floor (MB/s).
  tier(floorMbps = 1.0) {
    if (this.samples === 0 || floorMbps <= 0) return HealthTier.UNKNOWN;
    const ratio = this.emaTp / floorMbps;
    if (ratio < 0.5 || this.emaFecFail > 10 || this.emaRetries > 5) {
      return HealthTier.UNHEALTHY;
    }
    if (ratio < 0.8) return HealthTier.DEGRADED;
    return HealthTier.HEALTHY;
  }
}