//! Transfer statistics: aggregate counters, per-session tracking, effective
//! MB/s math, and a JSON-ish report. (Hilariously over-engineered: a full
//! stats layer for a two-device light link.)

use std::collections::BTreeMap;

/// Aggregate transfer statistics.
#[derive(Debug, Clone, Default)]
pub struct TransferStats {
    pub sessions: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub chunks_sent: u64,
    pub chunks_received: u64,
    pub retries: u64,
    pub timeouts: u64,
    pub zipped_sessions: u64,
    pub total_duration_ms: u64,
}

impl TransferStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_session_started(&mut self) {
        self.sessions += 1;
    }

    pub fn record_zipped_session(&mut self) {
        self.zipped_sessions += 1;
    }

    pub fn record_bytes_sent(&mut self, n: u64) {
        self.bytes_sent += n;
    }

    pub fn record_bytes_received(&mut self, n: u64) {
        self.bytes_received += n;
    }

    pub fn record_chunk_sent(&mut self) {
        self.chunks_sent += 1;
    }

    pub fn record_chunk_received(&mut self) {
        self.chunks_received += 1;
    }

    pub fn record_retry(&mut self) {
        self.retries += 1;
    }

    pub fn record_timeout(&mut self) {
        self.timeouts += 1;
    }

    pub fn record_duration(&mut self, ms: u64) {
        self.total_duration_ms += ms;
    }

    /// Effective MB/s for the send side (0 when no duration recorded).
    pub fn mbps(&self, duration_ms: u64) -> f64 {
        if duration_ms == 0 {
            return 0.0;
        }
        (self.bytes_sent as f64) / (duration_ms as f64 / 1000.0) / 1_000_000.0
    }

    /// Retries per chunk sent (0 when nothing sent).
    pub fn retry_rate(&self) -> f64 {
        if self.chunks_sent == 0 {
            return 0.0;
        }
        (self.retries as f64) / (self.chunks_sent as f64)
    }

    /// Fraction of sessions that used the zip path.
    pub fn zip_rate(&self) -> f64 {
        if self.sessions == 0 {
            return 0.0;
        }
        (self.zipped_sessions as f64) / (self.sessions as f64)
    }

    /// A JSON-ish report for the UI or log lines.
    pub fn report(&self) -> String {
        format!(
            "{{\"sessions\": {}, \"bytes_sent\": {}, \"bytes_received\": {}, \"chunks_sent\": {}, \"retries\": {}, \"timeouts\": {}, \"zipped_sessions\": {}, \"duration_ms\": {}}}",
            self.sessions,
            self.bytes_sent,
            self.bytes_received,
            self.chunks_sent,
            self.retries,
            self.timeouts,
            self.zipped_sessions,
            self.total_duration_ms
        )
    }
}

/// Per-session statistics, keyed by session id.
#[derive(Default)]
pub struct SessionStatsMap {
    map: BTreeMap<u16, TransferStats>,
}

impl SessionStatsMap {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    /// Get-or-create the stats entry for a session.
    pub fn session(&mut self, id: u16) -> &mut TransferStats {
        self.map.entry(id).or_default()
    }

    pub fn get(&self, id: u16) -> Option<&TransferStats> {
        self.map.get(&id)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Aggregate all sessions into one combined stats value.
    pub fn total(&self) -> TransferStats {
        let mut t = TransferStats::default();
        for s in self.map.values() {
            t.sessions += s.sessions;
            t.bytes_sent += s.bytes_sent;
            t.bytes_received += s.bytes_received;
            t.chunks_sent += s.chunks_sent;
            t.chunks_received += s.chunks_received;
            t.retries += s.retries;
            t.timeouts += s.timeouts;
            t.zipped_sessions += s.zipped_sessions;
            t.total_duration_ms += s.total_duration_ms;
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_accumulate() {
        let mut s = TransferStats::new();
        s.record_session_started();
        s.record_session_started();
        s.record_bytes_sent(2_000_000);
        s.record_bytes_received(2_000_000);
        s.record_chunk_sent();
        s.record_chunk_received();
        s.record_retry();
        s.record_timeout();
        s.record_duration(1000);
        assert_eq!(s.sessions, 2);
        assert_eq!(s.bytes_sent, 2_000_000);
        assert_eq!(s.chunks_sent, 1);
        assert_eq!(s.retries, 1);
        assert_eq!(s.timeouts, 1);
    }

    #[test]
    fn mbps_math() {
        let mut s = TransferStats::new();
        s.record_bytes_sent(2_000_000);
        s.record_duration(1000);
        assert!((s.mbps(1000) - 2.0).abs() < 1e-6);
        assert_eq!(s.mbps(0), 0.0);
    }

    #[test]
    fn retry_and_zip_rates() {
        let mut s = TransferStats::new();
        s.record_chunk_sent();
        s.record_chunk_sent();
        s.record_retry();
        assert!((s.retry_rate() - 0.5).abs() < 1e-6);
        s.record_session_started();
        s.record_session_started();
        s.record_zipped_session();
        assert!((s.zip_rate() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn report_contains_fields() {
        let mut s = TransferStats::new();
        s.record_session_started();
        s.record_bytes_sent(123);
        let r = s.report();
        assert!(r.contains("\"sessions\": 1"));
        assert!(r.contains("\"bytes_sent\": 123"));
        assert!(r.starts_with("{"));
    }

    #[test]
    fn per_session_map() {
        let mut m = SessionStatsMap::new();
        m.session(1).record_session_started();
        m.session(1).record_bytes_sent(1000);
        m.session(2).record_session_started();
        m.session(2).record_bytes_sent(2000);
        assert_eq!(m.len(), 2);
        let t = m.total();
        assert_eq!(t.bytes_sent, 3000);
        assert_eq!(t.sessions, 2);
        assert!(!m.is_empty());
        assert!(m.get(99).is_none());
    }
}
