//! Audit trail for transfer actions: timestamped entries with actor,
//! action, and detail; filtering, JSON export, and a summary. (Hilariously
//! over-engineered: a full audit layer for a light link.)

use std::collections::VecDeque;

/// One audited action.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub ts_ms: u64,
    pub actor: &'static str,
    pub action: &'static str,
    pub detail: String,
}

/// A bounded, filterable audit log.
#[derive(Default)]
pub struct AuditLog {
    entries: VecDeque<AuditEntry>,
    cap: usize,
}

impl AuditLog {
    pub fn new(cap: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            cap,
        }
    }

    /// Record an action; the ring caps at `cap` entries.
    pub fn record(&mut self, ts_ms: u64, actor: &'static str, action: &'static str, detail: &str) {
        self.entries.push_back(AuditEntry {
            ts_ms,
            actor,
            action,
            detail: detail.to_string(),
        });
        if self.entries.len() > self.cap {
            self.entries.pop_front();
        }
    }

    pub fn last(&self) -> Option<&AuditEntry> {
        self.entries.back()
    }

    pub fn filter_by_action(&self, action: &str) -> Vec<AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.action == action)
            .cloned()
            .collect()
    }

    pub fn filter_by_actor(&self, actor: &str) -> Vec<AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.actor == actor)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// JSON-ish export of all entries.
    pub fn to_json(&self) -> String {
        let items: Vec<String> = self
            .entries
            .iter()
            .map(|e| {
                format!(
                    "{{\"ts_ms\": {}, \"actor\": \"{}\", \"action\": \"{}\", \"detail\": \"{}\"}}",
                    e.ts_ms, e.actor, e.action, e.detail
                )
            })
            .collect();
        format!("[\n{}\n]", items.join(",\n"))
    }

    /// A one-line summary counting actions ("5 events: 3 transfer.start, 2 transfer.complete").
    pub fn summary(&self) -> String {
        use std::collections::BTreeMap;
        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        for e in self.entries.iter() {
            *counts.entry(e.action).or_insert(0) += 1;
        }
        if counts.is_empty() {
            "no audited actions".to_string()
        } else {
            let parts: Vec<String> = counts.iter().map(|(a, n)| format!("{}x{}", n, a)).collect();
            format!("{} events: {}", self.len(), parts.join(", "))
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> AuditLog {
        AuditLog::new(5)
    }

    #[test]
    fn record_and_last() {
        let mut l = log();
        l.record(1, "tx", "transfer.start", "session 7");
        l.record(2, "rx", "transfer.start", "session 8");
        assert_eq!(l.last().unwrap().action, "transfer.start");
        assert_eq!(l.len(), 2);
        l.clear();
        assert!(l.is_empty());
    }

    #[test]
    fn ring_caps_at_cap() {
        let mut l = log(); // cap 5
        for i in 0..7 {
            l.record(i as u64, "ui", "tick", &format!("n{}", i));
        }
        assert_eq!(l.len(), 5);
        assert_eq!(l.last().unwrap().detail, "n6");
    }

    #[test]
    fn filter_by_action_and_actor() {
        let mut l = AuditLog::new(100);
        l.record(1, "tx", "transfer.start", "s7");
        l.record(2, "rx", "transfer.start", "s8");
        l.record(3, "tx", "transfer.complete", "s7 done");
        l.record(4, "rx", "chunk.received", "chunk 3");
        let starts = l.filter_by_action("transfer.start");
        assert_eq!(starts.len(), 2);
        let tx = l.filter_by_actor("tx");
        assert_eq!(tx.len(), 2);
        let chunks = l.filter_by_action("chunk.received");
        assert_eq!(chunks[0].actor, "rx");
        assert_eq!(l.filter_by_action("nope").len(), 0);
    }

    #[test]
    fn json_export_shape() {
        let mut l = AuditLog::new(10);
        l.record(42, "tx", "transfer.start", "session 7");
        let j = l.to_json();
        assert!(j.starts_with("["));
        assert!(j.contains("\"ts_ms\": 42"));
        assert!(j.contains("\"action\": \"transfer.start\""));
        assert!(j.contains("\"detail\": \"session 7\""));
    }

    #[test]
    fn summary_counts_actions() {
        let mut l = AuditLog::new(100);
        l.record(1, "tx", "transfer.start", "a");
        l.record(2, "tx", "transfer.start", "b");
        l.record(3, "tx", "transfer.complete", "c");
        l.record(4, "rx", "chunk.received", "d");
        let s = l.summary();
        assert!(s.contains("4 events"));
        assert!(s.contains("2xtransfer.start"));
        assert!(s.contains("1xtransfer.complete"));
        assert!(s.contains("1xchunk.received"));
        let empty = AuditLog::new(10);
        assert_eq!(empty.summary(), "no audited actions");
    }
}
