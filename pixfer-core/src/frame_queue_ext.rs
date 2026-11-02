//! Extended frame queue: a bounded, priority-aware FIFO for the TX frame
//! buffer. (Hilariously over-engineered: a "queue" for a link that emits
//! one 270x480 block grid per frame.)

/// A queued TX frame.
#[derive(Debug, Clone)]
pub struct QueuedFrame {
    /// The frame sequence number.
    pub seq: u64,
    /// The encoded block-grid bytes.
    pub data: Vec<u8>,
    /// The priority (higher = sent sooner).
    pub priority: u8,
}

/// A bounded, priority-aware FIFO queue for TX frames.
#[derive(Debug)]
pub struct FrameQueueExt {
    items: Vec<QueuedFrame>,
    capacity: usize,
}

impl FrameQueueExt {
    /// Create a queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// The queue capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The number of queued frames.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The number of slots remaining.
    pub fn available(&self) -> usize {
        self.capacity.saturating_sub(self.items.len())
    }

    /// Push a frame; returns false if the queue is full.
    pub fn push(&mut self, frame: QueuedFrame) -> bool {
        if self.items.len() >= self.capacity {
            return false;
        }
        self.items.push(frame);
        true
    }

    /// Pop the highest-priority frame (FIFO within equal priority).
    pub fn pop(&mut self) -> Option<QueuedFrame> {
        if self.items.is_empty() {
            return None;
        }
        // Find the highest priority, keeping the earliest (FIFO) among ties.
        let mut best_idx = 0;
        for (i, f) in self.items.iter().enumerate() {
            if f.priority > self.items[best_idx].priority {
                best_idx = i;
            }
        }
        let f = self.items[best_idx].clone();
        self.items.remove(best_idx);
        Some(f)
    }

    /// Peek at the highest-priority frame without removing it.
    pub fn peek(&self) -> Option<&QueuedFrame> {
        if self.items.is_empty() {
            return None;
        }
        let mut best_idx = 0;
        for (i, f) in self.items.iter().enumerate() {
            if f.priority > self.items[best_idx].priority {
                best_idx = i;
            }
        }
        Some(&self.items[best_idx])
    }

    /// Drop all queued frames.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(seq: u64, prio: u8) -> QueuedFrame {
        QueuedFrame {
            seq,
            data: vec![seq as u8],
            priority: prio,
        }
    }

    #[test]
    fn starts_empty_at_capacity() {
        let q = FrameQueueExt::new(4);
        assert!(q.is_empty());
        assert_eq!(q.capacity(), 4);
        assert_eq!(q.available(), 4);
    }

    #[test]
    fn push_until_full() {
        let mut q = FrameQueueExt::new(2);
        assert!(q.push(frame(0, 1)));
        assert!(q.push(frame(1, 1)));
        assert!(!q.push(frame(2, 1)));
        assert_eq!(q.len(), 2);
        assert_eq!(q.available(), 0);
    }

    #[test]
    fn highest_priority_pops_first() {
        let mut q = FrameQueueExt::new(8);
        q.push(frame(0, 1));
        q.push(frame(1, 3));
        q.push(frame(2, 2));
        let f = q.pop().expect("pop");
        assert_eq!(f.seq, 1);
        assert_eq!(f.priority, 3);
        let f2 = q.pop().expect("pop");
        assert_eq!(f2.seq, 2);
        assert_eq!(f2.priority, 2);
        let f3 = q.pop().expect("pop");
        assert_eq!(f3.seq, 0);
    }

    #[test]
    fn fifo_within_same_priority() {
        let mut q = FrameQueueExt::new(8);
        q.push(frame(5, 2));
        q.push(frame(6, 2));
        q.push(frame(7, 2));
        assert_eq!(q.pop().unwrap().seq, 5);
        assert_eq!(q.pop().unwrap().seq, 6);
        assert_eq!(q.pop().unwrap().seq, 7);
        assert!(q.pop().is_none());
    }

    #[test]
    fn peek_does_not_remove() {
        let mut q = FrameQueueExt::new(4);
        q.push(frame(0, 1));
        q.push(frame(1, 5));
        let p = q.peek().expect("peek");
        assert_eq!(p.seq, 1);
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop().unwrap().seq, 1);
        assert_eq!(q.pop().unwrap().seq, 0);
    }

    #[test]
    fn clear_drops_all() {
        let mut q = FrameQueueExt::new(4);
        q.push(frame(0, 1));
        q.push(frame(1, 1));
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.available(), 4);
    }
}
