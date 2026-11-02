// pixfer web link integrity report — aggregate CRC/RS check results across
// a transfer into a single health report.
// Mirrors `pixfer-core/src/link_integrity_report.rs`.

// A single integrity-check outcome.
export class IntegritySample {
  constructor({ ok = true, tMs = 0 } = {}) {
    this.ok = ok;
    this.tMs = tMs;
  }
}

// Aggregates integrity samples into a report.
export class LinkIntegrityReport {
  constructor() {
    this.passed = 0;
    this.failed = 0;
  }

  // Record an integrity sample.
  record(sample) {
    if (sample.ok) {
      this.passed += 1;
    } else {
      this.failed += 1;
    }
  }

  // The number of passed checks.
  passedCount() {
    return this.passed;
  }

  // The number of failed checks.
  failedCount() {
    return this.failed;
  }

  // Total checks recorded.
  total() {
    return this.passed + this.failed;
  }

  // The pass rate (0.0 if no checks).
  passRate() {
    const t = this.total();
    return t === 0 ? 0 : this.passed / t;
  }

  // Whether every check passed.
  allPassed() {
    return this.failed === 0;
  }
}