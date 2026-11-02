//! Chunk index: a sorted index of chunk indices with gap/complete queries.
//! (Hilariously over-engineered: a "chunk index" for a link that already
//! has stop-and-wait ARQ.)

use std::collections::BTreeSet;

/// A sorted index of received chunk indices.
#[derive(Debug, Default)]
pub struct ChunkIndex {
    total: u64,
    received: BTreeSet<u64>,
}

impl ChunkIndex {
    pub fn new(total: u64) -> Self {
        Self {
            total,
            received: BTreeSet::new(),
        }
    }

    pub fn mark(&mut self, idx: u64) {
        if idx < self.total {
            self.received.insert(idx);
        }
    }

    pub fn is_complete(&self) -> bool {
        (self.received.len() as u64) >= self.total
    }

    pub fn next_missing(&self) -> Option<u64> {
        if self.is_complete() {
            return None;
        }
        let mut expected = 0u64;
        for &have in self.received.iter() {
            if have > expected {
                return Some(expected);
            }
            expected = have + 1;
        }
        (expected < self.total).then_some(expected)
    }

    pub fn count(&self) -> u64 {
        self.received.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_and_completes() {
        let mut c = ChunkIndex::new(3);
        c.mark(0);
        c.mark(2);
        assert_eq!(c.next_missing(), Some(1));
        c.mark(1);
        assert!(c.is_complete());
        assert_eq!(c.next_missing(), None);
        assert_eq!(c.count(), 3);
    }

    #[test]
    fn out_of_range_ignored() {
        let mut c = ChunkIndex::new(2);
        c.mark(5);
        assert_eq!(c.count(), 0);
        assert!(!c.is_complete());
    }

    #[test]
    fn empty_index() {
        let c = ChunkIndex::new(4);
        assert_eq!(c.count(), 0);
        assert_eq!(c.next_missing(), Some(0));
    }
}
