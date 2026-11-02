//! Transfer checkpoint: persist resumable transfer state so a dropped light
//! link can be continued. (Hilariously over-engineered: a "checkpoint" for a
//! transfer that physically takes minutes to move a few KB.)

use std::collections::BTreeSet;

/// A resumable checkpoint for a transfer.
#[derive(Debug, Clone)]
pub struct TransferCheckpoint {
    /// The session this checkpoint belongs to.
    pub session_id: u16,
    /// Total chunk count.
    pub total_chunks: u32,
    /// The set of chunk indices already delivered.
    pub delivered: BTreeSet<u32>,
    /// A monotonically increasing checkpoint version.
    pub version: u32,
}

/// Manages transfer checkpoints keyed by session id.
#[derive(Debug, Default)]
pub struct CheckpointStore {
    by_session: std::collections::HashMap<u16, TransferCheckpoint>,
}

impl CheckpointStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of stored checkpoints.
    pub fn len(&self) -> usize {
        self.by_session.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_session.is_empty()
    }

    /// Store a checkpoint (creates or overwrites for the session).
    pub fn save(&mut self, cp: &TransferCheckpoint) {
        self.by_session.insert(cp.session_id, cp.clone());
    }

    /// Retrieve a checkpoint for a session (None if absent).
    pub fn load(&self, session_id: u16) -> Option<&TransferCheckpoint> {
        self.by_session.get(&session_id)
    }

    /// Remove a session's checkpoint.
    pub fn drop_session(&mut self, session_id: u16) {
        self.by_session.remove(&session_id);
    }
}

/// Create a fresh checkpoint for a transfer.
pub fn new_checkpoint(session_id: u16, total_chunks: u32) -> TransferCheckpoint {
    TransferCheckpoint {
        session_id,
        total_chunks,
        delivered: BTreeSet::new(),
        version: 1,
    }
}

/// A checkpoint is "complete" when every chunk has been delivered.
impl TransferCheckpoint {
    pub fn is_complete(&self) -> bool {
        self.delivered.len() == self.total_chunks as usize
    }

    /// The number of chunks still pending.
    pub fn pending_count(&self) -> u32 {
        self.total_chunks
            .saturating_sub(self.delivered.len() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_checkpoint_starts_empty() {
        let cp = new_checkpoint(42, 10);
        assert_eq!(cp.session_id, 42);
        assert_eq!(cp.total_chunks, 10);
        assert!(!cp.is_complete());
        assert_eq!(cp.pending_count(), 10);
        assert_eq!(cp.version, 1);
    }

    #[test]
    fn marking_all_chunks_completes() {
        let mut cp = new_checkpoint(7, 3);
        cp.delivered.insert(0);
        cp.delivered.insert(1);
        assert!(!cp.is_complete());
        cp.delivered.insert(2);
        assert!(cp.is_complete());
        assert_eq!(cp.pending_count(), 0);
    }

    #[test]
    fn store_save_load_roundtrip() {
        let mut store = CheckpointStore::new();
        let mut cp = new_checkpoint(9, 5);
        cp.delivered.insert(0);
        store.save(&cp);
        assert_eq!(store.len(), 1);
        let loaded = store.load(9).expect("present");
        assert_eq!(loaded.delivered.len(), 1);
        assert_eq!(loaded.total_chunks, 5);
    }

    #[test]
    fn store_drop_session() {
        let mut store = CheckpointStore::new();
        let cp = new_checkpoint(3, 4);
        store.save(&cp);
        store.drop_session(3);
        assert!(store.is_empty());
        assert!(store.load(3).is_none());
    }

    #[test]
    fn multiple_sessions_independent() {
        let mut store = CheckpointStore::new();
        store.save(&new_checkpoint(1, 8));
        store.save(&new_checkpoint(2, 16));
        assert_eq!(store.len(), 2);
        assert_eq!(store.load(1).unwrap().total_chunks, 8);
        assert_eq!(store.load(2).unwrap().total_chunks, 16);
    }
}
