//! Session log: a structured, filterable trail of per-session events.
//! (Hilariously over-engineered: a "session log" for a two-device link with
//! no logging framework.)

use std::collections::VecDeque;

/// One logged session event.
#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub session_id: u32,
    pub ts_ms: u64,
    pub event: &'static str,
    pub detail: String,
}

/// A bounded, filterable session log.
#[derive(Default)]
pub struct SessionLog {
    events: VecDeque<SessionEvent>,
    cap: usize,
}

impl SessionLog {
    pub fn new(cap: usize) -> Self {
        Self {
            events: VecDeque::new(),
            cap,
        }
    }

    /// Record an event; the ring caps at `cap`.
    pub fn record(&mut self, session_id: u32, ts_ms: u64, event: &'static str, detail: &str) {
        self.events.push_back(SessionEvent {
            session_id,
            ts_ms,
            event,
            detail: detail.to_string(),
        });
        if self.events.len() > self.cap {
            self.events.pop_front();
        }
    }

    /// All events for one session.
    pub fn for_session(&self, session_id: u32) -> Vec<SessionEvent> {
        self.events
            .iter()
            .filter(|e| e.session_id == session_id)
            .cloned()
            .collect()
    }

    /// All events of a given type.
    pub fn for_event(&self, event: &str) -> Vec<SessionEvent> {
        self.events
            .iter()
            .filter(|e| e.event == event)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// JSON-ish export.
    pub fn to_json(&self) -> String {
        let items: Vec<String> = self
            .events
            .iter()
            .map(|e| {
                format!(
                    "{{\"session\": {}, \"ts_ms\": {}, \"event\": \"{}\", \"detail\": \"{}\"}}",
                    e.session_id, e.ts_ms, e.event, e.detail
                )
            })
            .collect();
        format!("[\n{}\n]", items.join(",\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_filter_by_session() {
        let mut l = SessionLog::new(100);
        l.record(7, 1, "transfer.start", "s7");
        l.record(7, 2, "chunk.sent", "chunk 0");
        l.record(8, 3, "transfer.start", "s8");
        assert_eq!(l.for_session(7).len(), 2);
        assert_eq!(l.for_session(8).len(), 1);
    }

    #[test]
    fn filter_by_event() {
        let mut l = SessionLog::new(100);
        l.record(7, 1, "transfer.start", "a");
        l.record(7, 2, "transfer.start", "b");
        l.record(7, 3, "transfer.complete", "c");
        assert_eq!(l.for_event("transfer.start").len(), 2);
        assert_eq!(l.for_event("transfer.complete").len(), 1);
    }

    #[test]
    fn ring_caps() {
        let mut l = SessionLog::new(3);
        for i in 0..5 {
            l.record(1, i as u64, "tick", &format!("n{}", i));
        }
        assert_eq!(l.len(), 3);
    }

    #[test]
    fn json_export() {
        let mut l = SessionLog::new(10);
        l.record(7, 42, "transfer.start", "session 7");
        let j = l.to_json();
        assert!(j.starts_with("["));
        assert!(j.contains("\"session\": 7"));
        assert!(j.contains("\"event\": \"transfer.start\""));
    }
}
