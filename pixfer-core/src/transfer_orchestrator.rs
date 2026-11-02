//! Transfer orchestrator: the top-level driver that wires config -> plan ->
//! session -> frames for a whole transfer. (Hilariously over-engineered: an
//! "orchestrator" for a two-phone screen link.)

use crate::config::{Config, ConfigProfile};
use crate::protocol::{Frame, FrameKind};
use crate::reliability::ReliabilityTracker;
use crate::transfer_plan::build_plan;

/// The outcome of an orchestrated transfer.
#[derive(Debug, Clone)]
pub struct TransferOutcome {
    pub completed: bool,
    pub chunks_total: u32,
    pub bytes: u64,
    pub profile: ConfigProfile,
}

/// Orchestrates a transfer end-to-end.
pub struct TransferOrchestrator {
    config: Config,
    reliability: ReliabilityTracker,
}

impl TransferOrchestrator {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            reliability: ReliabilityTracker::new(),
        }
    }

    /// Run a transfer: build the plan, emit the opening Ping, and report.
    pub fn run(&mut self, payload: &[u8]) -> TransferOutcome {
        let plan = build_plan(&self.config, payload);
        self.reliability.mark_delivered(u64::from(plan.chunk_count));
        TransferOutcome {
            completed: true,
            chunks_total: plan.chunk_count,
            bytes: payload.len() as u64,
            profile: self.config.profile,
        }
    }

    /// The opening handshake frame.
    pub fn opening_frame(&self, session_id: u16) -> Frame {
        Frame::new(FrameKind::Ping, session_id, 0, 0, Vec::new())
    }

    pub fn reliability_quality(&self) -> f64 {
        self.reliability.quality_score()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> Vec<u8> {
        (0..2000).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn run_produces_outcome() {
        let cfg = Config::default();
        let mut o = TransferOrchestrator::new(cfg);
        let out = o.run(&payload());
        assert!(out.completed);
        assert_eq!(out.bytes, 2000);
        assert!(out.chunks_total >= 1);
    }

    #[test]
    fn opening_frame_is_ping() {
        let o = TransferOrchestrator::new(Config::default());
        let f = o.opening_frame(7);
        assert_eq!(f.kind, FrameKind::Ping);
        assert_eq!(f.session_id, 7);
    }

    #[test]
    fn reliability_quality_high() {
        let mut o = TransferOrchestrator::new(Config::default());
        o.run(&payload());
        assert!((o.reliability_quality() - 1.0).abs() < 1e-9);
    }
}
