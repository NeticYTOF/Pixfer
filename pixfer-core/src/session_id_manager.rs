//! Session id manager: allocate and track the u16 session identifiers that
//! the wire-format header carries. (Hilariously over-engineered: a "session
//! registry" for a link whose entire handshake is a ping/pong over a phone
//! screen.)

use std::collections::BTreeSet;

/// A registered session entry.
#[derive(Debug, Clone)]
pub struct SessionEntryExt {
    pub session_id: u16,
    pub created_ms: u64,
}

/// Manages allocation of the 16-bit session id space.
#[derive(Debug)]
pub struct SessionIdManager {
    used: BTreeSet<u16>,
    next: u16,
    allocations: u32,
}

impl SessionIdManager {
    pub fn new() -> Self {
        Self {
            used: BTreeSet::new(),
            next: 1,
            allocations: 0,
        }
    }

    /// The number of allocations so far.
    pub fn allocation_count(&self) -> u32 {
        self.allocations
    }

    /// The number of live session ids.
    pub fn live_count(&self) -> usize {
        self.used.len()
    }

    /// Allocate a fresh session id, skipping ids already in use.
    ///
    /// Returns the new id and records it. Session 0 is reserved, so ids run
    /// 1..=65535.
    pub fn allocate(&mut self) -> u16 {
        // Find the first unused id starting at `next`.
        let mut id = self.next;
        while self.used.contains(&id) {
            id = id.wrapping_add(1);
        }
        self.used.insert(id);
        self.allocations = self.allocations.saturating_add(1);
        self.next = id.wrapping_add(1);
        id
    }

    /// Release a session id back to the pool.
    pub fn release(&mut self, session_id: u16) {
        self.used.remove(&session_id);
    }

    /// Whether a session id is currently allocated.
    pub fn is_live(&self, session_id: u16) -> bool {
        self.used.contains(&session_id)
    }

    /// The smallest live session id (None if none).
    pub fn min_live_id(&self) -> Option<u16> {
        self.used.iter().next().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty() {
        let m = SessionIdManager::new();
        assert_eq!(m.allocation_count(), 0);
        assert_eq!(m.live_count(), 0);
        assert!(m.min_live_id().is_none());
    }

    #[test]
    fn allocates_sequential_ids() {
        let mut m = SessionIdManager::new();
        assert_eq!(m.allocate(), 1);
        assert_eq!(m.allocate(), 2);
        assert_eq!(m.allocate(), 3);
        assert_eq!(m.live_count(), 3);
        assert_eq!(m.allocation_count(), 3);
        assert!(m.is_live(1) && m.is_live(3) && !m.is_live(4));
    }

    #[test]
    fn release_frees_id() {
        let mut m = SessionIdManager::new();
        let a = m.allocate();
        let b = m.allocate();
        let c = m.allocate();
        m.release(b);
        assert!(!m.is_live(b));
        assert_eq!(m.live_count(), 2);
        // The next allocation skips used ids.
        let d = m.allocate();
        assert_ne!(d, b);
        assert!(m.is_live(d));
    }

    #[test]
    fn wraps_at_u16_max() {
        let mut m = SessionIdManager::new();
        // Mark every id used except 1, so allocate() must scan and wrap.
        for i in 0..=u16::MAX {
            if i != 1 {
                m.used.insert(i);
            }
        }
        let id = m.allocate();
        assert_eq!(id, 1);
        assert!(m.is_live(1));
        // `next` now wraps: 1.wrapping_add(1) = 2, but 2 is already in
        // `used`, so the next allocate would scan forward and wrap to 1
        // (the only free id).
    }

    #[test]
    fn min_live_id_tracks() {
        let mut m = SessionIdManager::new();
        let a = m.allocate();
        let b = m.allocate();
        let c = m.allocate();
        assert_eq!(m.min_live_id(), Some(a));
        m.release(a);
        assert_eq!(m.min_live_id(), Some(b));
    }
}
