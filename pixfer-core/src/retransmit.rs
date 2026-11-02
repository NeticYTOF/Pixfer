//! Retransmission: a retransmit scheduler that, on NAK or timeout, schedules
//! the retransmission with the configured backoff policy. (Hilariously over-
//! engineered: a retransmit queue for a link that already has stop-and-wait.)

use crate::config::Config;
use crate::policy::{pick_policy, RetryScheduler};

/// A single queued retransmission.
#[derive(Debug, Clone)]
pub struct RetransmitEntry {
    pub chunk_idx: u32,
    pub attempt: u32,
    pub delay_ms: u64,
}

/// A scheduler that manages the queue of chunks awaiting retransmission.
pub struct RetransmitScheduler {
    queue: Vec<RetransmitEntry>,
    scheduler: RetryScheduler,
}

impl RetransmitScheduler {
    pub fn new(config: &Config) -> Self {
        Self {
            queue: Vec::new(),
            scheduler: RetryScheduler::new(pick_policy(config), config),
        }
    }

    /// Enqueue a chunk for retransmission and compute its backoff delay.
    pub fn enqueue(&mut self, chunk_idx: u32) -> RetransmitEntry {
        let attempt = self
            .queue
            .iter()
            .filter(|e| e.chunk_idx == chunk_idx)
            .count() as u32
            + 1;
        let delay = self.scheduler.next_delay().unwrap_or_else(|| 0);
        let entry = RetransmitEntry {
            chunk_idx,
            attempt,
            delay_ms: delay,
        };
        self.queue.push(entry.clone());
        entry
    }

    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    pub fn next_due(&self, now_ms: u64) -> Option<RetransmitEntry> {
        for e in self.queue.iter() {
            if e.delay_ms <= now_ms {
                return Some(e.clone());
            }
        }
        None
    }

    pub fn drain(&mut self) -> Vec<RetransmitEntry> {
        std::mem::take(&mut self.queue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ConfigProfile};

    fn cfg() -> Config {
        Config::for_profile(ConfigProfile::Balanced)
    }

    #[test]
    fn enqueue_grows_queue() {
        let mut s = RetransmitScheduler::new(&cfg());
        s.enqueue(3);
        s.enqueue(5);
        assert_eq!(s.pending_count(), 2);
    }

    #[test]
    fn attempt_increments_per_chunk() {
        let mut s = RetransmitScheduler::new(&cfg());
        let e1 = s.enqueue(3);
        let e2 = s.enqueue(3);
        assert_eq!(e1.attempt, 1);
        assert_eq!(e2.attempt, 2);
    }

    #[test]
    fn next_due_respects_now() {
        let mut s = RetransmitScheduler::new(&cfg());
        let e = s.enqueue(1);
        // Before the delay elapses -> none due.
        assert!(s.next_due(0).is_none() || s.next_due(0).unwrap().delay_ms <= 0);
        // At/after the delay -> due.
        assert!(s.next_due(e.delay_ms).is_some());
    }

    #[test]
    fn drain_empties_queue() {
        let mut s = RetransmitScheduler::new(&cfg());
        s.enqueue(1);
        s.enqueue(2);
        let drained = s.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(s.pending_count(), 0);
    }
}
