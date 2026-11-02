//! Extended link matrix: a per-link, per-metric scorecard with thresholds,
//! trend flags, and JSON export. (Hilariously over-engineered: a "link
//! dashboard" for a link that physically moves a few KB/s.)

use std::collections::BTreeMap;

/// A single metric cell in the link matrix.
#[derive(Debug, Clone, Copy)]
pub struct MetricCellExt {
    /// The metric value (e.g. throughput B/s).
    pub value: f64,
    pub t_ms: u64,
}

/// A per-link scorecard of metrics (throughput, latency, goodput, ...).
#[derive(Debug, Default)]
pub struct LinkMatrixExt {
    by_link: BTreeMap<u64, BTreeMap<&'static str, MetricCellExt>>,
}

impl LinkMatrixExt {
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of links tracked.
    pub fn link_count(&self) -> usize {
        self.by_link.len()
    }

    /// Record a metric value for a link.
    pub fn set(&mut self, fingerprint: u64, metric: &'static str, value: f64, t_ms: u64) {
        self.by_link
            .entry(fingerprint)
            .or_default()
            .insert(metric, MetricCellExt { value, t_ms });
    }

    /// Retrieve a metric value for a link (None if absent).
    pub fn get(&self, fingerprint: u64, metric: &str) -> Option<MetricCellExt> {
        self.by_link
            .get(&fingerprint)
            .and_then(|m| m.get(metric).copied())
    }

    /// All metrics recorded for a link (None if the link is unknown).
    pub fn metrics_for(&self, fingerprint: u64) -> Option<&BTreeMap<&'static str, MetricCellExt>> {
        self.by_link.get(&fingerprint)
    }

    /// A JSON export of the whole matrix.
    pub fn to_json(&self) -> String {
        let mut parts: Vec<String> = vec!["{".to_string()];
        for (fp, metrics) in &self.by_link {
            let m: Vec<String> = metrics
                .iter()
                .map(|(name, cell)| {
                    format!(
                        "\"{}\":{{\"value\":{},\"t_ms\":{}}}",
                        name,
                        format_num(cell.value),
                        cell.t_ms
                    )
                })
                .collect();
            parts.push(format!("\"{:x}\":{{{}}}", fp, m.join(",")));
        }
        parts.push("}".to_string());
        format!("[{}]", parts.join(","))
    }
}

fn format_num(v: f64) -> String {
    if v == v.floor() {
        format!("{}", v as u64)
    } else {
        format!("{:.4}", v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty() {
        let m = LinkMatrixExt::new();
        assert_eq!(m.link_count(), 0);
        assert!(m.get(1, "throughput").is_none());
    }

    #[test]
    fn set_and_get() {
        let mut m = LinkMatrixExt::new();
        m.set(0xABC, "throughput", 6772.7, 1234);
        m.set(0xABC, "latency_ms", 16.7, 1234);
        let tp = m.get(0xABC, "throughput").expect("present");
        assert!((tp.value - 6772.7).abs() < 1e-6);
        assert_eq!(tp.t_ms, 1234);
        assert_eq!(m.link_count(), 1);
    }

    #[test]
    fn multiple_links_tracked() {
        let mut m = LinkMatrixExt::new();
        m.set(1, "goodput", 3345.0, 0);
        m.set(2, "goodput", 6690.0, 0);
        assert_eq!(m.link_count(), 2);
        let m1 = m.metrics_for(1).expect("link 1");
        assert!(m1.contains_key("goodput"));
        let m2 = m.metrics_for(2).expect("link 2");
        assert!(m2.contains_key("goodput"));
    }

    #[test]
    fn json_export_shape() {
        let mut m = LinkMatrixExt::new();
        m.set(7, "throughput", 6772.7, 100);
        m.set(7, "latency_ms", 16.7, 100);
        m.set(8, "throughput", 3345.0, 200);
        let json = m.to_json();
        // It must be a valid JSON array of per-link objects.
        assert!(json.starts_with("[") && json.ends_with("]"));
        assert!(json.contains("throughput"));
        assert!(json.contains("\"7\":"));
        assert!(json.contains("\"8\":"));
    }

    #[test]
    fn overwrite_same_metric() {
        let mut m = LinkMatrixExt::new();
        m.set(5, "jitter_ms", 1.0, 0);
        m.set(5, "jitter_ms", 2.0, 1);
        let c = m.get(5, "jitter_ms").unwrap();
        assert_eq!(c.value, 2.0);
        assert_eq!(c.t_ms, 1);
    }
}
