//! Link integrity report: aggregate CRC/RS check results across a transfer
//! into a single health report. (Hilariously over-engineered: an "integrity
//! dashboard" for a link whose entire payload is 223 bytes at a time.)

/// A single integrity-check outcome.
#[derive(Debug, Clone, Copy)]
pub struct IntegritySample {
    /// Whether the check passed.
    pub ok: bool,
    pub t_ms: u64,
}

/// Aggregates integrity samples into a report.
#[derive(Debug, Default)]
pub struct LinkIntegrityReport {
    passed: u32,
    failed: u32,
}

impl LinkIntegrityReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an integrity sample.
    pub fn record(&mut self, sample: IntegritySample) {
        if sample.ok {
            self.passed = self.passed.saturating_add(1);
        } else {
            self.failed = self.failed.saturating_add(1);
        }
    }

    /// The number of passed checks.
    pub fn passed(&self) -> u32 {
        self.passed
    }

    /// The number of failed checks.
    pub fn failed(&self) -> u32 {
        self.failed
    }

    /// Total checks recorded.
    pub fn total(&self) -> u32 {
        self.passed + self.failed
    }

    /// The pass rate (0.0 if no checks).
    pub fn pass_rate(&self) -> f64 {
        if self.total() == 0 {
            0.0
        } else {
            self.passed as f64 / self.total() as f64
        }
    }

    /// Whether every check passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ok: bool) -> IntegritySample {
        IntegritySample { ok, t_ms: 0 }
    }

    #[test]
    fn empty_report_zero_rate() {
        let r = LinkIntegrityReport::new();
        assert_eq!(r.total(), 0);
        assert_eq!(r.pass_rate(), 0.0);
        assert!(r.all_passed());
    }

    #[test]
    fn record_and_counts() {
        let mut r = LinkIntegrityReport::new();
        r.record(sample(true));
        r.record(sample(true));
        r.record(sample(false));
        assert_eq!(r.passed(), 2);
        assert_eq!(r.failed(), 1);
        assert_eq!(r.total(), 3);
        assert!(!r.all_passed());
    }

    #[test]
    fn pass_rate_math() {
        let mut r = LinkIntegrityReport::new();
        for _ in 0..9 {
            r.record(sample(true));
        }
        r.record(sample(false));
        assert!((r.pass_rate() - 0.9).abs() < 1e-9);
        assert_eq!(r.failed(), 1);
    }

    #[test]
    fn all_passed_when_no_failures() {
        let mut r = LinkIntegrityReport::new();
        r.record(sample(true));
        r.record(sample(true));
        assert!(r.all_passed());
    }
}
