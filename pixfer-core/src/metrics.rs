//! Over-engineered metrics: counters, gauges, histograms with quantiles,
//! a `ThroughputMeter` for MB/s, and a `Metrics` facade with `report()`.

use std::collections::BTreeMap;

/// Monotonic counter (the most basic metric).
#[derive(Debug, Default, Clone)]
pub struct Counter {
    value: u64,
}

impl Counter {
    pub fn add(&mut self, n: u64) {
        self.value += n;
    }
    pub fn value(&self) -> u64 {
        self.value
    }
    pub fn reset(&mut self) {
        self.value = 0;
    }
}

/// Instantaneous gauge.
#[derive(Debug, Default, Clone)]
pub struct Gauge {
    value: f64,
}

impl Gauge {
    pub fn set(&mut self, v: f64) {
        self.value = v;
    }
    pub fn get(&self) -> f64 {
        self.value
    }
}

/// Fixed-bucket histogram with quantile estimates (p50/p90/p95/p99).
#[derive(Debug, Clone)]
pub struct Histogram {
    buckets: Vec<u64>,
    edges: Vec<f64>,
    count: u64,
    sum: f64,
}

impl Histogram {
    pub fn new(edges: Vec<f64>) -> Self {
        assert!(!edges.is_empty(), "histogram needs at least one edge");
        Self {
            buckets: vec![0; edges.len()],
            edges: edges,
            count: 0,
            sum: 0.0,
        }
    }

    pub fn record(&mut self, v: f64) {
        self.count += 1;
        self.sum += v;
        // Find the first edge >= v (edges are sorted ascending).
        let idx = self.edges.iter().position(|&e| v <= e);
        if let Some(i) = idx {
            self.buckets[i] += 1;
        } else {
            // Above the max edge -> count in the last bucket as overflow.
            let last = self.buckets.len() - 1;
            self.buckets[last] += 1;
        }
    }

    /// Quantile estimate by walking the CDF.
    pub fn quantile(&self, q: f64) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let target = (self.count as f64) * q;
        let mut cdf = 0.0;
        for (i, &e) in self.edges.iter().enumerate() {
            cdf += self.buckets[i] as f64;
            if cdf >= target {
                return e;
            }
        }
        *self.edges.last().unwrap()
    }

    pub fn p50(&self) -> f64 {
        self.quantile(0.50)
    }
    pub fn p95(&self) -> f64 {
        self.quantile(0.95)
    }
    pub fn p99(&self) -> f64 {
        self.quantile(0.99)
    }
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }
    pub fn count(&self) -> u64 {
        self.count
    }
}

/// Sliding-window throughput meter reporting MB/s.
#[derive(Debug, Default)]
pub struct ThroughputMeter {
    bytes: f64,
    t0: Option<f64>,
}

impl ThroughputMeter {
    /// Start a timed window.
    pub fn start(&mut self, now_ms: u64) {
        self.bytes = 0.0;
        self.t0 = Some(now_ms as f64);
    }

    pub fn add_bytes(&mut self, n: u64) {
        self.bytes += n as f64;
    }

    /// MB/s over the elapsed window.
    pub fn mbps(&self, now_ms: u64) -> f64 {
        let t0 = match self.t0 {
            Some(t0) => t0,
            None => return 0.0,
        };
        let elapsed_s = (now_ms as f64 - t0) / 1000.0;
        if elapsed_s <= 0.0 {
            return 0.0;
        }
        (self.bytes / elapsed_s) / 1_048_576.0
    }
}

/// The metrics facade: named counters/gauges/histograms + a text report.
#[derive(Debug, Default)]
pub struct Metrics {
    counters: BTreeMap<String, Counter>,
    gauges: BTreeMap<String, Gauge>,
    pub throughput: ThroughputMeter,
}

impl Metrics {
    pub fn inc(&mut self, name: &str) {
        self.counters.entry(name.to_string()).or_default().add(1);
    }
    pub fn add(&mut self, name: &str, n: u64) {
        self.counters.entry(name.to_string()).or_default().add(n);
    }
    pub fn gauge_set(&mut self, name: &str, v: f64) {
        self.gauges.entry(name.to_string()).or_default().set(v);
    }
    pub fn gauge_get(&self, name: &str) -> f64 {
        self.gauges.get(name).map(|g| g.get()).unwrap_or(0.0)
    }
    pub fn counter(&self, name: &str) -> u64 {
        self.counters.get(name).map(|c| c.value()).unwrap_or(0)
    }

    /// A flat, greppable report block.
    pub fn report(&self) -> String {
        let mut out = String::new();
        for (k, c) in &self.counters {
            out.push_str(&format!("counter.{k} = {}\n", c.value()));
        }
        for (k, g) in &self.gauges {
            out.push_str(&format!("gauge.{k} = {:.3}\n", g.get()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_and_gauge() {
        let mut m = Metrics::default();
        m.inc("chunks_sent");
        m.inc("chunks_sent");
        m.add("bytes_sent", 4096);
        m.gauge_set("throughput_mbps", 2.5);
        assert_eq!(m.counter("chunks_sent"), 2);
        assert_eq!(m.counter("bytes_sent"), 4096);
        assert_eq!(m.gauge_get("throughput_mbps"), 2.5);
        let rep = m.report();
        assert!(rep.contains("counter.chunks_sent = 2"));
        assert!(rep.contains("gauge.throughput_mbps"));
    }

    #[test]
    fn histogram_quantiles() {
        let mut h = Histogram::new(vec![1.0, 5.0, 10.0, 20.0, 50.0]);
        for v in [0.5, 2.0, 4.0, 7.0, 12.0, 30.0] {
            h.record(v);
        }
        assert_eq!(h.count(), 6);
        assert!(h.p50() >= 2.0 && h.p50() <= 10.0);
        assert!(h.p99() <= 50.0);
        assert!((h.mean() - (0.5 + 2.0 + 4.0 + 7.0 + 12.0 + 30.0) / 6.0).abs() < 1e-9);
    }

    #[test]
    fn throughput_meter() {
        let mut m = Metrics::default();
        m.throughput.start(0);
        m.throughput.add_bytes(2_000_000);
        // 2 MB over 1 s (1000 ms)
        let mbps = m.throughput.mbps(1000);
        assert!((mbps - 1.907).abs() < 0.01); // 2e6/1048576
    }
}
