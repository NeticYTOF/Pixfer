//! Retry and backoff policies for the stop-and-wait ARQ loop: fixed,
//! linear, and exponential backoff strategies behind a `RetryPolicy` trait,
//! plus a `RetryBudget` guard. (Hilariously over-engineered: a full backoff
//! framework for a light link.)

use crate::config::{Config, ConfigProfile};

/// A retry/backoff strategy.
pub trait RetryPolicy: Send {
    /// The delay before attempt `attempt` (1-based).
    fn delay_ms(&self, attempt: u32) -> u64;
    fn name(&self) -> &'static str;
}

/// Constant delay between retries.
pub struct FixedDelayPolicy {
    pub base_ms: u64,
}

impl RetryPolicy for FixedDelayPolicy {
    fn delay_ms(&self, _attempt: u32) -> u64 {
        self.base_ms
    }
    fn name(&self) -> &'static str {
        "fixed"
    }
}

/// Linearly growing delay.
pub struct LinearBackoffPolicy {
    pub base_ms: u64,
}

impl RetryPolicy for LinearBackoffPolicy {
    fn delay_ms(&self, attempt: u32) -> u64 {
        self.base_ms * (attempt as u64)
    }
    fn name(&self) -> &'static str {
        "linear"
    }
}

/// Exponential backoff with a cap.
pub struct ExponentialBackoffPolicy {
    pub base_ms: u64,
    pub cap_ms: u64,
}

impl RetryPolicy for ExponentialBackoffPolicy {
    fn delay_ms(&self, attempt: u32) -> u64 {
        let shift = (attempt - 1).min(20);
        let exp = 1u64 << shift;
        (self.base_ms * exp).min(self.cap_ms)
    }
    fn name(&self) -> &'static str {
        "exponential"
    }
}

/// Choose a policy from the config profile (the "it depends on your profile"
/// flavor of over-engineering).
pub fn pick_policy(config: &Config) -> Box<dyn RetryPolicy> {
    match config.profile {
        ConfigProfile::LowPower => Box::new(ExponentialBackoffPolicy {
            base_ms: 100,
            cap_ms: config.ack_timeout_ms,
        }),
        ConfigProfile::Balanced => Box::new(LinearBackoffPolicy { base_ms: 200 }),
        ConfigProfile::MaxThroughput => Box::new(FixedDelayPolicy {
            base_ms: (config.ack_timeout_ms / 4).max(1),
        }),
    }
}

/// A hard cap on retry attempts for one chunk.
#[derive(Debug, Clone)]
pub struct RetryBudget {
    pub max_retries: u32,
    pub attempts: u32,
}

impl RetryBudget {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            attempts: 0,
        }
    }

    pub fn can_retry(&self) -> bool {
        self.attempts < self.max_retries
    }

    pub fn record_attempt(&mut self) {
        if self.attempts < u32::MAX {
            self.attempts += 1;
        }
    }

    pub fn exhausted(&self) -> bool {
        self.attempts >= self.max_retries
    }

    pub fn remaining(&self) -> u32 {
        self.max_retries.saturating_sub(self.attempts)
    }
}

/// A retry loop driver: given a policy + budget, it computes the schedule
/// of delays for a full retry sequence.
pub struct RetryScheduler {
    policy: Box<dyn RetryPolicy>,
    budget: RetryBudget,
}

impl RetryScheduler {
    pub fn new(policy: Box<dyn RetryPolicy>, config: &Config) -> Self {
        Self {
            policy,
            budget: RetryBudget::new(config.max_retries),
        }
    }

    /// The full delay schedule (ms) for up to `budget.max_retries` attempts.
    pub fn schedule(&self) -> Vec<u64> {
        let mut out = Vec::with_capacity(self.budget.max_retries as usize);
        for attempt in 1..=self.budget.max_retries {
            out.push(self.policy.delay_ms(attempt));
        }
        out
    }

    pub fn total_wait_ms(&self) -> u64 {
        self.schedule().iter().sum()
    }

    pub fn budget(&self) -> &RetryBudget {
        &self.budget
    }

    pub fn policy_name(&self) -> &'static str {
        self.policy.name()
    }

    /// One retry cycle: record the attempt and return the next delay.
    pub fn next_delay(&mut self) -> Option<u64> {
        if !self.budget.can_retry() {
            return None;
        }
        self.budget.record_attempt();
        Some(self.policy.delay_ms(self.budget.attempts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_policy_is_constant() {
        let p = FixedDelayPolicy { base_ms: 250 };
        assert_eq!(p.delay_ms(1), 250);
        assert_eq!(p.delay_ms(5), 250);
        assert_eq!(p.name(), "fixed");
    }

    #[test]
    fn linear_policy_grows() {
        let p = LinearBackoffPolicy { base_ms: 100 };
        assert_eq!(p.delay_ms(1), 100);
        assert_eq!(p.delay_ms(3), 300);
        assert_eq!(p.name(), "linear");
    }

    #[test]
    fn exponential_policy_caps() {
        let p = ExponentialBackoffPolicy {
            base_ms: 100,
            cap_ms: 400,
        };
        assert_eq!(p.delay_ms(1), 100);
        assert_eq!(p.delay_ms(2), 200);
        assert_eq!(p.delay_ms(3), 400);
        assert_eq!(p.delay_ms(10), 400); // capped
        assert_eq!(p.name(), "exponential");
    }

    #[test]
    fn pick_policy_follows_profile() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let bal = Config::for_profile(ConfigProfile::Balanced);
        let max = Config::for_profile(ConfigProfile::MaxThroughput);
        assert_eq!(pick_policy(&low).name(), "exponential");
        assert_eq!(pick_policy(&bal).name(), "linear");
        assert_eq!(pick_policy(&max).name(), "fixed");
    }

    #[test]
    fn budget_tracks_attempts() {
        let mut b = RetryBudget::new(3);
        assert!(b.can_retry());
        assert!(!b.exhausted());
        b.record_attempt();
        b.record_attempt();
        b.record_attempt();
        assert!(b.exhausted());
        assert_eq!(b.remaining(), 0);
        assert_eq!(b.attempts, 3);
    }

    #[test]
    fn scheduler_emits_full_schedule() {
        let cfg = Config::for_profile(ConfigProfile::LowPower);
        let s = RetryScheduler::new(pick_policy(&cfg), &cfg);
        let sched = s.schedule();
        assert_eq!(sched.len(), cfg.max_retries as usize);
        assert_eq!(s.policy_name(), "exponential");
        // Total wait is the sum of the schedule.
        assert_eq!(s.total_wait_ms(), sched.iter().sum());
    }

    #[test]
    fn scheduler_next_delay_honors_budget() {
        let cfg = Config::for_profile(ConfigProfile::MaxThroughput);
        let mut s = RetryScheduler::new(pick_policy(&cfg), &cfg);
        let mut delays = Vec::new();
        while let Some(d) = s.next_delay() {
            delays.push(d);
        }
        assert_eq!(delays.len(), cfg.max_retries as usize);
        // MaxThroughput uses a fixed delay = ack_timeout/4
        assert!(delays.iter().all(|&d| d == cfg.ack_timeout_ms / 4));
    }

    #[test]
    fn budget_overflow_safe() {
        let mut b = RetryBudget::new(u32::MAX);
        b.record_attempt();
        assert_eq!(b.attempts, 1);
    }
}
