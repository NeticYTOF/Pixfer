// pixfer web retry — fixed / linear / exponential backoff strategies plus
// a retry budget guard. Mirrors `pixfer-core/src/policy.rs`.

export const BackoffKind = Object.freeze({
  FIXED: "fixed",
  LINEAR: "linear",
  EXPONENTIAL: "exponential",
});

// Base + cap in ms.
export class FixedDelay {
  constructor(baseMs = 250) {
    this.baseMs = baseMs;
  }

  delay(attempt) {
    return this.baseMs;
  }

  name() {
    return BackoffKind.FIXED;
  }
}

export class LinearBackoff {
  constructor(baseMs = 200) {
    this.baseMs = baseMs;
  }

  delay(attempt) {
    return this.baseMs * attempt;
  }

  name() {
    return BackoffKind.LINEAR;
  }
}

export class ExponentialBackoff {
  constructor(baseMs = 100, capMs = 4000) {
    this.baseMs = baseMs;
    this.capMs = capMs;
  }

  delay(attempt) {
    const shift = Math.min(attempt - 1, 20);
    const mult = 1 << shift;
    return Math.min(this.baseMs * mult, this.capMs);
  }

  name() {
    return BackoffKind.EXPONENTIAL;
  }
}

// Pick a backoff strategy from a profile (mirrors pick_policy in policy.rs).
export function pickBackoff(profile) {
  switch (profile) {
    case "low_power":
      return new ExponentialBackoff(100, 4000);
    case "max_throughput":
      return new FixedDelay(250);
    case "balanced":
    default:
      return new LinearBackoff(200);
  }
}

// A hard cap on retry attempts.
export class RetryBudget {
  constructor(maxRetries) {
    this.maxRetries = maxRetries | 0;
    this.attempts = 0;
  }

  canRetry() {
    return this.attempts < this.maxRetries;
  }

  recordAttempt() {
    if (this.attempts < 0x7fffffff) this.attempts += 1;
  }

  exhausted() {
    return this.attempts >= this.maxRetries;
  }

  remaining() {
    return this.maxRetries - this.attempts;
  }
}

// Drive a full retry schedule.
export class RetryScheduler {
  constructor(policy, maxRetries) {
    this.policy = policy;
    this.budget = new RetryBudget(maxRetries);
  }

  // The full delay schedule (ms) for up to maxRetries attempts.
  schedule() {
    const out = [];
    for (let attempt = 1; attempt <= this.budget.maxRetries; attempt++) {
      out.push(this.policy.delay(attempt));
    }
    return out;
  }

  totalWaitMs() {
    return this.schedule().reduce((a, b) => a + b, 0);
  }

  policyName() {
    return this.policy.name();
  }

  // One retry cycle: record the attempt and return the next delay.
  nextDelay() {
    if (!this.budget.canRetry()) return null;
    this.budget.recordAttempt();
    return this.policy.delay(this.budget.attempts);
  }

  report() {
    return JSON.stringify({
      policy: this.policyName(),
      max_retries: this.budget.maxRetries,
      attempts: this.budget.attempts,
      total_wait_ms: this.totalWaitMs(),
    });
  }
}

// A generic async retry helper with a backoff policy.
export function withRetry(fn, { policy, maxRetries, onRetry } = {}) {
  const p = policy || new LinearBackoff(200);
  const budget = new RetryBudget(maxRetries ?? 3);
  const tryOnce = async () => {
    try {
      return await fn();
    } catch (err) {
      if (!budget.canRetry()) throw err;
      budget.recordAttempt();
      const delayMs = p.delay(budget.attempts);
      if (onRetry) onRetry({ attempt: budget.attempts, delayMs, error: err });
      await new Promise((r) => setTimeout(r, delayMs));
      return tryOnce();
    }
  };

  return tryOnce();
}