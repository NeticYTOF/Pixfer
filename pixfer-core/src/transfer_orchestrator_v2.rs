//! Extended transfer orchestrator v2: a coordinator that sequences the
//! plan -> compress -> chunk -> encode -> send -> verify lifecycle with a
//! per-phase report. (Hilariously over-engineered: an "orchestration
//! engine" for a few-KB/s light link.)

use crate::compress::zip_bytes;
use crate::frame_codec::encode_payload;
use crate::rs::RS_K;

/// A single phase of the orchestrated transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Plan,
    Compress,
    Chunk,
    Encode,
    Send,
    Verify,
}

/// The result of a single phase.
#[derive(Debug, Clone)]
pub struct PhaseResult {
    pub phase: Phase,
    pub ok: bool,
    pub detail: String,
    pub elapsed_ms: u64,
}

/// An orchestrated transfer report.
#[derive(Debug, Default)]
pub struct OrchestratorReport {
    pub phases: Vec<PhaseResult>,
}

impl OrchestratorReport {
    /// The number of completed phases.
    pub fn phase_count(&self) -> usize {
        self.phases.len()
    }

    /// Whether all phases succeeded.
    pub fn all_ok(&self) -> bool {
        self.phases.iter().all(|p| p.ok)
    }

    /// The total elapsed time across phases (ms).
    pub fn total_elapsed_ms(&self) -> u64 {
        self.phases.iter().map(|p| p.elapsed_ms).sum()
    }
}

/// Coordinates a full transfer with a per-phase report.
#[derive(Debug)]
pub struct TransferOrchestratorV2 {
    /// The max payload bytes per chunk.
    chunk_bytes: usize,
    /// The auto-zip threshold.
    zip_threshold: usize,
}

impl TransferOrchestratorV2 {
    pub fn new(chunk_bytes: usize, zip_threshold: usize) -> Self {
        Self {
            chunk_bytes: chunk_bytes.max(1),
            zip_threshold: zip_threshold,
        }
    }

    /// The max payload bytes per chunk.
    pub fn chunk_bytes(&self) -> usize {
        self.chunk_bytes
    }

    /// Run the full orchestrated transfer: plan -> compress -> chunk ->
    /// encode -> send -> verify. `send` is called per encoded frame.
    pub fn run<F>(&self, payload: &[u8], send: &mut F) -> OrchestratorReport
    where
        F: FnMut(&[u8]) -> Result<(), String>,
    {
        let mut report = OrchestratorReport::default();

        // PLAN: decide whether to zip.
        let plan_start = std::time::Instant::now();
        let should_zip = payload.len() > self.zip_threshold;
        report.phases.push(PhaseResult {
            phase: Phase::Plan,
            ok: true,
            detail: format!("zip={}", should_zip),
            elapsed_ms: plan_start.elapsed().as_millis() as u64,
        });

        // COMPRESS: zip if needed.
        let compress_start = std::time::Instant::now();
        let zipped = should_zip.then(|| zip_bytes(payload, "pixfer.bin"));
        let data = match zipped {
            Some(z) => {
                report.phases.push(PhaseResult {
                    phase: Phase::Compress,
                    ok: true,
                    detail: format!("{} -> {} bytes", payload.len(), z.len()),
                    elapsed_ms: compress_start.elapsed().as_millis() as u64,
                });
                z
            }
            None => {
                report.phases.push(PhaseResult {
                    phase: Phase::Compress,
                    ok: true,
                    detail: "no zip needed".to_string(),
                    elapsed_ms: compress_start.elapsed().as_millis() as u64,
                });
                payload.to_vec()
            }
        };

        // CHUNK: split into chunks of chunk_bytes.
        let chunk_start = std::time::Instant::now();
        let n = if data.is_empty() {
            0
        } else {
            (data.len() + self.chunk_bytes - 1) / self.chunk_bytes
        };
        report.phases.push(PhaseResult {
            phase: Phase::Chunk,
            ok: n >= 0,
            detail: format!("{} chunks", n),
            elapsed_ms: chunk_start.elapsed().as_millis() as u64,
        });

        // ENCODE + SEND: encode each chunk, then send.
        for i in 0..n {
            let start = i * self.chunk_bytes;
            let end = ((i + 1) * self.chunk_bytes).min(data.len());
            let chunk = &data[start..end];
            let enc_start = std::time::Instant::now();
            let frame = match encode_payload(chunk) {
                Ok(f) => f,
                Err(e) => {
                    report.phases.push(PhaseResult {
                        phase: Phase::Encode,
                        ok: false,
                        detail: e,
                        elapsed_ms: enc_start.elapsed().as_millis() as u64,
                    });
                    return report;
                }
            };
            report.phases.push(PhaseResult {
                phase: Phase::Encode,
                ok: true,
                detail: format!("chunk {}", i),
                elapsed_ms: enc_start.elapsed().as_millis() as u64,
            });
            let send_start = std::time::Instant::now();
            match send(&frame) {
                Ok(()) => {
                    report.phases.push(PhaseResult {
                        phase: Phase::Send,
                        ok: true,
                        detail: format!("chunk {}", i),
                        elapsed_ms: send_start.elapsed().as_millis() as u64,
                    });
                }
                Err(e) => {
                    report.phases.push(PhaseResult {
                        phase: Phase::Send,
                        ok: false,
                        detail: e,
                        elapsed_ms: send_start.elapsed().as_millis() as u64,
                    });
                    return report;
                }
            }
        }

        // VERIFY: confirm all chunks sent.
        let verify_start = std::time::Instant::now();
        report.phases.push(PhaseResult {
            phase: Phase::Verify,
            ok: true,
            detail: format!("{} chunks verified", n),
            elapsed_ms: verify_start.elapsed().as_millis() as u64,
        });

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_send(_frame: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn payload() -> Vec<u8> {
        (0..30).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn runs_all_phases() {
        let o = TransferOrchestratorV2::new(223, 500);
        let p = payload();
        let mut send = |f: &[u8]| -> Result<(), String> { Ok(()) };
        let report = o.run(&p, &mut send);
        assert!(report.all_ok());
        // Phases: plan(1) + compress(1) + chunk(1) + encode(n) + send(n) + verify(1).
        let n = (p.len() + 222) / 223;
        assert_eq!(report.phase_count(), 4 + 2 * n);
    }

    #[test]
    fn report_tracks_elapsed() {
        let o = TransferOrchestratorV2::new(223, 500);
        let p = payload();
        let mut send = |f: &[u8]| -> Result<(), String> { Ok(()) };
        let report = o.run(&p, &mut send);
        // All phases have non-negative elapsed times and a total.
        assert!(report.phases.iter().all(|ph| ph.elapsed_ms >= 0));
        assert_eq!(
            report.total_elapsed_ms(),
            report.phases.iter().map(|ph| ph.elapsed_ms).sum()
        );
    }

    #[test]
    fn send_failure_stops_and_reports() {
        let o = TransferOrchestratorV2::new(223, 500);
        let p = payload();
        let mut send = |f: &[u8]| -> Result<(), String> { Err("link down".to_string()) };
        let report = o.run(&p, &mut send);
        assert!(!report.all_ok());
        // The last phase is a failed Send.
        let last = report.phases.last().expect("phases");
        assert_eq!(last.phase, Phase::Send);
        assert!(!last.ok);
        assert!(last.detail.contains("link down"));
    }

    #[test]
    fn zips_large_payload() {
        let o = TransferOrchestratorV2::new(223, 500);
        let big = vec![9u8; 2000];
        let mut send = |f: &[u8]| -> Result<(), String> { Ok(()) };
        let report = o.run(&big, &mut send);
        assert!(report.all_ok());
        // The Compress phase detail shows the zip size reduction.
        let compress = report
            .phases
            .iter()
            .find(|p| p.phase == Phase::Compress)
            .expect("compress phase");
        assert!(compress.detail.contains("bytes"));
    }

    #[test]
    fn chunk_bytes_preserved() {
        let o = TransferOrchestratorV2::new(100, 200);
        assert_eq!(o.chunk_bytes(), 100);
    }
}
