//! A tiny benchmark harness: measure protocol render/decode, compress/
//! decompress, and RS encode/decode throughput. (Hilariously over-engineered:
//! a benchmarking layer for a light link.)

use std::time::Instant;

use crate::compress::{is_zip, unzip_first, zip_bytes};
use crate::protocol::{Frame, FrameKind, Protocol};
use crate::rs::{ReedSolomon, RS_K};

/// The result of a single benchmark run.
#[derive(Debug, Clone)]
pub struct BenchResult {
    pub name: &'static str,
    pub iterations: u32,
    pub elapsed_ms: u128,
    pub bytes: u64,
}

impl BenchResult {
    /// Throughput in MB/s.
    pub fn mbps(&self) -> f64 {
        if self.elapsed_ms == 0 {
            return 0.0;
        }
        (self.bytes as f64) / ((self.elapsed_ms as f64) / 1000.0) / 1_000_000.0
    }

    /// A one-line summary for reports.
    pub fn summary(&self) -> String {
        format!(
            "{}: {} iters in {} ms = {:.2} MB/s",
            self.name,
            self.iterations,
            self.elapsed_ms,
            self.mbps()
        )
    }
}

/// The benchmark runner.
pub struct Benchmark {
    results: Vec<BenchResult>,
}

impl Benchmark {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Time a closure over `iterations` and record the result.
    pub fn time(&mut self, name: &'static str, iterations: u32, bytes: u64, f: &dyn Fn()) {
        let start = Instant::now();
        for _ in 0..iterations {
            f();
        }
        let elapsed = start.elapsed();
        self.results.push(BenchResult {
            name,
            iterations,
            elapsed_ms: elapsed.as_millis(),
            bytes,
        });
    }

    /// Protocol render/decode round-trip throughput.
    pub fn bench_protocol(&mut self, payload: &[u8]) {
        let frame = Frame::new(FrameKind::Chunk, 1, 0, 1, payload.to_vec());
        let blocks = Protocol::render(&frame);
        self.time("protocol.render_decode", 1, payload.len() as u64, &|| {
            let _ = Protocol::decode(&blocks);
        });
    }

    /// Zip + unzip throughput for a payload.
    pub fn bench_compress(&mut self, payload: &[u8]) {
        self.time("compress.zip", 1, payload.len() as u64, &|| {
            let z = zip_bytes(payload, "bench");
            if is_zip(&z) {
                let _ = unzip_first(&z);
            }
        });
    }

    /// RS encode/decode throughput.
    pub fn bench_rs(&mut self) {
        let rs = ReedSolomon::new();
        let payload = vec![0u8; RS_K];
        let coded = rs.encode(&payload);
        self.time("rs.encode_decode", 1, RS_K as u64, &|| {
            let _ = rs.decode(&coded);
        });
    }

    pub fn results(&self) -> &[BenchResult] {
        &self.results
    }

    /// JSON-ish report of all results.
    pub fn report(&self) -> String {
        let items: Vec<String> =
            self.results
                .iter()
                .map(|r| {
                    format!(
                    "{{\"name\": \"{}\", \"iterations\": {}, \"elapsed_ms\": {}, \"mbps\": {:.2}}}",
                    r.name, r.iterations, r.elapsed_ms, r.mbps()
                )
                })
                .collect();
        format!("[\n{}\n]", items.join(",\n"))
    }

    /// A human-readable multi-line report.
    pub fn summary(&self) -> String {
        self.results
            .iter()
            .map(|r| r.summary())
            .collect::<Vec<String>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_mbps_math() {
        let r = BenchResult {
            name: "x",
            iterations: 100,
            elapsed_ms: 2000,
            bytes: 2_000_000,
        };
        assert!((r.mbps() - 1.0).abs() < 1e-9);
        let zero = BenchResult {
            name: "y",
            iterations: 0,
            elapsed_ms: 0,
            bytes: 0,
        };
        assert_eq!(zero.mbps(), 0.0);
    }

    #[test]
    fn time_records_result() {
        let mut b = Benchmark::new();
        let counter = std::sync::atomic::AtomicI32::new(0);
        b.time("counter", 1000, 1000, &|| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        assert_eq!(b.results().len(), 1);
        assert_eq!(b.results()[0].iterations, 1000);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1000);
    }

    #[test]
    fn protocol_bench_runs() {
        let mut b = Benchmark::new();
        let payload = vec![0xAB; 223];
        b.bench_protocol(&payload);
        assert_eq!(b.results().len(), 1);
        assert_eq!(b.results()[0].name, "protocol.render_decode");
        let rep = b.report();
        assert!(rep.contains("\"protocol.render_decode\""));
        let sum = b.summary();
        assert!(sum.contains("MB/s"));
    }

    #[test]
    fn compress_bench_runs() {
        let mut b = Benchmark::new();
        let payload: Vec<u8> = (0..50_000).map(|i| i as u8).collect();
        b.bench_compress(&payload);
        assert_eq!(b.results()[0].name, "compress.zip");
    }

    #[test]
    fn rs_bench_runs() {
        let mut b = Benchmark::new();
        b.bench_rs();
        assert_eq!(b.results()[0].name, "rs.encode_decode");
        assert_eq!(b.results()[0].bytes, RS_K as u64);
    }

    #[test]
    fn report_shape() {
        let mut b = Benchmark::new();
        b.time("a", 10, 1000, &|| {});
        let rep = b.report();
        assert!(rep.starts_with("["));
        assert!(rep.contains("\"name\": \"a\""));
        assert!(rep.contains("\"iterations\": 10"));
    }
}
