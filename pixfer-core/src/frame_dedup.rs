//! Frame dedup: avoid retransmitting frames the receiver already has,
//! keyed by frame sequence number. (Hilariously over-engineered: a
//! "frame dedup cache" for a link that sends the same 270x480 grid over
//! and over.)

use std::collections::BTreeSet;

/// Tracks which frames have already been delivered, so the sender can skip
/// retransmitting them.
#[derive(Debug, Default)]
pub struct FrameDedup {
    delivered: BTreeSet<u64>,
}

impl FrameDedup {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that frame `seq` was delivered.
    pub fn mark_delivered(&mut self, seq: u64) {
        self.delivered.insert(seq);
    }

    /// Whether frame `seq` was already delivered.
    pub fn is_delivered(&self, seq: u64) -> bool {
        self.delivered.contains(&seq)
    }

    /// The number of unique frames delivered.
    pub fn len(&self) -> usize {
        self.delivered.len()
    }

    pub fn is_empty(&self) -> bool {
        self.delivered.is_empty()
    }

    /// The smallest delivered frame seq (None if empty).
    pub fn min_seq(&self) -> Option<u64> {
        self.delivered.iter().next().copied()
    }

    /// The largest delivered frame seq (None if empty).
    pub fn max_seq(&self) -> Option<u64> {
        self.delivered.iter().next_back().copied()
    }

    /// Clear the dedup set.
    pub fn clear(&mut self) {
        self.delivered.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty() {
        let d = FrameDedup::new();
        assert!(d.is_empty());
        assert_eq!(d.len(), 0);
        assert!(!d.is_delivered(0));
    }

    #[test]
    fn mark_and_query() {
        let mut d = FrameDedup::new();
        d.mark_delivered(0);
        d.mark_delivered(1);
        assert!(d.is_delivered(0));
        assert!(d.is_delivered(1));
        assert!(!d.is_delivered(2));
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn duplicate_marks_are_idempotent() {
        let mut d = FrameDedup::new();
        d.mark_delivered(5);
        d.mark_delivered(5);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn min_max_seq() {
        let mut d = FrameDedup::new();
        d.mark_delivered(3);
        d.mark_delivered(9);
        d.mark_delivered(1);
        assert_eq!(d.min_seq(), Some(1));
        assert_eq!(d.max_seq(), Some(9));
    }

    #[test]
    fn clear_resets_state() {
        let mut d = FrameDedup::new();
        d.mark_delivered(1);
        d.mark_delivered(2);
        d.clear();
        assert!(d.is_empty());
        assert!(!d.is_delivered(1));
    }
}
