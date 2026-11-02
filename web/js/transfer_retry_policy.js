// pixfer web transfer retry policy — fixed / linear / exponential ARQ
// retry schedules with deterministic jitter.
// Mirrors `pixfer-core/src/transfer_retry_policy.rs`.

export const RetryPolicyKind = Object.freeze({
  FIXED: "fixed",
  LINEAR: "linear",
  EXPONENTIAL: "exponential",
});

// A retry policy descriptor (kind, base delay, multiplier, cap, jitter).
export class RetryPolicy {
  constructor({ kind, baseMs = 100, multiplier = 1.0, capMs = 0, jitterMs = 0 } = {}) {
    this.kind = kind;
    this.baseMs = baseMs >>> 0;
    this.multiplier = multiplier;
    this.capMs = capMs >>> 0;
    this.jitterMs = jitterMs >>> 0;
  }

  // A fixed-delay policy.
  static fixed(baseMs) {
    return new RetryPolicy({ kind: RetryPolicyKind.FIXED, baseMs, capMs: baseMs });
  }

  // A linearly-increasing delay policy.
  static linear(baseMs, capMs) {
    return new RetryPolicy({ kind: RetryPolicyKind.LINEAR, baseMs, capMs });
  }

  // An exponentially-increasing delay policy.
  static exponential(baseMs, multiplier, capMs) {
    return new RetryPolicy({ kind: RetryPolicyKind.EXPONENTIAL, baseMs, multiplier, capMs });
  }

  // The delay (ms) before retry `attempt` (1-based), including any
  // deterministic jitter.
  delay(attempt) {
    const base = this.kind === RetryPolicyKind.FIXED
      ? this.baseMs
      : this.kind === RetryPolicyKind.LINEAR
        ? Math.min(this.baseMs * attempt, this.capMs)
        : Math.min(
            Math.floor(this.baseMs * Math.pow(this.multiplier, attempt - 1)),
            this.capMs,
          );
    const jitter = this.jitterMs === 0
      ? 0
      : attempt % (this.jitterMs + 1);
    return base + jitter;
  }

  // The total wait across `maxRetries` attempts.
  totalWait(maxRetries) {
    let total = 0;
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      total += this.delay(attempt);
    }
    return total;
  }
}