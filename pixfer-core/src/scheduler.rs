//! Transfer job scheduling: a `TransferScheduler` with pluggable
//! `SchedulerPolicy` strategies (FIFO, round-robin, priority) for
//! queueing and running transfer jobs. (Hilariously over-engineered:
//! a full scheduler for a light link.)

use std::collections::VecDeque;

/// A transfer job to be scheduled.
#[derive(Debug, Clone)]
pub struct TransferJob {
    pub id: u32,
    pub payload_bytes: u64,
    /// 0 = most urgent.
    pub priority: u8,
    pub session_id: u16,
}

/// A scheduling strategy.
pub trait SchedulerPolicy: Send {
    /// Choose the index of the next job in `jobs` given the scheduler's
    /// rotation `cursor`.
    fn pick(&self, jobs: &[TransferJob], cursor: usize) -> Option<usize>;
    fn name(&self) -> &'static str;
}

/// Serve jobs in queue order.
pub struct FifoPolicy;

impl SchedulerPolicy for FifoPolicy {
    fn pick(&self, _jobs: &[TransferJob], _cursor: usize) -> Option<usize> {
        Some(0)
    }
    fn name(&self) -> &'static str {
        "fifo"
    }
}

/// Rotate through the queue; the scheduler's cursor advances after each pick.
pub struct RoundRobinPolicy;

impl SchedulerPolicy for RoundRobinPolicy {
    fn pick(&self, jobs: &[TransferJob], cursor: usize) -> Option<usize> {
        if jobs.is_empty() {
            return None;
        }
        Some(cursor % jobs.len())
    }
    fn name(&self) -> &'static str {
        "round_robin"
    }
}

/// Serve the most urgent job (lowest `priority` value) first, FIFO on ties.
pub struct PriorityPolicy;

impl SchedulerPolicy for PriorityPolicy {
    fn pick(&self, jobs: &[TransferJob], _cursor: usize) -> Option<usize> {
        jobs.iter()
            .enumerate()
            .min_by_key(|(_, j)| (j.priority, 0))
            .map(|(i, _)| i)
    }
    fn name(&self) -> &'static str {
        "priority"
    }
}

/// The scheduler: a job queue + policy + rotation cursor.
#[derive(Default)]
pub struct TransferScheduler {
    queue: VecDeque<TransferJob>,
    policy: Option<Box<dyn SchedulerPolicy>>,
    cursor: usize,
    running: Option<TransferJob>,
    completed: u64,
}

impl TransferScheduler {
    pub fn with_policy(policy: Box<dyn SchedulerPolicy>) -> Self {
        Self {
            queue: VecDeque::new(),
            policy: Some(policy),
            cursor: 0,
            running: None,
            completed: 0,
        }
    }

    pub fn policy_name(&self) -> &'static str {
        match &self.policy {
            Some(p) => p.name(),
            None => "none",
        }
    }

    pub fn enqueue(&mut self, job: TransferJob) {
        self.queue.push_back(job);
    }

    /// Pick the next job per the policy, remove it from the queue, and
    /// mark it running.
    pub fn next(&mut self) -> Option<&TransferJob> {
        if self.queue.is_empty() {
            return None;
        }
        let jobs = self.queue.make_contiguous();
        let idx = self.policy.as_ref().and_then(|p| p.pick(jobs, self.cursor));
        let i = match idx {
            Some(i) => i,
            None => return None,
        };
        // Remove exactly the picked job, keeping all others in order.
        let mut items: Vec<TransferJob> = self.queue.iter().cloned().collect();
        let picked = items.remove(i);
        self.queue = items.iter().cloned().collect();
        self.running = Some(picked);
        // The next turn starts at the new front of the queue.
        self.cursor = 0;
        self.running.as_ref()
    }

    /// Mark the running job complete and free the slot.
    pub fn complete_current(&mut self) {
        if self.running.is_some() {
            self.completed += 1;
            self.running = None;
        }
    }

    /// Cancel a queued job by id; returns true if it was found.
    pub fn cancel(&mut self, id: u32) -> bool {
        let before = self.queue.len();
        self.queue.retain(|j| j.id != id);
        self.queue.len() < before
    }

    /// Cancel the running job.
    pub fn cancel_running(&mut self) -> bool {
        self.running = None;
        true
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn completed_count(&self) -> u64 {
        self.completed
    }

    /// JSON-ish report of the queue state.
    pub fn report(&self) -> String {
        let items: Vec<String> = self
            .queue
            .iter()
            .map(|j| {
                format!(
                    "{{\"id\": {}, \"bytes\": {}, \"priority\": {}, \"session_id\": {}}}",
                    j.id, j.payload_bytes, j.priority, j.session_id
                )
            })
            .collect();
        format!(
            "{{\"policy\": \"{}\", \"queue_len\": {}, \"completed\": {}, \"jobs\": [{}]}}",
            self.policy_name(),
            self.len(),
            self.completed,
            items.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: u32, bytes: u64, priority: u8) -> TransferJob {
        TransferJob {
            id,
            payload_bytes: bytes,
            priority,
            session_id: id as u16,
        }
    }

    #[test]
    fn fifo_serves_in_order() {
        let mut s = TransferScheduler::with_policy(Box::new(FifoPolicy));
        s.enqueue(job(1, 100, 5));
        s.enqueue(job(2, 200, 1));
        s.enqueue(job(3, 300, 9));
        assert_eq!(s.next().unwrap().id, 1);
        s.complete_current();
        assert_eq!(s.next().unwrap().id, 2);
        s.complete_current();
        assert_eq!(s.next().unwrap().id, 3);
        s.complete_current();
        assert!(s.next().is_none());
        assert_eq!(s.completed_count(), 3);
    }

    #[test]
    fn round_robin_rotates() {
        let mut s = TransferScheduler::with_policy(Box::new(RoundRobinPolicy));
        s.enqueue(job(1, 10, 1));
        s.enqueue(job(2, 20, 2));
        s.enqueue(job(3, 30, 3));
        assert_eq!(s.next().unwrap().id, 1); // cursor 0 -> job 1
        s.complete_current();
        assert_eq!(s.next().unwrap().id, 2); // cursor 1 -> job 2
        s.complete_current();
        assert_eq!(s.next().unwrap().id, 3);
        s.complete_current();
        assert!(s.next().is_none());
        assert_eq!(s.policy_name(), "round_robin");
    }

    #[test]
    fn priority_serves_most_urgent_first() {
        let mut s = TransferScheduler::with_policy(Box::new(PriorityPolicy));
        s.enqueue(job(1, 100, 5));
        s.enqueue(job(2, 200, 0)); // most urgent
        s.enqueue(job(3, 300, 2));
        assert_eq!(s.next().unwrap().id, 2); // priority 0
        s.complete_current();
        assert_eq!(s.next().unwrap().id, 3); // priority 2
        s.complete_current();
        assert_eq!(s.next().unwrap().id, 1); // priority 5
        s.complete_current();
        assert_eq!(s.policy_name(), "priority");
    }

    #[test]
    fn cancel_removes_queued_job() {
        let mut s = TransferScheduler::with_policy(Box::new(FifoPolicy));
        s.enqueue(job(1, 1, 1));
        s.enqueue(job(2, 2, 2));
        s.enqueue(job(3, 3, 3));
        assert!(s.cancel(2));
        assert!(!s.cancel(99));
        assert_eq!(s.len(), 2);
        assert_eq!(s.next().unwrap().id, 1);
        s.complete_current();
        assert_eq!(s.next().unwrap().id, 3);
    }

    #[test]
    fn report_shape() {
        let mut s = TransferScheduler::with_policy(Box::new(FifoPolicy));
        s.enqueue(job(7, 123, 2));
        let r = s.report();
        assert!(r.contains("\"policy\": \"fifo\""));
        assert!(r.contains("\"queue_len\": 1"));
        assert!(r.contains("\"id\": 7"));
        assert!(r.contains("\"bytes\": 123"));
    }

    #[test]
    fn empty_scheduler_returns_none() {
        let mut s = TransferScheduler::with_policy(Box::new(FifoPolicy));
        assert!(s.next().is_none());
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }
}
