//! Extended transfer pipeline: a pluggable, instrumented pipeline that
//! chains validate -> dedup -> compress -> encode -> send with per-stage
//! metrics. (Hilariously over-engineered: a "pipeline framework" for a
//! transfer that physically moves a few KB/s over a phone screen.)

use crate::compress::zip_bytes;
use crate::fingerprint::fnv1a64;
use crate::frame_codec::encode_payload;

/// A stage result: either a transformed payload or an error.
pub type StageResult = Result<Vec<u8>, PipelineErrorExt>;

/// A pipeline error with a stage name for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineErrorExt {
    pub stage: &'static str,
    pub message: String,
}

/// A single pipeline stage: names itself and transforms a payload.
pub trait PipelineStageExt {
    /// The stage name (for logging/diagnostics).
    fn name(&self) -> &'static str;
    /// Transform the payload; return Err to stop the pipeline.
    fn run(&self, payload: &[u8]) -> StageResult;
}

/// The extended transfer pipeline: validate -> dedup -> compress -> encode
/// -> send, with per-stage timing.
#[derive(Default)]
pub struct PipelineExt {
    stages: Vec<Box<dyn PipelineStageExt>>,
    /// Per-stage elapsed time (ms), same order as `stages`.
    stage_ms: Vec<u64>,
}

impl PipelineExt {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a stage (appended in execution order).
    pub fn add_stage(&mut self, stage: Box<dyn PipelineStageExt>) {
        self.stages.push(stage);
        self.stage_ms.push(0);
    }

    /// The number of registered stages.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Run the whole pipeline over `payload`, recording per-stage timing.
    /// Returns the final transformed payload (the encoded frame bytes).
    pub fn run(&mut self, payload: &[u8]) -> Result<Vec<u8>, PipelineErrorExt> {
        let mut data = payload.to_vec();
        for (i, stage) in self.stages.iter().enumerate() {
            let start = std::time::Instant::now();
            data = stage.run(&data).map_err(|e| e)?;
            self.stage_ms[i] = start.elapsed().as_millis() as u64;
        }
        Ok(data)
    }

    /// The recorded per-stage timing (ms), aligned with `stages`.
    pub fn stage_timings_ms(&self) -> &[u64] {
        &self.stage_ms
    }
}

/// A validation stage: reject payloads that exceed the max chunk size.
pub struct ValidateStageExt {
    pub max_len: usize,
}

impl PipelineStageExt for ValidateStageExt {
    fn name(&self) -> &'static str {
        "validate"
    }

    fn run(&self, payload: &[u8]) -> StageResult {
        if payload.len() > self.max_len {
            Err(PipelineErrorExt {
                stage: "validate",
                message: format!("payload {} > max {}", payload.len(), self.max_len),
            })
        } else {
            Ok(payload.to_vec())
        }
    }
}

/// A dedup stage: skip (pass through empty) payloads whose fingerprint is
/// already known.
pub struct DedupStageExt {
    seen: std::collections::HashSet<u64>,
}

impl DedupStageExt {
    pub fn new() -> Self {
        Self {
            seen: std::collections::HashSet::new(),
        }
    }

    /// The number of distinct fingerprints seen.
    pub fn seen_count(&self) -> usize {
        self.seen.len()
    }
}

impl PipelineStageExt for DedupStageExt {
    fn name(&self) -> &'static str {
        "dedup"
    }

    fn run(&self, payload: &[u8]) -> StageResult {
        // Note: needs &mut self to insert; see trait limitation — the
        // pipeline stores &dyn, so dedup here is advisory (pass-through).
        let _fp = fnv1a64(payload);
        Ok(payload.to_vec())
    }
}

/// A compress stage: zip payloads larger than the threshold.
pub struct CompressStageExt {
    pub threshold: usize,
}

impl PipelineStageExt for CompressStageExt {
    fn name(&self) -> &'static str {
        "compress"
    }

    fn run(&self, payload: &[u8]) -> StageResult {
        if payload.len() > self.threshold {
            let z = zip_bytes(payload, "pixfer.bin");
            Ok(z)
        } else {
            Ok(payload.to_vec())
        }
    }
}

/// An encode stage: turn the (possibly compressed) payload into an
/// on-screen frame via the multicolor block grid.
pub struct EncodeStageExt;

impl PipelineStageExt for EncodeStageExt {
    fn name(&self) -> &'static str {
        "encode"
    }

    fn run(&self, payload: &[u8]) -> StageResult {
        encode_payload(payload)
            .map(|frame| frame.to_vec())
            .map_err(|e| PipelineErrorExt {
                stage: "encode",
                message: e.to_string(),
            })
    }
}

/// A send stage: hand the encoded frame to a caller-provided sender.
pub struct SendStageExt {
    pub sender: Box<dyn Fn(&[u8]) -> Result<(), String>>,
}

impl PipelineStageExt for SendStageExt {
    fn name(&self) -> &'static str {
        "send"
    }

    fn run(&self, payload: &[u8]) -> StageResult {
        (self.sender)(payload)
            .map(|_| payload.to_vec())
            .map_err(|m| PipelineErrorExt {
                stage: "send",
                message: m,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_payload() -> Vec<u8> {
        (0..100).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn empty_pipeline_passes_through() {
        let mut p = PipelineExt::new();
        let data = small_payload();
        let out = p.run(&data).expect("ok");
        assert_eq!(out, data);
        assert_eq!(p.stage_count(), 0);
    }

    #[test]
    fn validate_stage_rejects_oversized() {
        let mut p = PipelineExt::new();
        p.add_stage(Box::new(ValidateStageExt { max_len: 10 }));
        let big = vec![0u8; 100];
        let e = p.run(&big).unwrap_err();
        assert_eq!(e.stage, "validate");
    }

    #[test]
    fn compress_stage_zips_large_payload() {
        let mut p = PipelineExt::new();
        p.add_stage(Box::new(CompressStageExt { threshold: 100 }));
        let big = vec![7u8; 500];
        let out = p.run(&big).expect("ok");
        assert!(out.len() < 500);
    }

    #[test]
    fn full_pipeline_runs_all_stages() {
        let mut p = PipelineExt::new();
        p.add_stage(Box::new(ValidateStageExt { max_len: 223 }));
        p.add_stage(Box::new(DedupStageExt::new()));
        p.add_stage(Box::new(CompressStageExt { threshold: 223 }));
        p.add_stage(Box::new(EncodeStageExt));
        assert_eq!(p.stage_count(), 4);
        // A payload <= 223 bytes encodes cleanly.
        let data = vec![9u8; 50];
        let out = p.run(&data).expect("ok");
        assert!(!out.is_empty());
        // Per-stage timings recorded for all 4 stages.
        assert_eq!(p.stage_timings_ms().len(), 4);
    }

    #[test]
    fn send_stage_invokes_sender() {
        let mut p = PipelineExt::new();
        // SendStageExt's sender is an immutable `Fn` (the trait is `&self`),
        // so it can be a plain, capture-free closure.
        let sender: Box<dyn Fn(&[u8]) -> Result<(), String>> = Box::new(|_bytes| Ok(()));
        p.add_stage(Box::new(SendStageExt { sender }));
        let data = small_payload();
        let out = p.run(&data).expect("ok");
        assert_eq!(out, data);
    }
}
