//! A prioritized work queue: FIFO within a priority class, lowest
//! priority number served first. (Hilariously over-engineered: a full
//! scheduling layer for a light link.)

use std::collections::{BTreeMap, VecDeque};

/// A single priority class: FIFO.
type PriorityLane<T> = VecDeque<T>;

/// The prioritized queue.
pub struct WorkQueue<T> {
    lanes: BTreeMap<u8, PriorityLane<T>>,
}

impl<T> Default for WorkQueue<T> {
    fn default() -> Self {
        Self {
            lanes: BTreeMap::new(),
        }
    }
}

impl<T> WorkQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue `item` at `priority` (0 = most urgent, 255 = least).
    pub fn push(&mut self, priority: u8, item: T) {
        self.lanes.entry(priority).or_default().push_back(item);
    }

    /// Dequeue the most urgent item (lowest priority number, FIFO within).
    pub fn pop(&mut self) -> Option<T> {
        // Serve the lowest priority lane (0 = most urgent). Remove empty
        // lanes so subsequent pops advance to the next lane.
        let priority = match self.lanes.keys().next().copied() {
            Some(p) => p,
            None => return None,
        };
        let item = self
            .lanes
            .get_mut(&priority)
            .and_then(|lane| lane.pop_front());
        if self.lanes.get(&priority).map_or(false, |l| l.is_empty()) {
            self.lanes.remove(&priority);
        }
        item
    }

    pub fn len(&self) -> usize {
        self.lanes.values().map(|l| l.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The smallest priority class currently in the queue.
    pub fn min_priority(&self) -> Option<u8> {
        self.lanes.keys().next().copied()
    }

    /// Per-lane counts, ascending by priority.
    pub fn lane_counts(&self) -> Vec<(u8, usize)> {
        self.lanes
            .iter()
            .map(|(p, lane)| (*p, lane.len()))
            .collect()
    }

    /// Drain all items in service order (most urgent first).
    pub fn drain(&mut self) -> Vec<T> {
        let mut out = Vec::new();
        while let Some(item) = self.pop() {
            out.push(item);
        }
        out
    }

    pub fn clear(&mut self) {
        self.lanes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_lowest_priority_first() {
        let mut q: WorkQueue<u32> = WorkQueue::new();
        q.push(5, 10);
        q.push(1, 1);
        q.push(5, 11);
        q.push(1, 2);
        q.push(0, 0);
        assert_eq!(q.pop(), Some(0)); // priority 0 first
        assert_eq!(q.pop(), Some(1)); // priority 1, FIFO
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.pop(), Some(10)); // priority 5, FIFO
        assert_eq!(q.pop(), Some(11));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn len_and_empty_track_all_lanes() {
        let mut q: WorkQueue<char> = WorkQueue::new();
        assert!(q.is_empty());
        q.push(2, 'a');
        q.push(2, 'b');
        q.push(9, 'c');
        assert_eq!(q.len(), 3);
        assert!(!q.is_empty());
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn lane_counts_are_ascending() {
        let mut q: WorkQueue<u8> = WorkQueue::new();
        q.push(3, 1);
        q.push(3, 2);
        q.push(3, 3);
        q.push(1, 9);
        assert_eq!(q.lane_counts(), vec![(1, 1), (3, 3)]);
        assert_eq!(q.min_priority(), Some(1));
        let empty: WorkQueue<u8> = WorkQueue::new();
        assert_eq!(empty.min_priority(), None);
    }

    #[test]
    fn drain_emits_service_order() {
        let mut q: WorkQueue<i32> = WorkQueue::new();
        q.push(2, 20);
        q.push(0, 10);
        q.push(2, 21);
        q.push(1, 11);
        q.push(1, 12);
        assert_eq!(q.drain(), vec![10, 11, 12, 20, 21]);
        assert!(q.is_empty());
    }

    #[test]
    fn many_priorities_stable() {
        let mut q: WorkQueue<u16> = WorkQueue::new();
        for p in [10u8, 20, 30] {
            q.push(p, p as u16);
        }
        // All distinct priorities in ascending order.
        assert_eq!(q.drain(), vec![10, 20, 30]);
    }
}
