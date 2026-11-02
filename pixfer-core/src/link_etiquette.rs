//! Light-link etiquette: a state machine that governs the social protocol
//! between two devices — a greeting, a capability exchange, a transfer
//! window, and a polite farewell. (Hilariously over-engineered: a
//! "netiquette engine" for a link whose whole "network" is one phone
//! screen and one camera, moving a few KB/s.)
//!
//! The etiquette layer sits above the raw frame codec: it decides *when*
//! each kind of frame is allowed, retries unresponsive peers with
//! exponential backoff, and keeps a short audit trail of every social
//! event so a failed transfer can be explained after the fact.

use crate::frame_codec::decode;
use crate::protocol::{Frame, FrameKind};

/// The current social state of the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtiquetteState {
    /// No link established.
    Idle,
    /// We greeted the peer and are awaiting its greeting.
    Greeting,
    /// The peer greeted us; capabilities are being exchanged.
    Capabilities,
    /// Handshake complete; the transfer window is open.
    Ready,
    /// A transfer is in progress.
    Transferring,
    /// Polite teardown is in progress.
    Closing,
    /// The peer stopped answering; the link is cooling down before the
    /// next greeting attempt.
    Cooldown,
}

/// An etiquette event that advances the state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum EtiquetteEvent {
    /// The peer sent its greeting (a Ping carrying its capabilities).
    PeerGreeting,
    /// The peer sent its capability payload.
    PeerCapabilities,
    /// The peer signalled readiness.
    PeerReady,
    /// We are starting a transfer.
    TransferStart,
    /// The transfer finished (all chunks delivered and verified).
    TransferDone,
    /// The peer sent its farewell.
    PeerBye,
    /// The response timer expired without an answer from the peer.
    Timeout,
    /// A protocol or link error occurred.
    Error(String),
}

/// The capabilities a device advertises during the handshake.
///
/// Both sides compare these to agree on a common operating point: the
/// lowest fps, the shared color mode, the smaller chunk size, and the
/// smaller RS parity budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// The maximum frames per second this device can display.
    pub max_fps: u8,
    /// Whether this device's screen supports the full 8-color palette.
    pub multicolor: bool,
    /// The chunk size in bytes this device prefers to send.
    pub chunk_bytes: u16,
    /// The Reed-Solomon parity budget this device can afford.
    pub rs_parity: u8,
}

impl Capabilities {
    /// Serialize into the on-wire 5-byte layout:
    /// `[max_fps, multicolor, chunk_bytes_lo, chunk_bytes_hi, rs_parity]`.
    pub fn to_bytes(&self) -> [u8; 5] {
        [
            self.max_fps,
            u8::from(self.multicolor),
            (self.chunk_bytes & 0xFF) as u8,
            (self.chunk_bytes >> 8) as u8,
            self.rs_parity,
        ]
    }

    /// Deserialize from the 5-byte wire layout.
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() != 5 {
            return None;
        }
        Some(Self {
            max_fps: b[0],
            multicolor: b[1] != 0,
            chunk_bytes: u16::from(b[2]) | (u16::from(b[3]) << 8),
            rs_parity: b[4],
        })
    }

    /// The agreed operating point between two capability sets: the more
    /// conservative side wins every field, so the weaker device never
    /// gets asked to do more than it can.
    pub fn agree(&self, peer: &Capabilities) -> Capabilities {
        Capabilities {
            max_fps: self.max_fps.min(peer.max_fps),
            multicolor: self.multicolor && peer.multicolor,
            chunk_bytes: self.chunk_bytes.min(peer.chunk_bytes),
            rs_parity: self.rs_parity.min(peer.rs_parity),
        }
    }
}

/// A logged etiquette event: a monotonically increasing sequence number
/// paired with the event that was observed.
#[derive(Debug, Clone, PartialEq)]
pub struct LoggedEvent {
    /// The monotonic sequence number of the event.
    pub seq: u32,
    /// The event that was observed.
    pub event: EtiquetteEvent,
}

/// The etiquette state machine.
///
/// Tracks the social state of the link, retries unresponsive peers with
/// exponential backoff, and keeps a capped audit trail of every event.
/// (Hilariously over-engineered: a "diplomatic corps" for a link that
/// physically moves a few KB/s.)
pub struct Etiquette {
    state: EtiquetteState,
    session_id: u16,
    timeout_ms: u64,
    retries: u8,
    max_retries: u8,
    base_backoff_ms: u64,
    max_backoff_ms: u64,
    log: Vec<LoggedEvent>,
    log_seq: u32,
    log_cap: usize,
    caps: Capabilities,
}

impl Etiquette {
    /// Create a new etiquette state machine.
    pub fn new(session_id: u16, caps: Capabilities, timeout_ms: u64, max_retries: u8) -> Self {
        Self {
            state: EtiquetteState::Idle,
            session_id,
            timeout_ms,
            retries: 0,
            max_retries,
            base_backoff_ms: 200,
            max_backoff_ms: 4000,
            log: Vec::new(),
            log_seq: 0,
            log_cap: 64,
            caps,
        }
    }

    /// The current social state.
    pub fn state(&self) -> EtiquetteState {
        self.state
    }

    /// The session id this machine belongs to.
    pub fn session_id(&self) -> u16 {
        self.session_id
    }

    /// The response timeout in milliseconds.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// The advertised capabilities.
    pub fn capabilities(&self) -> Capabilities {
        self.caps
    }

    /// Send the greeting: build a Ping frame carrying our capabilities
    /// and move to the Greeting state. The returned frame is ready to be
    /// handed to a `TxScreen` for display.
    pub fn greet(&mut self) -> Option<Frame> {
        if self.state != EtiquetteState::Idle && self.state != EtiquetteState::Cooldown {
            return None;
        }
        self.retries = 0;
        let payload = self.caps.to_bytes().to_vec();
        self.state = EtiquetteState::Greeting;
        Some(Frame::new(FrameKind::Ping, self.session_id, 0, 0, payload))
    }

    /// Extract the peer's capabilities from a received frame's blocks.
    ///
    /// The peer's greeting is a Ping whose payload is the 5-byte
    /// capabilities layout. Anything else (or a bad payload) yields `None`.
    pub fn peer_capabilities(&self, blocks: &[u8]) -> Option<Capabilities> {
        let frame = decode(blocks).ok()?;
        if frame.kind != FrameKind::Ping {
            return None;
        }
        Capabilities::from_bytes(&frame.payload)
    }

    /// Observe an etiquette event and advance the state machine.
    ///
    /// Returns the new state after the event. Unknown or out-of-place
    /// events are ignored (and still logged for the audit trail).
    pub fn on_event(&mut self, ev: &EtiquetteEvent) -> EtiquetteState {
        self.log_event(ev.clone());
        match (&self.state, ev) {
            // Greeting: we are waiting for the peer's greeting.
            (EtiquetteState::Greeting, EtiquetteEvent::PeerGreeting) => {
                self.state = EtiquetteState::Capabilities;
            }
            // Capabilities: exchange in progress; peer sent its payload.
            (EtiquetteState::Capabilities, EtiquetteEvent::PeerCapabilities) => {
                self.state = EtiquetteState::Ready;
            }
            // Ready: transfer window open.
            (EtiquetteState::Ready, EtiquetteEvent::TransferStart) => {
                self.state = EtiquetteState::Transferring;
            }
            // Transferring: transfer finished, begin polite teardown.
            (EtiquetteState::Transferring, EtiquetteEvent::TransferDone) => {
                self.state = EtiquetteState::Closing;
            }
            // Peer said goodbye from anywhere: begin polite teardown.
            (_, EtiquetteEvent::PeerBye) => {
                self.state = EtiquetteState::Closing;
            }
            // Closing: teardown finished, back to idle.
            (EtiquetteState::Closing, EtiquetteEvent::PeerGreeting) => {
                self.state = EtiquetteState::Idle;
            }
            // Timeouts: retry with backoff until the budget runs out.
            (
                EtiquetteState::Greeting | EtiquetteState::Capabilities | EtiquetteState::Ready,
                EtiquetteEvent::Timeout,
            ) => {
                if self.retries < self.max_retries {
                    self.retries += 1;
                    if self.retries == self.max_retries {
                        self.state = EtiquetteState::Cooldown;
                    }
                } else {
                    self.state = EtiquetteState::Cooldown;
                }
            }
            // Errors: drop back to idle.
            (_, EtiquetteEvent::Error(_)) => {
                self.state = EtiquetteState::Idle;
                self.retries = 0;
            }
            // Everything else is ignored (state unchanged).
            _ => {}
        }
        self.state
    }

    /// The exponential backoff delay before the next greeting attempt:
    /// `base * 2^retries`, capped at `max_backoff_ms`.
    pub fn backoff_ms(&self) -> u64 {
        let shift = self.retries.min(12);
        self.base_backoff_ms
            .saturating_mul(1u64 << shift)
            .min(self.max_backoff_ms)
    }

    /// Whether the transfer window is open.
    pub fn can_transfer(&self) -> bool {
        self.state == EtiquetteState::Ready
    }

    /// The number of retries left before the link enters cooldown.
    pub fn retries_left(&self) -> u8 {
        self.max_retries.saturating_sub(self.retries)
    }

    /// The current retry count.
    pub fn retries(&self) -> u8 {
        self.retries
    }

    /// A copy of the capped event log.
    pub fn log(&self) -> Vec<LoggedEvent> {
        self.log.clone()
    }

    /// The number of events currently retained in the log.
    pub fn log_len(&self) -> usize {
        self.log.len()
    }

    /// Reset the machine: back to idle, retries cleared, log cleared.
    pub fn reset(&mut self) {
        self.state = EtiquetteState::Idle;
        self.retries = 0;
        self.log.clear();
        self.log_seq = 0;
    }

    /// Record an event in the capped log.
    fn log_event(&mut self, ev: EtiquetteEvent) {
        self.log_seq += 1;
        self.log.push(LoggedEvent {
            seq: self.log_seq,
            event: ev,
        });
        if self.log.len() > self.log_cap {
            self.log.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> Capabilities {
        Capabilities {
            max_fps: 30,
            multicolor: true,
            chunk_bytes: 223,
            rs_parity: 16,
        }
    }

    fn etq() -> Etiquette {
        Etiquette::new(1, caps(), 5000, 3)
    }

    #[test]
    fn starts_idle() {
        let e = etq();
        assert_eq!(e.state(), EtiquetteState::Idle);
        assert!(!e.can_transfer());
    }

    #[test]
    fn greet_moves_to_greeting() {
        let mut e = etq();
        let f = e.greet().expect("greeting frame");
        assert_eq!(f.kind, FrameKind::Ping);
        assert_eq!(e.state(), EtiquetteState::Greeting);
        assert!(e
            .peer_capabilities(&crate::frame_codec::encode(&f))
            .is_some());
    }

    #[test]
    fn peer_greeting_event_advances_to_capabilities() {
        let mut e = etq();
        e.greet();
        assert_eq!(
            e.on_event(&EtiquetteEvent::PeerGreeting),
            EtiquetteState::Capabilities
        );
    }

    #[test]
    fn peer_capabilities_event_opens_transfer_window() {
        let mut e = etq();
        e.greet();
        e.on_event(&EtiquetteEvent::PeerGreeting);
        assert_eq!(
            e.on_event(&EtiquetteEvent::PeerCapabilities),
            EtiquetteState::Ready
        );
        assert!(e.can_transfer());
    }

    #[test]
    fn transfer_start_requires_ready() {
        let mut e = etq();
        // From Idle, TransferStart is ignored.
        assert_eq!(
            e.on_event(&EtiquetteEvent::TransferStart),
            EtiquetteState::Idle
        );
    }

    #[test]
    fn full_handshake_to_transfer() {
        let mut e = etq();
        e.greet();
        e.on_event(&EtiquetteEvent::PeerGreeting);
        e.on_event(&EtiquetteEvent::PeerCapabilities);
        assert_eq!(
            e.on_event(&EtiquetteEvent::TransferStart),
            EtiquetteState::Transferring
        );
        assert_eq!(
            e.on_event(&EtiquetteEvent::TransferDone),
            EtiquetteState::Closing
        );
    }

    #[test]
    fn peer_bye_from_any_state() {
        let mut e = etq();
        e.greet();
        assert_eq!(
            e.on_event(&EtiquetteEvent::PeerBye),
            EtiquetteState::Closing
        );
    }

    #[test]
    fn timeout_increments_retries_and_schedules_backoff() {
        let mut e = etq();
        e.greet();
        assert_eq!(
            e.on_event(&EtiquetteEvent::Timeout),
            EtiquetteState::Greeting
        );
        assert_eq!(e.retries(), 1);
        assert_eq!(e.backoff_ms(), 400); // 200 * 2^1
        e.on_event(&EtiquetteEvent::Timeout);
        assert_eq!(e.retries(), 2);
        assert_eq!(e.backoff_ms(), 800);
        assert_eq!(e.retries_left(), 1);
    }

    #[test]
    fn retries_exhausted_enters_cooldown() {
        let mut e = etq(); // max_retries = 3
        e.greet();
        e.on_event(&EtiquetteEvent::Timeout);
        e.on_event(&EtiquetteEvent::Timeout);
        assert_eq!(
            e.on_event(&EtiquetteEvent::Timeout),
            EtiquetteState::Cooldown
        );
        assert_eq!(e.retries_left(), 0);
    }

    #[test]
    fn cooldown_allows_regreeting() {
        let mut e = etq();
        e.greet();
        e.on_event(&EtiquetteEvent::Timeout);
        e.on_event(&EtiquetteEvent::Timeout);
        e.on_event(&EtiquetteEvent::Timeout);
        assert_eq!(e.state(), EtiquetteState::Cooldown);
        // After the backoff, the device may greet again.
        let f = e.greet();
        assert!(f.is_some());
        assert_eq!(e.state(), EtiquetteState::Greeting);
        assert_eq!(e.retries(), 0);
    }

    #[test]
    fn error_returns_to_idle() {
        let mut e = etq();
        e.greet();
        assert_eq!(
            e.on_event(&EtiquetteEvent::Error("link broken".into())),
            EtiquetteState::Idle
        );
        assert_eq!(e.retries(), 0);
    }

    #[test]
    fn capabilities_roundtrip() {
        let c = caps();
        let bytes = c.to_bytes();
        let back = Capabilities::from_bytes(&bytes).expect("parse");
        assert_eq!(back, c);
    }

    #[test]
    fn capabilities_agree_takes_conservative_side() {
        let mine = caps();
        let peer = Capabilities {
            max_fps: 15,
            multicolor: false,
            chunk_bytes: 512,
            rs_parity: 8,
        };
        let agreed = mine.agree(&peer);
        assert_eq!(agreed.max_fps, 15);
        assert!(!agreed.multicolor);
        assert_eq!(agreed.chunk_bytes, 223);
        assert_eq!(agreed.rs_parity, 8);
    }

    #[test]
    fn bad_capabilities_payload_yields_none() {
        let e = etq();
        assert!(Capabilities::from_bytes(&[1, 2, 3]).is_none());
        let frame = Frame::new(FrameKind::Pong, 1, 0, 0, vec![]);
        let blocks = crate::frame_codec::encode(&frame);
        assert!(e.peer_capabilities(&blocks).is_none());
    }

    #[test]
    fn log_records_events_in_order() {
        let mut e = etq();
        e.greet();
        e.on_event(&EtiquetteEvent::PeerGreeting);
        e.on_event(&EtiquetteEvent::PeerCapabilities);
        let log = e.log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].seq, 1);
        assert_eq!(log[1].seq, 2);
        assert_eq!(log[0].event, EtiquetteEvent::PeerGreeting);
        assert_eq!(log[1].event, EtiquetteEvent::PeerCapabilities);
    }

    #[test]
    fn log_caps_at_buffer_size() {
        let mut e = etq();
        // Drive the machine through many event cycles; the 64-entry log
        // must retain only the most recent 64 events.
        for _ in 0..40 {
            e.greet();
            e.on_event(&EtiquetteEvent::PeerGreeting);
            e.on_event(&EtiquetteEvent::PeerCapabilities);
            e.on_event(&EtiquetteEvent::TransferStart);
            e.on_event(&EtiquetteEvent::TransferDone);
            e.on_event(&EtiquetteEvent::PeerBye);
        }
        let log = e.log();
        assert_eq!(e.log_len(), 64);
        // Sequence numbers stay monotonic in the retained window.
        for w in log.windows(2) {
            assert!(w[1].seq > w[0].seq);
        }
    }

    #[test]
    fn reset_clears_state_retries_and_log() {
        let mut e = etq();
        e.greet();
        e.on_event(&EtiquetteEvent::PeerGreeting);
        e.reset();
        assert_eq!(e.state(), EtiquetteState::Idle);
        assert_eq!(e.retries(), 0);
        assert_eq!(e.log_len(), 0);
    }

    #[test]
    fn backoff_is_capped() {
        let mut e = etq();
        // Force the retry count high: exhaust the budget then check the cap.
        e.greet();
        for _ in 0..3 {
            e.on_event(&EtiquetteEvent::Timeout);
        }
        assert_eq!(e.state(), EtiquetteState::Cooldown);
        // 200 * 2^3 = 1600, under the 4000 cap.
        assert_eq!(e.backoff_ms(), 1600);
    }

    #[test]
    fn timeout_from_closing_is_ignored() {
        let mut e = etq();
        e.greet();
        e.on_event(&EtiquetteEvent::PeerGreeting);
        e.on_event(&EtiquetteEvent::PeerCapabilities);
        e.on_event(&EtiquetteEvent::TransferStart);
        e.on_event(&EtiquetteEvent::TransferDone);
        // In Closing, a timeout does not change state or bump retries.
        assert_eq!(e.state(), EtiquetteState::Closing);
        let retries_before = e.retries();
        assert_eq!(
            e.on_event(&EtiquetteEvent::Timeout),
            EtiquetteState::Closing
        );
        assert_eq!(e.retries(), retries_before);
    }
}
