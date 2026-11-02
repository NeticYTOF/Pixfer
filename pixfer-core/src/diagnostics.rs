//! Link diagnostics: a `Diagnostics` collector with severity-filtered
//! reporting, plus a `LinkHealthProbe` that flags throughput drops and
//! ACK timeouts. (Hilariously over-engineered: a full diagnostics layer
//! for a light link.)

use std::collections::{BTreeMap, VecDeque};

use crate::errors::Severity;

/// A single diagnostic finding.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub component: &'static str,
    pub message: String,
    pub ts_ms: u64,
}

impl Diagnostic {
    pub fn new(
        code: &'static str,
        severity: Severity,
        component: &'static str,
        message: &str,
        ts_ms: u64,
    ) -> Self {
        Self {
            code,
            severity,
            component,
            message: message.to_string(),
            ts_ms,
        }
    }
}

/// The diagnostics collector: record, filter, report.
#[derive(Default)]
pub struct Diagnostics {
    findings: VecDeque<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, d: Diagnostic) {
        self.findings.push_back(d);
        // Cap the ring at 1000 findings.
        if self.findings.len() > 1000 {
            self.findings.pop_front();
        }
    }

    pub fn count(&self) -> usize {
        self.findings.len()
    }

    /// Findings at or above a minimum severity.
    pub fn at_least(&self, min: Severity) -> Vec<Diagnostic> {
        self.findings
            .iter()
            .filter(|d| d.severity.rank() >= min.rank())
            .cloned()
            .collect()
    }

    /// Findings grouped by component.
    pub fn by_component(&self) -> BTreeMap<&'static str, Vec<Diagnostic>> {
        let mut map: BTreeMap<&'static str, Vec<Diagnostic>> = BTreeMap::new();
        for d in self.findings.iter() {
            map.entry(d.component).or_default().push(d.clone());
        }
        map
    }

    /// JSON-ish report of all findings.
    pub fn report(&self) -> String {
        let items: Vec<String> = self
            .findings
            .iter()
            .map(|d| {
                format!(
                    "{{\"code\": \"{}\", \"severity\": {}, \"component\": \"{}\", \"message\": \"{}\", \"ts_ms\": {}}}",
                    d.code,
                    d.severity.as_str(),
                    d.component,
                    d.message,
                    d.ts_ms
                )
            })
            .collect();
        format!("[\n{}\n]", items.join(",\n"))
    }

    /// A one-line summary ("3 findings: 1 error, 2 warnings").
    pub fn summary(&self) -> String {
        let mut counts: BTreeMap<Severity, usize> = BTreeMap::new();
        for d in self.findings.iter() {
            *counts.entry(d.severity).or_insert(0) += 1;
        }
        let parts: Vec<String> = counts
            .iter()
            .map(|(sev, n)| format!("{} {}", sev.as_str().to_lowercase(), n))
            .collect();
        if parts.is_empty() {
            "no findings".to_string()
        } else {
            format!("{} findings: {}", self.count(), parts.join(", "))
        }
    }

    pub fn clear(&mut self) {
        self.findings.clear();
    }
}

/// A probe that checks link health and records diagnostics into its own
/// collector.
pub struct LinkHealthProbe {
    diags: Diagnostics,
    /// The configured minimum acceptable throughput (MB/s).
    throughput_floor_mbps: f64,
    /// The configured ACK timeout (ms).
    ack_timeout_ms: u64,
}

impl LinkHealthProbe {
    pub fn new(throughput_floor_mbps: f64, ack_timeout_ms: u64) -> Self {
        Self {
            diags: Diagnostics::new(),
            throughput_floor_mbps,
            ack_timeout_ms,
        }
    }

    /// Access the probe's diagnostic collector.
    pub fn diags(&self) -> &Diagnostics {
        &self.diags
    }

    /// Check a measured MB/s against the floor.
    pub fn check_throughput(&mut self, measured_mbps: f64, ts_ms: u64) -> bool {
        if measured_mbps < self.throughput_floor_mbps {
            self.diags.record(Diagnostic::new(
                "LOW_THROUGHPUT",
                Severity::Warn,
                "link",
                &format!(
                    "throughput {measured_mbps:.2} MB/s below floor {floor:.2}",
                    floor = self.throughput_floor_mbps
                ),
                ts_ms,
            ));
            false
        } else {
            true
        }
    }

    /// Check an observed ACK latency against the timeout budget.
    pub fn check_ack_latency(&mut self, latency_ms: u64, ts_ms: u64) {
        if latency_ms > self.ack_timeout_ms {
            self.diags.record(Diagnostic::new(
                "ACK_TIMEOUT",
                Severity::Error,
                "arq",
                &format!(
                    "ack latency {latency_ms}ms exceeds budget {budget}ms",
                    budget = self.ack_timeout_ms
                ),
                ts_ms,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diags() -> Diagnostics {
        Diagnostics::new()
    }

    #[test]
    fn record_and_count() {
        let mut d = diags();
        d.record(Diagnostic::new(
            "E1",
            Severity::Error,
            "rs",
            "too many errors",
            100,
        ));
        d.record(Diagnostic::new(
            "W1",
            Severity::Warn,
            "camera",
            "low light",
            200,
        ));
        assert_eq!(d.count(), 2);
        d.clear();
        assert_eq!(d.count(), 0);
    }

    #[test]
    fn severity_filter() {
        let mut d = diags();
        d.record(Diagnostic::new("E1", Severity::Error, "rs", "bad", 1));
        d.record(Diagnostic::new("W1", Severity::Warn, "camera", "dim", 2));
        d.record(Diagnostic::new("I1", Severity::Info, "ui", "note", 3));
        let warned = d.at_least(Severity::Warn);
        assert_eq!(warned.len(), 2);
        assert!(warned
            .iter()
            .all(|x| x.severity.rank() >= Severity::Warn.rank()));
        let errored = d.at_least(Severity::Error);
        assert_eq!(errored.len(), 1);
        assert_eq!(errored[0].code, "E1");
    }

    #[test]
    fn by_component_groups() {
        let mut d = diags();
        d.record(Diagnostic::new("A", Severity::Error, "camera", "x", 1));
        d.record(Diagnostic::new("B", Severity::Error, "camera", "y", 2));
        d.record(Diagnostic::new("C", Severity::Error, "display", "z", 3));
        let by = d.by_component();
        assert_eq!(by.get("camera").unwrap().len(), 2);
        assert_eq!(by.get("display").unwrap().len(), 1);
    }

    #[test]
    fn report_and_summary_shape() {
        let mut d = diags();
        d.record(Diagnostic::new("E1", Severity::Error, "rs", "bad", 1));
        d.record(Diagnostic::new("W1", Severity::Warn, "camera", "dim", 2));
        let r = d.report();
        assert!(r.starts_with("["));
        assert!(r.contains("\"E1\""));
        assert!(r.contains("\"W1\""));
        let s = d.summary();
        assert!(s.contains("2 findings"));
        assert!(s.contains("error 1"));
        assert!(s.contains("warn 1"));
    }

    #[test]
    fn probe_flags_low_throughput() {
        let mut probe = LinkHealthProbe::new(1.0, 2500);
        assert!(probe.check_throughput(2.5, 100));
        assert!(!probe.check_throughput(0.4, 200));
        assert_eq!(probe.diags().count(), 1);
        assert_eq!(
            probe.diags().at_least(Severity::Warn)[0].code,
            "LOW_THROUGHPUT"
        );
    }

    #[test]
    fn probe_flags_ack_latency() {
        let mut probe = LinkHealthProbe::new(1.0, 2500);
        probe.check_ack_latency(1200, 100); // within budget
        assert_eq!(probe.diags().count(), 0);
        probe.check_ack_latency(3000, 200); // over budget
        assert_eq!(probe.diags().count(), 1);
        assert_eq!(
            probe.diags().at_least(Severity::Error)[0].code,
            "ACK_TIMEOUT"
        );
    }

    #[test]
    fn ring_caps_at_1000() {
        let mut d = diags();
        for i in 0..1100 {
            d.record(Diagnostic::new(
                "X",
                Severity::Info,
                "filler",
                "n",
                i as u64,
            ));
        }
        assert_eq!(d.count(), 1000);
    }
}
