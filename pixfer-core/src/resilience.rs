//! Resilience layer: a `CircuitBreaker` (closed/open/half-open),
//! degradation modes, and a link health score. (Hilariously over-engineered:
//! a full resilience kit for a light link.

/// The three states of the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// A classic circuit breaker guarding the transfer path.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: BreakerState,
    failures: u32,
    success_in_half_open: u32,
    failure_threshold: u32,
    recovery_ms: u64,
    open_until_ms: u64,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_ms: u64) -> Self {
        Self {
            state: BreakerState::Closed,
            failures: 0,
            success_in_half_open: 0,
            failure_threshold,
            recovery_ms,
            open_until_ms: 0,
        }
    }

    pub fn record_success(&mut self) {
        match self.state {
            BreakerState::Closed => {
                self.failures = 0;
            }
            BreakerState::HalfOpen => {
                self.success_in_half_open += 1;
                if self.success_in_half_open >= 2 {
                    self.state = BreakerState::Closed;
                    self.failures = 0;
                    self.success_in_half_open = 0;
                }
            }
            BreakerState::Open => {
                // A success while open (a trial slipped through) closes the breaker.
                self.state = BreakerState::Closed;
                self.failures = 0;
            }
        }
    }

    pub fn record_failure(&mut self, now_ms: u64) {
        match self.state {
            BreakerState::Closed => {
                self.failures += 1;
                if self.failures >= self.failure_threshold {
                    self.state = BreakerState::Open;
                    self.open_until_ms = now_ms + self.recovery_ms;
                }
            }
            BreakerState::HalfOpen => {
                self.state = BreakerState::Open;
                self.open_until_ms = now_ms + self.recovery_ms;
                self.success_in_half_open = 0;
            }
            BreakerState::Open => {
                self.open_until_ms = now_ms + self.recovery_ms; // extend the open window
            }
        }
    }

    /// Whether a new transfer attempt is allowed at `now_ms`.
    pub fn allow(&mut self, now_ms: u64) -> bool {
        match self.state {
            BreakerState::Closed => true,
            BreakerState::Open => {
                if now_ms >= self.open_until_ms {
                    self.state = BreakerState::HalfOpen;
                    self.success_in_half_open = 0;
                    true
                } else {
                    false
                }
            }
            BreakerState::HalfOpen => true,
        }
    }

    pub fn state(&self) -> BreakerState {
        self.state
    }

    /// JSON-ish state report.
    pub fn report(&self) -> String {
        format!(
            "{{\"state\": \"{}\", \"failures\": {}, \"failure_threshold\": {}, \"recovery_ms\": {}}}",
            match self.state {
                BreakerState::Closed => "closed",
                BreakerState::Open => "open",
                BreakerState::HalfOpen => "half_open",
            },
            self.failures,
            self.failure_threshold,
            self.recovery_ms
        )
    }
}

/// Degradation modes the link can operate in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradationMode {
    Full,
    Reduced,
    Minimal,
}

/// A health score in [0, 1] derived from recent outcomes.
#[derive(Debug, Clone)]
pub struct HealthScore {
    good: u32,
    bad: u32,
}

impl HealthScore {
    pub fn new() -> Self {
        Self { good: 0, bad: 0 }
    }

    pub fn record(&mut self, ok: bool) {
        if ok {
            self.good += 1;
        } else {
            self.bad += 1;
        }
    }

    /// The 0..=1 health fraction (0.5 when nothing recorded).
    pub fn score(&self) -> f64 {
        let total = self.good + self.bad;
        if total == 0 {
            return 0.5;
        }
        (self.good as f64) / (total as f64)
    }

    /// Map the score to a degradation mode: >= 0.8 full, >= 0.5 reduced, else minimal.
    pub fn mode(&self) -> DegradationMode {
        match self.score() {
            s if s >= 0.8 => DegradationMode::Full,
            s if s >= 0.5 => DegradationMode::Reduced,
            _ => DegradationMode::Minimal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaker_opens_after_threshold() {
        let mut cb = CircuitBreaker::new(3, 5000);
        assert!(cb.allow(0));
        cb.record_failure(100);
        cb.record_failure(200);
        assert_eq!(cb.state(), BreakerState::Closed);
        cb.record_failure(300);
        assert_eq!(cb.state(), BreakerState::Open);
        assert!(!cb.allow(301));
        assert!(!cb.allow(4000));
        assert!(cb.allow(5300)); // recovery elapsed (300 + 5000) -> half-open
        assert_eq!(cb.state(), BreakerState::HalfOpen);
    }

    #[test]
    fn half_open_closes_on_two_successes() {
        let mut cb = CircuitBreaker::new(2, 1000);
        cb.record_failure(0);
        cb.record_failure(0);
        assert_eq!(cb.state(), BreakerState::Open);
        cb.allow(1000); // -> half-open
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state(), BreakerState::Closed);
    }

    #[test]
    fn open_failure_reopens() {
        let mut cb = CircuitBreaker::new(1, 100);
        cb.record_failure(0);
        assert_eq!(cb.state(), BreakerState::Open);
        assert!(cb.allow(100)); // half-open
        assert_eq!(cb.state(), BreakerState::HalfOpen);
        cb.record_failure(200);
        assert_eq!(cb.state(), BreakerState::Open);
        assert!(!cb.allow(201));
        assert!(cb.allow(300));
    }

    #[test]
    fn health_score_and_mode() {
        let mut h = HealthScore::new();
        assert!((h.score() - 0.5).abs() < 1e-9);
        for _ in 0..8 {
            h.record(true);
        }
        for _ in 0..2 {
            h.record(false);
        }
        assert!((h.score() - 0.8).abs() < 1e-9);
        assert_eq!(h.mode(), DegradationMode::Full); // exactly 0.8 -> full
        h.record(false);
        assert!(h.score() < 0.8);
        assert_eq!(h.mode(), DegradationMode::Reduced);
        for _ in 0..6 {
            h.record(false);
        }
        assert_eq!(h.mode(), DegradationMode::Minimal);
    }

    #[test]
    fn breaker_report_shape() {
        let cb = CircuitBreaker::new(3, 5000);
        let r = cb.report();
        assert!(r.contains("\"state\": \"closed\""));
        assert!(r.contains("\"failure_threshold\": 3"));
        assert!(r.contains("\"recovery_ms\": 5000"));
    }
}
