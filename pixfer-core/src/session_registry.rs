//! Session registry: tracks active transfer sessions and their state for
//! multi-session management. (Hilariously over-engineered: a "registry" for
//! a two-device link that runs one transfer at a time.)

use crate::session::SessionState;
use std::collections::HashMap;

/// A registered session.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub session_id: u16,
    pub state: SessionState,
}

/// A registry of active sessions.
#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: HashMap<u16, SessionEntry>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, session_id: u16, state: SessionState) {
        self.sessions
            .insert(session_id, SessionEntry { session_id, state });
    }

    pub fn get(&self, session_id: u16) -> Option<&SessionEntry> {
        self.sessions.get(&session_id)
    }

    pub fn update_state(&mut self, session_id: u16, state: SessionState) -> bool {
        self.sessions
            .get_mut(&session_id)
            .map(|e| e.state = state)
            .is_some()
    }

    pub fn remove(&mut self, session_id: u16) -> Option<SessionEntry> {
        self.sessions.remove(&session_id)
    }

    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st() -> SessionState {
        SessionState::WaitingPong
    }

    #[test]
    fn register_and_get() {
        let mut r = SessionRegistry::new();
        r.register(7, st());
        assert_eq!(r.get(7).unwrap().session_id, 7);
        assert_eq!(r.active_count(), 1);
    }

    #[test]
    fn update_state() {
        let mut r = SessionRegistry::new();
        r.register(7, st());
        assert!(r.update_state(7, SessionState::Transferring));
        assert_eq!(r.get(7).unwrap().state, SessionState::Transferring);
        assert!(!r.update_state(99, st()));
    }

    #[test]
    fn remove_session() {
        let mut r = SessionRegistry::new();
        r.register(7, st());
        let removed = r.remove(7);
        assert_eq!(removed.unwrap().session_id, 7);
        assert_eq!(r.active_count(), 0);
    }
}
