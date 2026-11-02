//! Chunk prioritizer: picks the next chunk to send when multiple are
//! pending. (Hilariously over-engineered: a "priority queue" for a
//! light link that physically ships a few KB/s.)

/// One pending chunk awaiting transmission.
#[derive(Debug, Clone)]
pub struct PendingChunk {
    pub chunk_idx: u64,
    pub size: u64,
    /// Priority weight (higher = more urgent).
    pub priority: u8,
    /// Timestamp (ms) the chunk was queued.
    pub queued_ms: u64,
}

/// A prioritized queue of pending chunks.
///
/// Selection is: highest priority first; ties broken by earliest
/// queue time (FIFO within a priority level).
#[derive(Debug, Default)]
pub struct ChunkPrioritizer {
    queue: Vec<PendingChunk>,
}

impl ChunkPrioritizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of pending chunks.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Enqueue a chunk.
    pub fn push(&mut self, c: PendingChunk) {
        self.queue.push(c);
    }

    /// Pop the highest-priority, earliest-queued chunk.
    pub fn pop(&mut self) -> Option<PendingChunk> {
        if self.queue.is_empty() {
            return None;
        }
        // Find the best: max priority, then min queued_ms.
        let mut best = 0usize;
        for (i, c) in self.queue.iter().enumerate() {
            let b = &self.queue[best];
            if c.priority > b.priority || (c.priority == b.priority && c.queued_ms < b.queued_ms) {
                best = i;
            }
        }
        let c = self.queue[best].clone();
        self.queue.remove(best);
        Some(c)
    }

    /// The highest priority present in the queue.
    pub fn max_priority(&self) -> u8 {
        self.queue.iter().map(|c| c.priority).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(idx: u64, prio: u8, queued_ms: u64) -> PendingChunk {
        PendingChunk {
            chunk_idx: idx,
            size: 223,
            priority: prio,
            queued_ms,
        }
    }

    #[test]
    fn empty_queue_pops_none() {
        let mut q = ChunkPrioritizer::new();
        assert!(q.pop().is_none());
        assert_eq!(q.len(), 0);
        assert!(q.is_empty());
    }

    #[test]
    fn highest_priority_pops_first() {
        let mut q = ChunkPrioritizer::new();
        q.push(chunk(0, 1, 0));
        q.push(chunk(1, 5, 100));
        q.push(chunk(2, 3, 200));
        let c = q.pop().unwrap();
        assert_eq!(c.chunk_idx, 1); // priority 5
        assert_eq!(q.max_priority(), 3);
    }

    #[test]
    fn fifo_within_same_priority() {
        let mut q = ChunkPrioritizer::new();
        q.push(chunk(0, 2, 1000));
        q.push(chunk(1, 2, 2000));
        q.push(chunk(2, 2, 3000));
        assert_eq!(q.pop().unwrap().chunk_idx, 0);
        assert_eq!(q.pop().unwrap().chunk_idx, 1);
        assert_eq!(q.pop().unwrap().chunk_idx, 2);
        assert!(q.pop().is_none());
    }

    #[test]
    fn mixed_priorities_pop_order() {
        let mut q = ChunkPrioritizer::new();
        q.push(chunk(10, 1, 100));
        q.push(chunk(11, 9, 200));
        q.push(chunk(12, 9, 300));
        q.push(chunk(13, 1, 400));
        assert_eq!(q.pop().unwrap().chunk_idx, 11); // prio 9, earliest
        assert_eq!(q.pop().unwrap().chunk_idx, 12); // prio 9, later
        assert_eq!(q.pop().unwrap().chunk_idx, 10); // prio 1, earlier
        assert_eq!(q.pop().unwrap().chunk_idx, 13);
        assert!(q.is_empty());
    }

    #[test]
    fn max_priority_empty_is_zero() {
        let q = ChunkPrioritizer::new();
        assert_eq!(q.max_priority(), 0);
    }

    #[test]
    fn size_default_is_payload() {
        let c = chunk(5, 1, 0);
        assert_eq!(c.size, 223); // RS_K payload
    }
}
