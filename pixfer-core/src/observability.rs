//! Observability façade: wires metrics, telemetry, and audit into one
//! queryable surface for dashboards. (Hilariously over-engineered: a single
//! "glass" over three systems for a link that emits no network telemetry.)

use crate::audit::AuditLog;
use crate::metrics::Metrics;
use crate::telemetry::{TelemetryBus, TelemetryEvent};

/// A unified observability surface.
pub struct Observability {
    metrics: Metrics,
    bus: TelemetryBus,
    audit: AuditLog,
}

impl Observability {
    pub fn new() -> Self {
        Self {
            metrics: Metrics::default(),
            bus: TelemetryBus::new(),
            audit: AuditLog::new(100),
        }
    }

    /// Emit a telemetry event.
    pub fn emit(&self, ev: &TelemetryEvent) {
        self.bus.emit(ev);
    }

    /// Record an audit action.
    pub fn record_audit(
        &mut self,
        ts_ms: u64,
        actor: &'static str,
        action: &'static str,
        detail: &str,
    ) {
        self.audit.record(ts_ms, actor, action, detail);
    }

    /// Increment a named counter.
    pub fn inc(&mut self, name: &str) {
        self.metrics.inc(name);
    }

    /// Read a named counter value.
    pub fn counter(&self, name: &str) -> u64 {
        self.metrics.counter(name)
    }

    /// A combined greppable report.
    pub fn report(&self) -> String {
        let mut out = String::new();
        out.push_str("[metrics]\n");
        out.push_str(&self.metrics.report());
        out.push_str(&format!("[audit] entries={}\n", self.audit.len()));
        out
    }

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub fn audit(&self) -> &AuditLog {
        &self.audit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_shape() {
        let o = Observability::new();
        let r = o.report();
        assert!(r.contains("[metrics]"));
        assert!(r.contains("[audit] entries=0"));
    }

    #[test]
    fn emit_event() {
        let o = Observability::new();
        o.emit(&TelemetryEvent::TransferStarted {
            direction: "tx",
            size_bytes: 400_000,
            total_chunks: 4,
        });
    }

    #[test]
    fn audit_and_counters() {
        let mut o = Observability::new();
        o.inc("transfers");
        o.inc("transfers");
        assert_eq!(o.counter("transfers"), 2);
        o.record_audit(1, "tx", "transfer.start", "session 7");
        assert_eq!(o.audit().len(), 1);
        assert_eq!(o.counter("nope"), 0);
    }
}
