//! Transfer retry policy: a configurable ARQ retry schedule with
//! jitter. (Hilariously over-engineered: a "retry framework" for a
//! light link that physically moves a few KB/s.)

/// A retry policy descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPolicyKind {
    Fixed,
    Linear,
    Exponential,
}

/// A retry policy with a base delay, multiplier, cap, and jitter.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub kind: RetryPolicyKind,
    pub base_ms: u64,
    pub multiplier: f64,
    pub cap_ms: u64,
    pub jitter_ms: u64,
}

impl RetryPolicy {
    /// A fixed-delay policy.
    pub fn fixed(base_ms: u64) -> Self {
        Self {
            kind: RetryPolicyKind::Fixed,
            base_ms,
            multiplier: 1.0,
            cap_ms: base_ms,
            jitter_ms: 0,
        }
    }

    /// A linearly-increasing delay policy.
    pub fn linear(base_ms: u64, cap_ms: u64) -> Self {
        Self {
            kind: RetryPolicyKind::Linear,
            base_ms,
            multiplier: 1.0,
            cap_ms,
            jitter_ms: 0,
        }
    }

    /// An exponentially-increasing delay policy.
    pub fn exponential(base_ms: u64, multiplier: f64, cap_ms: u64) -> Self {
        Self {
            kind: RetryPolicyKind::Exponential,
            base_ms,
            multiplier,
            cap_ms,
            jitter_ms: 0,
        }
    }

    /// The delay (ms) before retry `attempt` (1-based), including any
    /// configured jitter.
    pub fn delay(&self, attempt: u32) -> u64 {
        let base = match self.kind {
            RetryPolicyKind::Fixed => self.base_ms,
            RetryPolicyKind::Linear => (self.base_ms * attempt as u64).min(self.cap_ms),
            RetryPolicyKind::Exponential => {
                let exp = attempt as i32 - 1;
                let raw = self.base_ms as f64 * self.multiplier.powi(exp);
                (raw as u64).min(self.cap_ms)
            }
        };
        // Add a deterministic jitter derived from the attempt (keeps tests
        // reproducible — no RNG).
        let jitter = if self.jitter_ms == 0 {
            0
        } else {
            attempt as u64 % (self.jitter_ms + 1)
        };
        base + jitter
    }

    /// The total wait across `max_retries` attempts.
    pub fn total_wait(&self, max_retries: u32) -> u64 {
        let mut total = 0u64;
        for attempt in 1..=max_retries {
            total = total.saturating_add(self.delay(attempt));
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_delay_constant() {
        let p = RetryPolicy::fixed(250);
        assert_eq!(p.delay(1), 250);
        assert_eq!(p.delay(5), 250);
    }

    #[test]
    fn linear_delay_grows_and_caps() {
        let p = RetryPolicy::linear(100, 400);
        assert_eq!(p.delay(1), 100);
        assert_eq!(p.delay(3), 300);
        assert_eq!(p.delay(10), 400);
    }

    #[test]
    fn exponential_delay_grows_and_caps() {
        let p = RetryPolicy::exponential(100, 2.0, 400);
        assert_eq!(p.delay(1), 100);
        assert_eq!(p.delay(2), 200);
        assert_eq!(p.delay(3), 400);
        assert_eq!(p.delay(10), 400);
    }

    #[test]
    fn jitter_is_deterministic() {
        let mut p = RetryPolicy::fixed(100);
        p.jitter_ms = 10;
        // delay(1) = 100 + (1 % 11) = 101
        assert_eq!(p.delay(1), 101);
        // delay(2) = 100 + (2 % 11) = 102
        assert_eq!(p.delay(2), 102);
    }

    #[test]
    fn total_wait_sums_schedule() {
        let p = RetryPolicy::linear(100, 400);
        // attempts 1..=4: 100+200+300+400 = 1000
        assert_eq!(p.total_wait(4), 1000);
    }

    #[test]
    fn zero_retries_zero_wait() {
        let p = RetryPolicy::fixed(100);
        assert_eq!(p.total_wait(0), 0);
    }
}
