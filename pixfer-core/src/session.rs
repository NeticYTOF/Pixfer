//! Transfer-session state machine: stop-and-wait ARQ with bounded retries,
//! used by both ends (TX renders PING/CHUNK/END; RX renders PONG/ACK/NAK).

use crate::protocol::{Frame, FrameKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Tx,
    Rx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    WaitingPong,
    Transferring,
    AwaitingAck,
    Done,
    Failed,
}

const MAX_RETRIES: u32 = 3;

pub struct TransferSession {
    role: Role,
    pub session_id: u16,
    pub total_chunks: u32,
    state: SessionState,
    next_chunk: u32,
    pending_chunk: Option<u32>,
    nak_next: bool,
    pong_sent: bool,
    retries: std::collections::HashMap<u32, u32>,
    received: std::collections::HashSet<u32>,
}

impl TransferSession {
    pub fn tx(session_id: u16, total_chunks: u32) -> Self {
        Self {
            role: Role::Tx,
            session_id,
            total_chunks,
            state: SessionState::WaitingPong,
            next_chunk: 0,
            pending_chunk: None,
            nak_next: false,
            pong_sent: false,
            retries: std::collections::HashMap::new(),
            received: std::collections::HashSet::new(),
        }
    }

    pub fn rx(session_id: u16, total_chunks: u32) -> Self {
        Self {
            role: Role::Rx,
            session_id,
            total_chunks,
            state: SessionState::WaitingPong,
            next_chunk: 0,
            pending_chunk: None,
            nak_next: false,
            pong_sent: false,
            retries: std::collections::HashMap::new(),
            received: std::collections::HashSet::new(),
        }
    }

    /// The next frame this side should render. Returns None when idle.
    pub fn next_frame(&mut self) -> Option<Frame> {
        match self.role {
            Role::Tx => match self.state {
                SessionState::WaitingPong => Some(Frame::new(
                    FrameKind::Ping,
                    self.session_id,
                    0,
                    self.total_chunks,
                    Vec::new(),
                )),
                SessionState::Transferring => {
                    if self.next_chunk < self.total_chunks {
                        let f = Frame::new(
                            FrameKind::Chunk,
                            self.session_id,
                            self.next_chunk,
                            self.total_chunks,
                            Vec::new(),
                        );
                        self.pending_chunk = Some(self.next_chunk);
                        self.state = SessionState::AwaitingAck;
                        Some(f)
                    } else {
                        self.state = SessionState::Done;
                        Some(Frame::new(
                            FrameKind::End,
                            self.session_id,
                            self.next_chunk,
                            self.total_chunks,
                            Vec::new(),
                        ))
                    }
                }
                SessionState::AwaitingAck => self.pending_chunk.map(|c| {
                    Frame::new(
                        FrameKind::Chunk,
                        self.session_id,
                        c,
                        self.total_chunks,
                        Vec::new(),
                    )
                }),
                _ => None,
            },
            Role::Rx => {
                if !self.pong_sent {
                    self.pong_sent = true;
                    self.state = SessionState::Transferring;
                    Some(Frame::new(
                        FrameKind::Pong,
                        self.session_id,
                        0,
                        self.total_chunks,
                        Vec::new(),
                    ))
                } else if let Some(c) = self.pending_chunk {
                    let kind = if self.nak_next {
                        FrameKind::Nak
                    } else {
                        FrameKind::Ack
                    };
                    Some(Frame::new(
                        kind,
                        self.session_id,
                        c,
                        self.total_chunks,
                        Vec::new(),
                    ))
                } else {
                    None
                }
            }
        }
    }

    /// TX: peer answered our PING with a PONG — start the transfer.
    pub fn on_pong(&mut self) {
        if self.role == Role::Tx && self.state == SessionState::WaitingPong {
            self.state = SessionState::Transferring;
        }
    }

    /// RX: peer sent a PING — mark the session started; next_frame will emit the PONG.
    pub fn on_ping(&mut self) {
        if self.role == Role::Rx {
            self.state = SessionState::Transferring;
        }
    }

    /// RX: chunk verified OK — queue an ACK.
    pub fn on_chunk_ok(&mut self, chunk: u32) {
        if self.role == Role::Rx {
            self.pending_chunk = Some(chunk);
            self.nak_next = false;
            self.state = SessionState::AwaitingAck;
            self.received.insert(chunk);
        }
    }

    /// RX: chunk failed CRC/RS — queue a NAK.
    pub fn on_chunk_err(&mut self, chunk: u32) {
        if self.role == Role::Rx {
            self.pending_chunk = Some(chunk);
            self.nak_next = true;
            self.state = SessionState::AwaitingAck;
        }
    }

    /// TX: peer ACKed a chunk — advance.
    pub fn on_ack(&mut self, chunk: u32) {
        if self.role == Role::Tx
            && self.state == SessionState::AwaitingAck
            && self.pending_chunk == Some(chunk)
        {
            self.next_chunk += 1;
            self.pending_chunk = None;
            if self.next_chunk >= self.total_chunks {
                self.state = SessionState::Transferring; // next_frame will now emit END
            } else {
                self.state = SessionState::Transferring;
            }
        }
    }

    /// TX: peer NAKed a chunk — retransmit it.
    pub fn on_nak(&mut self, chunk: u32) {
        if self.role == Role::Tx && self.state == SessionState::AwaitingAck {
            self.pending_chunk = Some(chunk);
        }
    }

    /// TX: no ACK arrived in time — retry the pending chunk, up to MAX_RETRIES.
    pub fn on_timeout(&mut self) -> Option<Frame> {
        if self.role != Role::Tx || self.state != SessionState::AwaitingAck {
            return None;
        }
        let chunk = match self.pending_chunk {
            Some(c) => c,
            None => return None,
        };
        let n = self.retries.get(&chunk).copied().unwrap_or(0) + 1;
        self.retries.insert(chunk, n);
        if n > MAX_RETRIES {
            self.state = SessionState::Failed;
            None
        } else {
            Some(Frame::new(
                FrameKind::Chunk,
                self.session_id,
                chunk,
                self.total_chunks,
                Vec::new(),
            ))
        }
    }

    /// RX bookkeeping: has every chunk been received?
    pub fn mark_received(&mut self, chunk: u32) {
        self.received.insert(chunk);
    }

    pub fn all_received(&self) -> bool {
        self.received.len() == self.total_chunks as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tx_full_flow() {
        let mut s = TransferSession::tx(7, 4);
        assert_eq!(s.next_frame().unwrap().kind, FrameKind::Ping);
        s.on_pong();
        for i in 0..4u32 {
            let f = s.next_frame().unwrap();
            assert_eq!(f.kind, FrameKind::Chunk);
            assert_eq!(f.chunk_idx, i);
            s.on_ack(i);
        }
        assert_eq!(s.next_frame().unwrap().kind, FrameKind::End);
        assert!(s.next_frame().is_none());
    }

    #[test]
    fn tx_retransmit_after_nak() {
        let mut s = TransferSession::tx(7, 4);
        s.on_pong();
        let f1 = s.next_frame().unwrap();
        assert_eq!(f1.chunk_idx, 0);
        s.on_nak(0);
        let f2 = s.next_frame().unwrap();
        assert_eq!(f2.kind, FrameKind::Chunk);
        assert_eq!(f2.chunk_idx, 0); // same chunk retransmitted
    }

    #[test]
    fn rx_pong_then_ack_nak() {
        let mut s = TransferSession::rx(7, 4);
        s.on_ping();
        assert_eq!(s.next_frame().unwrap().kind, FrameKind::Pong);
        s.on_chunk_ok(1);
        assert_eq!(s.next_frame().unwrap().kind, FrameKind::Ack);
        s.on_chunk_err(2);
        assert_eq!(s.next_frame().unwrap().kind, FrameKind::Nak);
    }

    #[test]
    fn timeout_fails_after_max_retries() {
        let mut s = TransferSession::tx(7, 1);
        s.on_pong();
        let _ = s.next_frame().unwrap(); // chunk 0 pending
        assert!(s.on_timeout().is_some()); // retry 1
        assert!(s.on_timeout().is_some()); // retry 2
        assert!(s.on_timeout().is_some()); // retry 3
        assert!(s.on_timeout().is_none()); // gave up
        assert_eq!(s.state, SessionState::Failed);
    }
}
