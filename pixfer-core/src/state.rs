//! A finite state machine for the transfer lifecycle with guarded
//! transitions and a full audit trail. (Hilariously over-engineered: a
//! formal FSM for a light link.)

use std::collections::VecDeque;

/// The lifecycle states of a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsmState {
    Idle,
    Transferring,
    Paused,
    Completed,
    Failed,
}

impl FsmState {
    pub fn as_str(self) -> &'static str {
        match self {
            FsmState::Idle => "idle",
            FsmState::Transferring => "transferring",
            FsmState::Paused => "paused",
            FsmState::Completed => "completed",
            FsmState::Failed => "failed",
        }
    }
}

/// Events that drive the FSM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsmEvent {
    Start,
    Pause,
    Resume,
    Complete,
    Fail,
    Abort,
}

/// The transfer FSM with a transition table and an audit trail.
pub struct TransferFsm {
    state: FsmState,
    history: VecDeque<(FsmState, FsmState)>,
}

impl Default for TransferFsm {
    fn default() -> Self {
        Self {
            state: FsmState::Idle,
            history: VecDeque::new(),
        }
    }
}

impl TransferFsm {
    pub fn new() -> Self {
        Self {
            state: FsmState::Idle,
            history: VecDeque::new(),
        }
    }

    /// The transition table: which events are legal from each state.
    fn can(&self, ev: FsmEvent) -> bool {
        match (self.state, ev) {
            (FsmState::Idle, FsmEvent::Start) => true,
            (
                FsmState::Transferring,
                FsmEvent::Pause | FsmEvent::Complete | FsmEvent::Fail | FsmEvent::Abort,
            ) => true,
            (FsmState::Paused, FsmEvent::Resume | FsmEvent::Abort) => true,
            (FsmState::Completed, _) => false,
            (FsmState::Failed, _) => false,
            _ => false,
        }
    }

    /// Apply an event if legal; returns true when the transition happened.
    pub fn on_event(&mut self, ev: FsmEvent) -> bool {
        if !self.can(ev) {
            return false;
        }
        let from = self.state;
        self.state = match (self.state, ev) {
            (FsmState::Idle, FsmEvent::Start) => FsmState::Transferring,
            (FsmState::Transferring, FsmEvent::Pause) => FsmState::Paused,
            (FsmState::Transferring, FsmEvent::Complete) => FsmState::Completed,
            (FsmState::Transferring, FsmEvent::Fail) => FsmState::Failed,
            (FsmState::Transferring, FsmEvent::Abort) => FsmState::Failed,
            (FsmState::Paused, FsmEvent::Resume) => FsmState::Transferring,
            (FsmState::Paused, FsmEvent::Abort) => FsmState::Failed,
            _ => self.state,
        };
        if self.state != from {
            self.history.push_back((from, self.state));
        }
        self.state != from
    }

    pub fn state(&self) -> FsmState {
        self.state
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.state, FsmState::Completed | FsmState::Failed)
    }

    /// The audit trail of (from, to) transitions.
    pub fn history(&self) -> &VecDeque<(FsmState, FsmState)> {
        &self.history
    }

    /// A human-readable transition log.
    pub fn history_report(&self) -> String {
        self.history
            .iter()
            .map(|(f, t)| format!("{} -> {}", f.as_str(), t.as_str()))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transferring_to_completed() {
        let mut m = TransferFsm::new();
        assert_eq!(m.state(), FsmState::Idle);
        assert!(m.on_event(FsmEvent::Start));
        assert_eq!(m.state(), FsmState::Transferring);
        assert!(m.on_event(FsmEvent::Pause));
        assert_eq!(m.state(), FsmState::Paused);
        assert!(m.on_event(FsmEvent::Resume));
        assert_eq!(m.state(), FsmState::Transferring);
        assert!(m.on_event(FsmEvent::Complete));
        assert_eq!(m.state(), FsmState::Completed);
        assert!(m.is_terminal());
        assert!(!m.on_event(FsmEvent::Start)); // terminal: no more transitions
        assert_eq!(m.history().len(), 4);
    }

    #[test]
    fn abort_from_transferring_fails() {
        let mut m = TransferFsm::new();
        m.on_event(FsmEvent::Start);
        assert!(m.on_event(FsmEvent::Abort));
        assert_eq!(m.state(), FsmState::Failed);
        assert!(m.is_terminal());
    }

    #[test]
    fn illegal_events_are_rejected() {
        let mut m = TransferFsm::new();
        assert!(!m.on_event(FsmEvent::Pause)); // Idle can't pause
        assert!(!m.on_event(FsmEvent::Complete)); // Idle can't complete
        assert_eq!(m.state(), FsmState::Idle);
        assert_eq!(m.history().len(), 0);
    }

    #[test]
    fn history_report_lists_transitions() {
        let mut m = TransferFsm::new();
        m.on_event(FsmEvent::Start);
        m.on_event(FsmEvent::Fail);
        let rep = m.history_report();
        assert!(rep.contains("idle -> transferring"));
        assert!(rep.contains("transferring -> failed"));
    }

    #[test]
    fn state_strings_are_stable() {
        assert_eq!(FsmState::Idle.as_str(), "idle");
        assert_eq!(FsmState::Transferring.as_str(), "transferring");
        assert_eq!(FsmState::Paused.as_str(), "paused");
        assert_eq!(FsmState::Completed.as_str(), "completed");
        assert_eq!(FsmState::Failed.as_str(), "failed");
    }
}
