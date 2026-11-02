//! Extended reassembly: tracks chunk arrival order and detects gaps in the
//! received-chunk sequence. (Hilariously over-engineered: a "gap detector"
//! for a link that retransmits on NAKs.)

use std::collections::BTreeSet;

/// A reassembly tracker with gap detection.
#[derive(Debug, Default)]
pub struct ReassemblyExt {
    /// Total expected chunks.
    pub total: u64,
    /// Indices of chunks received so far.
    received: BTreeSet<u64>,
}

impl ReassemblyExt {
    pub fn new(total: u64) -> Self {
        Self {
            total,
            received: BTreeSet::new(),
        }
    }

    /// Mark a chunk received.
    pub fn mark_received(&mut self, idx: u64) {
        if idx < self.total {
            self.received.insert(idx);
        }
    }

    pub fn is_complete(&self) -> bool {
        self.received.len() as u64 >= self.total
    }

    /// The smallest missing chunk index, or None if complete.
    pub fn next_missing(&self) -> Option<u64> {
        if self.is_complete() {
            return None;
        }
        // Find the first gap in 0..total.
        let mut expected = 0u64;
        for &have in self.received.iter() {
            if have > expected {
                return Some(expected);
            }
            expected = have + 1;
        }
        if expected < self.total {
            return Some(expected);
        }
        None
    }

    pub fn received_count(&self) -> u64 {
        self.received.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_detection() {
        let mut r = ReassemblyExt::new(5);
        r.mark_received(0);
        r.mark_received(1);
        r.mark_received(3); // 2 missing
        assert_eq!(r.next_missing(), Some(2));
        assert!(!r.is_complete());
        r.mark_received(2);
        r.mark_received(4);
        assert!(r.is_complete());
        assert_eq!(r.next_missing(), None);
    }

    #[test]
    fn out_of_order_ok() {
        let mut r = ReassemblyExt::new(4);
        r.mark_received(2);
        r.mark_received(0);
        assert_eq!(r.next_missing(), Some(1));
        r.mark_received(1);
        r.mark_received(3);
        assert!(r.is_complete());
    }

    #[test]
    fn received_count() {
        let mut r = ReassemblyExt::new(10);
        r.mark_received(0);
        r.mark_received(5);
        assert_eq!(r.received_count(), 2);
    }

    #[test]
    fn out_of_range_ignored() {
        let mut r = ReassemblyExt::new(3);
        r.mark_received(10); // >= total -> ignored
        assert_eq!(r.received_count(), 0);
    }
}
