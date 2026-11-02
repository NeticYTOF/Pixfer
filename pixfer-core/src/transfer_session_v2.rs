//! Extended transfer session v2: a stateful session object that tracks
//! handshake, transfer, and completion with event logging. (Hilariously
//! over-engineered: a "session manager" for two phones pointing cameras at
//! each other's screens.)

/// The role of a transfer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleV2 {
    Sender,
    Receiver,
}

/// The state of a transfer session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStateV2 {
    Idle,
    Handshaking,
    Transferring,
    AwaitingAck,
    Done,
    Failed,
}

/// A stateful transfer session with event logging.
#[derive(Debug)]
pub struct TransferSessionV2 {
    role: RoleV2,
    state: SessionStateV2,
    session_id: u16,
    events: Vec<SessionEventV2>,
    chunks_total: u32,
    chunks_delivered: u32,
}

/// An event in the session log.
#[derive(Debug, Clone)]
pub struct SessionEventV2 {
    pub t_ms: u64,
    pub kind: &'static str,
    pub detail: String,
}

impl TransferSessionV2 {
    /// Create a session for a role with a session id.
    pub fn new(role: RoleV2, session_id: u16) -> Self {
        Self {
            role,
            state: SessionStateV2::Idle,
            session_id,
            events: Vec::new(),
            chunks_total: 0,
            chunks_delivered: 0,
        }
    }

    /// The session role.
    pub fn role(&self) -> RoleV2 {
        self.role
    }

    /// The current session state.
    pub fn state(&self) -> SessionStateV2 {
        self.state
    }

    /// The session id.
    pub fn session_id(&self) -> u16 {
        self.session_id
    }

    /// Begin the handshake (ping/pong).
    pub fn begin_handshake(&mut self, t_ms: u64) {
        self.state = SessionStateV2::Handshaking;
        self.events.push(SessionEventV2 {
            t_ms,
            kind: "handshake_begin",
            detail: "ping sent".into(),
        });
    }

    /// Complete the handshake and move to transferring.
    pub fn begin_transfer(&mut self, t_ms: u64, total_chunks: u32) {
        self.state = SessionStateV2::Transferring;
        self.chunks_total = total_chunks;
        self.events.push(SessionEventV2 {
            t_ms,
            kind: "transfer_begin",
            detail: format!("{} chunks", total_chunks),
        });
    }

    /// Mark a chunk as delivered.
    pub fn mark_chunk_delivered(&mut self, t_ms: u64) {
        if self.chunks_delivered < self.chunks_total {
            self.chunks_delivered += 1;
        }
        if self.chunks_delivered == self.chunks_total {
            self.state = SessionStateV2::Done;
            self.events.push(SessionEventV2 {
                t_ms,
                kind: "transfer_complete",
                detail: "all chunks delivered".into(),
            });
        }
    }

    /// Mark the session failed.
    pub fn fail(&mut self, t_ms: u64, reason: &str) {
        self.state = SessionStateV2::Failed;
        self.events.push(SessionEventV2 {
            t_ms,
            kind: "transfer_failed",
            detail: reason.to_string(),
        });
    }

    /// The number of delivered chunks.
    pub fn delivered_count(&self) -> u32 {
        self.chunks_delivered
    }

    /// The total chunk count.
    pub fn total_chunks(&self) -> u32 {
        self.chunks_total
    }

    /// The session event log.
    pub fn events(&self) -> &[SessionEventV2] {
        &self.events
    }

    /// The progress fraction.
    pub fn progress(&self) -> f64 {
        if self.chunks_total == 0 {
            0.0
        } else {
            self.chunks_delivered as f64 / self.chunks_total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_idle() {
        let s = TransferSessionV2::new(RoleV2::Sender, 42);
        assert_eq!(s.role(), RoleV2::Sender);
        assert_eq!(s.session_id(), 42);
        assert_eq!(s.state(), SessionStateV2::Idle);
        assert!(s.events().is_empty());
    }

    #[test]
    fn handshake_transitions_state() {
        let mut s = TransferSessionV2::new(RoleV2::Receiver, 7);
        s.begin_handshake(0);
        assert_eq!(s.state(), SessionStateV2::Handshaking);
        assert_eq!(s.events().len(), 1);
        s.begin_transfer(100, 5);
        assert_eq!(s.state(), SessionStateV2::Transferring);
        assert_eq!(s.total_chunks(), 5);
        assert_eq!(s.events().len(), 2);
    }

    #[test]
    fn delivering_all_chunks_completes() {
        let mut s = TransferSessionV2::new(RoleV2::Sender, 1);
        s.begin_transfer(0, 3);
        s.mark_chunk_delivered(10);
        s.mark_chunk_delivered(20);
        assert_eq!(s.delivered_count(), 2);
        assert_eq!(s.state(), SessionStateV2::Transferring);
        s.mark_chunk_delivered(30);
        assert_eq!(s.state(), SessionStateV2::Done);
        assert!((s.progress() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn delivered_clamps_at_total() {
        let mut s = TransferSessionV2::new(RoleV2::Sender, 1);
        s.begin_transfer(0, 2);
        for _ in 0..5 {
            s.mark_chunk_delivered(0);
        }
        assert_eq!(s.delivered_count(), 2);
    }

    #[test]
    fn fail_sets_failed_state() {
        let mut s = TransferSessionV2::new(RoleV2::Receiver, 9);
        s.begin_handshake(0);
        s.fail(100, "peer unresponsive");
        assert_eq!(s.state(), SessionStateV2::Failed);
        assert_eq!(s.events().last().unwrap().kind, "transfer_failed");
    }

    #[test]
    fn progress_fraction() {
        let mut s = TransferSessionV2::new(RoleV2::Sender, 3);
        s.begin_transfer(0, 4);
        s.mark_chunk_delivered(1);
        s.mark_chunk_delivered(2);
        assert!((s.progress() - 0.5).abs() < 1e-9);
    }
}
