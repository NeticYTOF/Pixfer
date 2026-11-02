//! Chunk reordering: buffer out-of-order chunks and emit them in
//! sequential order. (Hilariously over-engineered: a "reorder buffer"
//! for a link that ships a few KB/s.)

/// A reordering buffer for chunk indices.
///
/// Chunks may arrive out of order (light links drop and reorder).
/// This buffer holds received chunk indices and only emits a chunk once
/// all lower indices have been received.
#[derive(Debug, Default)]
pub struct ChunkReorder {
    /// Total expected chunks.
    pub total: u32,
    /// Received chunk indices (unsorted).
    received: Vec<u32>,
    /// Chunks emitted in order so far.
    emitted: Vec<u32>,
}

impl ChunkReorder {
    pub fn new(total: u32) -> Self {
        Self {
            total,
            received: Vec::new(),
            emitted: Vec::new(),
        }
    }

    /// Record a received chunk index.
    pub fn receive(&mut self, idx: u32) {
        if idx < self.total && !self.received.contains(&idx) {
            self.received.push(idx);
        }
    }

    /// Emit all chunks that can now be sent in order (0, 1, 2, ...).
    /// Returns the list of newly-emitted chunk indices.
    pub fn drain_in_order(&mut self) -> Vec<u32> {
        let mut out = Vec::new();
        // Emit the longest in-order prefix of 0..total that is fully received.
        let mut next_idx = self.emitted.len() as u32;
        while next_idx < self.total && self.received.contains(&next_idx) {
            out.push(next_idx);
            self.emitted.push(next_idx);
            self.received.retain(|i| *i != next_idx);
            next_idx += 1;
        }
        out
    }

    /// Number of chunks received so far.
    pub fn received_count(&self) -> u32 {
        self.received.len() as u32
    }

    /// Number of chunks emitted in order.
    pub fn emitted_count(&self) -> u32 {
        self.emitted.len() as u32
    }

    /// Whether all chunks have been emitted in order.
    pub fn complete(&self) -> bool {
        self.emitted.len() as u32 >= self.total && self.total > 0
    }

    /// The first missing in-order index, or None if complete.
    pub fn next_missing(&self) -> Option<u32> {
        if self.complete() {
            return None;
        }
        let emitted = self.emitted.len() as u32;
        if emitted < self.total {
            Some(emitted)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_order_receives() {
        let mut r = ChunkReorder::new(4);
        r.receive(2);
        r.receive(0);
        r.receive(3);
        let emitted = r.drain_in_order();
        // 0 is emitted first; 1 is still missing.
        assert_eq!(emitted, vec![0]);
        assert_eq!(r.next_missing(), Some(1));
        r.receive(1);
        let emitted2 = r.drain_in_order();
        // Now 1, 2, 3 can all be emitted (2 and 3 were received out of order).
        assert_eq!(emitted2, vec![1, 2, 3]);
        assert!(r.complete());
        assert_eq!(r.next_missing(), None);
    }

    #[test]
    fn in_order_passes_through() {
        let mut r = ChunkReorder::new(3);
        r.receive(0);
        assert_eq!(r.drain_in_order(), vec![0]);
        r.receive(1);
        assert_eq!(r.drain_in_order(), vec![1]);
        r.receive(2);
        assert_eq!(r.drain_in_order(), vec![2]);
        assert!(r.complete());
    }

    #[test]
    fn duplicate_receive_ignored() {
        let mut r = ChunkReorder::new(3);
        r.receive(0);
        r.receive(0); // duplicate
        assert_eq!(r.received_count(), 1);
    }

    #[test]
    fn out_of_range_ignored() {
        let mut r = ChunkReorder::new(3);
        r.receive(5);
        assert_eq!(r.received_count(), 0);
    }

    #[test]
    fn emitted_count_tracks() {
        let mut r = ChunkReorder::new(4);
        r.receive(0);
        r.receive(1);
        r.drain_in_order();
        assert_eq!(r.emitted_count(), 2);
        // After draining, 0 and 1 are moved from `received` into `emitted`.
        assert_eq!(r.received_count(), 0);
    }

    #[test]
    fn total_zero_never_complete() {
        let r = ChunkReorder::new(0);
        assert!(!r.complete());
        assert_eq!(r.next_missing(), None);
    }
}
