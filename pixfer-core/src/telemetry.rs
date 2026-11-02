//! Telemetry: structured events with a small in-memory bus that the UI and
//! metrics layers subscribe to. Deliberately over-engineered: typed events,
//! a subscriber registry, and JSON-ish serialization.

use std::collections::HashMap;
use std::sync::Mutex;

/// A telemetry event type.
#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryEvent {
    TransferStarted {
        direction: &'static str,
        size_bytes: u64,
        total_chunks: u32,
    },
    ChunkSent {
        idx: u32,
        t_ms: u64,
    },
    ChunkAcked {
        idx: u32,
        latency_ms: u64,
    },
    ChunkReceived {
        idx: u32,
    },
    RetryScheduled {
        idx: u32,
        attempt: u32,
        timeout_ms: u64,
    },
    TransferCompleted {
        bytes: u64,
        elapsed_ms: u64,
        mbps: f64,
    },
    TransferFailed {
        code: &'static str,
        message: String,
    },
    CalibrationApplied {
        block_px: u32,
        off_x: i32,
        off_y: i32,
    },
    ExposureLocked {
        success: bool,
    },
}

impl TelemetryEvent {
    pub fn name(&self) -> &'static str {
        match self {
            TelemetryEvent::TransferStarted { .. } => "transfer.started",
            TelemetryEvent::ChunkSent { .. } => "chunk.sent",
            TelemetryEvent::ChunkAcked { .. } => "chunk.acked",
            TelemetryEvent::ChunkReceived { .. } => "chunk.received",
            TelemetryEvent::RetryScheduled { .. } => "retry.scheduled",
            TelemetryEvent::TransferCompleted { .. } => "transfer.completed",
            TelemetryEvent::TransferFailed { .. } => "transfer.failed",
            TelemetryEvent::CalibrationApplied { .. } => "calibration.applied",
            TelemetryEvent::ExposureLocked { .. } => "exposure.locked",
        }
    }
}

type Subscriber = Box<dyn Fn(&TelemetryEvent) + Send>;

/// An in-memory pub/sub bus.
pub struct TelemetryBus {
    subscribers: Mutex<Vec<Subscriber>>,
}

impl TelemetryBus {
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }
    pub fn subscribe(&self, sub: Subscriber) {
        self.subscribers.lock().unwrap().push(sub);
    }
    pub fn emit(&self, ev: &TelemetryEvent) {
        let subs = self.subscribers.lock().unwrap();
        for sub in subs.iter() {
            sub(ev);
        }
    }
}

impl Default for TelemetryBus {
    fn default() -> Self {
        Self::new()
    }
}

/// A metrics-style recorder that counts events by name.
#[derive(Default)]
pub struct TelemetryRecorder {
    pub counts: HashMap<&'static str, u64>,
    pub last: Option<TelemetryEvent>,
}

impl TelemetryRecorder {
    pub fn record(&mut self, ev: &TelemetryEvent) {
        *self.counts.entry(ev.name()).or_insert(0) += 1;
        self.last = Some(ev.clone());
    }

    pub fn as_subscriber(&self) -> Subscriber {
        // Can't easily move self into a 'static closure; callers wrap.
        Box::new(|_ev: &TelemetryEvent| {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names_are_stable() {
        assert_eq!(
            TelemetryEvent::TransferStarted {
                direction: "tx",
                size_bytes: 1024,
                total_chunks: 5,
            }
            .name(),
            "transfer.started"
        );
        assert_eq!(
            TelemetryEvent::TransferFailed {
                code: "TIMEOUT",
                message: "x".into(),
            }
            .name(),
            "transfer.failed"
        );
    }

    #[test]
    fn bus_fans_out_to_subscribers() {
        let bus = TelemetryBus::new();
        let hits = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let hits2 = hits.clone();
        bus.subscribe(Box::new(move |ev| {
            hits2.lock().unwrap().push(ev.name());
        }));
        bus.emit(&TelemetryEvent::ChunkSent { idx: 3, t_ms: 42 });
        bus.emit(&TelemetryEvent::TransferCompleted {
            bytes: 2_000_000,
            elapsed_ms: 1000,
            mbps: 1.9,
        });
        assert_eq!(
            hits.lock().unwrap().as_slice(),
            &["chunk.sent", "transfer.completed"]
        );
    }

    #[test]
    fn recorder_counts() {
        let mut rec = TelemetryRecorder::default();
        rec.record(&TelemetryEvent::ChunkAcked {
            idx: 0,
            latency_ms: 88,
        });
        rec.record(&TelemetryEvent::ChunkAcked {
            idx: 1,
            latency_ms: 91,
        });
        assert_eq!(*rec.counts.get("chunk.acked").unwrap(), 2);
        assert_eq!(rec.last.as_ref().unwrap().name(), "chunk.acked");
    }
}
