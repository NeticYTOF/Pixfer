//! Transfer rollback: undo a partially-completed transfer by restoring
//! the receiver's state to its pre-transfer snapshot. (Hilariously
//! over-engineered: a "transactional transfer" for a light link.)

use crate::protocol::{Frame, FrameKind};
use crate::reassembly::Reassembler;

/// A snapshot of the receiver's reassembly state before a transfer.
#[derive(Debug, Clone)]
pub struct ReassemblySnapshot {
    pub received: Vec<u32>,
    pub total_chunks: u32,
    pub session_id: u16,
    pub complete: bool,
}

/// A rollback manager for in-progress transfers.
///
/// Captures a snapshot before a transfer starts; on failure or cancel,
/// restores the receiver to that snapshot.
///
/// The receiver's `Reassembler` is swapped out for a freshly-constructed
/// one in the snapshot's state, so the caller must replace their own.
#[derive(Debug)]
pub struct TransferRollback {
    snapshot: Option<ReassemblySnapshot>,
    active: bool,
}

impl TransferRollback {
    pub fn new() -> Self {
        Self {
            snapshot: None,
            active: false,
        }
    }

    /// Capture the current reassembly state as a snapshot.
    pub fn begin(&mut self, reassembler: &Reassembler) {
        self.snapshot = Some(ReassemblySnapshot {
            received: reassembler.received_indices(),
            total_chunks: reassembler.total_chunks(),
            session_id: reassembler.session_id(),
            complete: reassembler.is_complete(),
        });
        self.active = true;
    }

    /// Build a fresh reassembler in the snapshot's state.
    pub fn restored(&self) -> Option<Reassembler> {
        self.snapshot.as_ref().map(|snap| {
            let mut r = Reassembler::new(snap.session_id, snap.total_chunks);
            for &idx in &snap.received {
                let f = Frame::new(
                    FrameKind::Chunk,
                    snap.session_id,
                    idx,
                    snap.total_chunks,
                    vec![0],
                );
                r.add(&f);
            }
            r
        })
    }

    /// Mark the transfer as rolled back (clears the active flag).
    pub fn rollback(&mut self) -> Option<Reassembler> {
        if !self.active {
            return None;
        }
        let restored = self.restored();
        self.active = false;
        self.snapshot = None;
        restored
    }

    /// Whether a transfer is currently in progress.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Mark the transfer as successfully completed (no rollback needed).
    pub fn commit(&mut self) {
        self.active = false;
        self.snapshot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reassembly::Reassembler;

    fn reassembler_with(session_id: u16, total_chunks: u32, received: &[u32]) -> Reassembler {
        let mut r = Reassembler::new(session_id, total_chunks);
        for &idx in received {
            let f = Frame::new(FrameKind::Chunk, session_id, idx, total_chunks, vec![0]);
            r.add(&f);
        }
        r
    }

    #[test]
    fn rollback_restores_pre_transfer_state() {
        let mut rb = TransferRollback::new();
        let r1 = reassembler_with(7, 5, &[0, 1, 2]);
        rb.begin(&r1);
        assert!(rb.is_active());
        // The restored reassembler should match the snapshot.
        let restored = rb.restored().unwrap();
        assert_eq!(restored.received_indices(), vec![0, 1, 2]);
        // A rollback returns the restored reassembler and clears the active flag.
        let rb2 = rb.rollback();
        assert!(rb2.is_some());
        assert!(!rb.is_active());
    }

    #[test]
    fn commit_disables_rollback() {
        let mut rb = TransferRollback::new();
        let r1 = reassembler_with(7, 4, &[0, 1]);
        rb.begin(&r1);
        rb.commit();
        assert!(!rb.is_active());
        assert!(rb.rollback().is_none());
    }

    #[test]
    fn rollback_without_begin_is_noop() {
        let mut rb = TransferRollback::new();
        assert!(!rb.is_active());
        assert!(rb.rollback().is_none());
    }

    #[test]
    fn snapshot_captures_received_and_complete() {
        let r = reassembler_with(7, 4, &[0, 1, 2]);
        let mut rb = TransferRollback::new();
        rb.begin(&r);
        if let Some(snap) = rb.snapshot {
            assert_eq!(snap.received, vec![0, 1, 2]);
            assert_eq!(snap.total_chunks, 4);
            assert_eq!(snap.session_id, 7);
            assert!(!snap.complete);
        }
    }
}
