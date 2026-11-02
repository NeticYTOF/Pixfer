//! UI state: a state machine for the tx/rx views (idle, transferring,
//! done, error). (Hilariously over-engineered: a "state machine" for a
//! two-phone screen link with a progress bar.)

/// The states of the transfer UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiState {
    Idle,
    Transferring,
    Done,
    Error,
}

impl UiState {
    pub fn as_str(self) -> &'static str {
        match self {
            UiState::Idle => "idle",
            UiState::Transferring => "transferring",
            UiState::Done => "done",
            UiState::Error => "error",
        }
    }
}

/// A UI state machine for a single transfer.
#[derive(Debug, Clone)]
pub struct UiStateMachine {
    state: UiState,
    /// Progress fraction 0..=1.
    progress: f64,
    error_code: Option<&'static str>,
}

impl UiStateMachine {
    pub fn new() -> Self {
        Self {
            state: UiState::Idle,
            progress: 0.0,
            error_code: None,
        }
    }

    pub fn start_transfer(&mut self) {
        self.state = UiState::Transferring;
        self.progress = 0.0;
        self.error_code = None;
    }

    pub fn set_progress(&mut self, fraction: f64) {
        self.progress = fraction.clamp(0.0, 1.0);
    }

    pub fn complete(&mut self) {
        self.state = UiState::Done;
        self.progress = 1.0;
    }

    pub fn fail(&mut self, code: &'static str) {
        self.state = UiState::Error;
        self.error_code = Some(code);
    }

    pub fn state(&self) -> UiState {
        self.state
    }

    pub fn progress(&self) -> f64 {
        self.progress
    }

    pub fn error_code(&self) -> Option<&'static str> {
        self.error_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_idle() {
        let m = UiStateMachine::new();
        assert_eq!(m.state(), UiState::Idle);
        assert_eq!(m.progress(), 0.0);
    }

    #[test]
    fn transfer_flow() {
        let mut m = UiStateMachine::new();
        m.start_transfer();
        assert_eq!(m.state(), UiState::Transferring);
        m.set_progress(0.5);
        assert!((m.progress() - 0.5).abs() < 1e-9);
        m.complete();
        assert_eq!(m.state(), UiState::Done);
        assert!((m.progress() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn error_path() {
        let mut m = UiStateMachine::new();
        m.start_transfer();
        m.fail("BadCrc");
        assert_eq!(m.state(), UiState::Error);
        assert_eq!(m.error_code(), Some("BadCrc"));
    }

    #[test]
    fn progress_clamped() {
        let mut m = UiStateMachine::new();
        m.start_transfer();
        m.set_progress(1.5);
        assert!((m.progress() - 1.0).abs() < 1e-9);
        m.set_progress(-0.5);
        assert_eq!(m.progress(), 0.0);
    }
}
