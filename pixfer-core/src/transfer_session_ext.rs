//! Transfer session extension: wraps the base `TransferSession` with
//! reliability tracking, progress, and a richer state view. (Hilariously
//! over-engineered: a "session wrapper" for a session that already exists.)

use crate::reliability::ReliabilityTracker;
use crate::session::{SessionState, TransferSession};

/// An extended transfer session that layers reliability + progress on top
/// of the base session.
pub struct TransferSessionExt {
    session: TransferSession,
    reliability: ReliabilityTracker,
    chunks_done: u32,
    state: SessionState,
}

impl TransferSessionExt {
    pub fn tx(session_id: u16, total_chunks: u32) -> Self {
        Self {
            session: TransferSession::tx(session_id, total_chunks),
            reliability: ReliabilityTracker::new(),
            chunks_done: 0,
            state: SessionState::WaitingPong,
        }
    }

    pub fn rx(session_id: u16, total_chunks: u32) -> Self {
        Self {
            session: TransferSession::rx(session_id, total_chunks),
            reliability: ReliabilityTracker::new(),
            chunks_done: 0,
            state: SessionState::WaitingPong,
        }
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn session_mut(&mut self) -> &mut TransferSession {
        &mut self.session
    }

    /// Advance state after a PONG (mirrors the session's on_pong transition).
    pub fn advance_to_transferring(&mut self) {
        if self.state == SessionState::WaitingPong {
            self.state = SessionState::Transferring;
        }
    }

    pub fn mark_chunk_done(&mut self) {
        self.chunks_done = self.chunks_done.saturating_add(1);
    }

    pub fn chunks_done(&self) -> u32 {
        self.chunks_done
    }

    pub fn total_chunks(&self) -> u32 {
        self.session.total_chunks
    }

    pub fn progress_fraction(&self) -> f64 {
        let total = self.total_chunks();
        if total == 0 {
            return 0.0;
        }
        (self.chunks_done as f64) / (total as f64)
    }

    pub fn reliability_quality(&self) -> f64 {
        self.reliability.quality_score()
    }

    pub fn session(&self) -> &TransferSession {
        &self.session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tx_ext_starts_waiting_pong() {
        let mut s = TransferSessionExt::tx(7, 4);
        assert_eq!(s.state(), SessionState::WaitingPong);
        s.session_mut().on_pong();
        s.advance_to_transferring();
        assert_eq!(s.state(), SessionState::Transferring);
    }

    #[test]
    fn progress_advances() {
        let mut s = TransferSessionExt::tx(7, 4);
        s.session_mut().on_pong();
        s.advance_to_transferring();
        for _ in 0..4 {
            s.mark_chunk_done();
        }
        assert!((s.progress_fraction() - 1.0).abs() < 1e-9);
        assert_eq!(s.chunks_done(), 4);
        assert_eq!(s.total_chunks(), 4);
    }

    #[test]
    fn rx_ext() {
        let s = TransferSessionExt::rx(7, 4);
        assert_eq!(s.state(), SessionState::WaitingPong);
    }

    #[test]
    fn reliability_quality_perfect() {
        let mut s = TransferSessionExt::tx(7, 4);
        s.session_mut().on_pong();
        s.advance_to_transferring();
        s.reliability.mark_delivered(4);
        assert!((s.reliability_quality() - 1.0).abs() < 1e-9);
    }
}
