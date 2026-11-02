//! The transfer pipeline: ordered stages (validate, compress, chunk, encode)
//! behind a `PipelineStage` trait, with a `Pipeline` runner and a standard
//! stage composition. (Hilariously over-engineered: a full pipeline framework
//! for a two-device light link.)

use crate::compress::zip_bytes;
use crate::config::Config;
use crate::protocol::{Frame, FrameKind};
use crate::rs::RS_K;

/// Mutable context threaded through every stage of the pipeline.
#[derive(Debug, Default)]
pub struct PipelineContext {
    pub session_id: u16,
    pub payload: Vec<u8>,
    pub zipped: bool,
    pub chunks: Vec<Vec<u8>>,
    pub frames: Vec<Frame>,
}

/// A single pipeline stage.
pub trait PipelineStage: Send {
    fn name(&self) -> &'static str;
    fn run(&self, ctx: &mut PipelineContext, config: &Config) -> Result<(), PipelineError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineError {
    InvalidConfig,
    EmptyPayload,
    ChunkingFailed,
    EncodeFailed,
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            PipelineError::InvalidConfig => "config failed validation",
            PipelineError::EmptyPayload => "payload is empty",
            PipelineError::ChunkingFailed => "chunking failed",
            PipelineError::EncodeFailed => "frame encoding failed",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for PipelineError {}

/// Validates the config before any work happens.
pub struct ValidateStage;

impl PipelineStage for ValidateStage {
    fn name(&self) -> &'static str {
        "validate"
    }
    fn run(&self, _ctx: &mut PipelineContext, config: &Config) -> Result<(), PipelineError> {
        if !config.validate().is_empty() {
            return Err(PipelineError::InvalidConfig);
        }
        Ok(())
    }
}

/// Auto-zips the payload when the config's zip policy says so.
pub struct CompressStage;

impl PipelineStage for CompressStage {
    fn name(&self) -> &'static str {
        "compress"
    }
    fn run(&self, ctx: &mut PipelineContext, config: &Config) -> Result<(), PipelineError> {
        if ctx.payload.is_empty() {
            return Err(PipelineError::EmptyPayload);
        }
        if config.needs_zip(ctx.payload.len() as u64) {
            ctx.payload = zip_bytes(&ctx.payload, "pixfer");
            ctx.zipped = true;
        }
        Ok(())
    }
}

/// Splits the payload into RS_K-byte chunks.
pub struct ChunkStage;

impl PipelineStage for ChunkStage {
    fn name(&self) -> &'static str {
        "chunk"
    }
    fn run(&self, ctx: &mut PipelineContext, _config: &Config) -> Result<(), PipelineError> {
        if ctx.payload.is_empty() {
            return Err(PipelineError::ChunkingFailed);
        }
        ctx.chunks.clear();
        let mut start = 0;
        while start < ctx.payload.len() {
            let end = (start + RS_K).min(ctx.payload.len());
            ctx.chunks.push(ctx.payload[start..end].to_vec());
            start = end;
        }
        if ctx.chunks.is_empty() {
            return Err(PipelineError::ChunkingFailed);
        }
        Ok(())
    }
}

/// Builds one Chunk frame per chunk.
pub struct EncodeStage;

impl PipelineStage for EncodeStage {
    fn name(&self) -> &'static str {
        "encode"
    }
    fn run(&self, ctx: &mut PipelineContext, _config: &Config) -> Result<(), PipelineError> {
        if ctx.chunks.is_empty() {
            return Err(PipelineError::EncodeFailed);
        }
        ctx.frames.clear();
        let total = ctx.chunks.len() as u32;
        for (i, chunk) in ctx.chunks.iter().enumerate() {
            ctx.frames.push(Frame::new(
                FrameKind::Chunk,
                ctx.session_id,
                i as u32,
                total,
                chunk.clone(),
            ));
        }
        if ctx.frames.len() != ctx.chunks.len() {
            return Err(PipelineError::EncodeFailed);
        }
        Ok(())
    }
}

/// The pipeline: an ordered list of stages plus a runner.
#[derive(Default)]
pub struct Pipeline {
    stages: Vec<Box<dyn PipelineStage>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_stage(&mut self, stage: Box<dyn PipelineStage>) {
        self.stages.push(stage);
    }

    pub fn clear_stages(&mut self) {
        self.stages.clear();
    }

    pub fn len(&self) -> usize {
        self.stages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    pub fn stage_names(&self) -> Vec<&'static str> {
        self.stages.iter().map(|s| s.name()).collect()
    }

    /// The standard pipeline: validate -> compress -> chunk -> encode.
    pub fn standard() -> Self {
        let mut p = Self::new();
        p.add_stage(Box::new(ValidateStage));
        p.add_stage(Box::new(CompressStage));
        p.add_stage(Box::new(ChunkStage));
        p.add_stage(Box::new(EncodeStage));
        p
    }

    /// Run every stage in order, threading the context.
    pub fn run(
        &self,
        config: &Config,
        session_id: u16,
        payload: Vec<u8>,
    ) -> Result<PipelineContext, PipelineError> {
        let mut ctx = PipelineContext {
            session_id,
            payload,
            zipped: false,
            chunks: Vec::new(),
            frames: Vec::new(),
        };
        for stage in &self.stages {
            stage.run(&mut ctx, config)?;
        }
        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigProfile;
    use crate::protocol::Protocol;

    #[test]
    fn standard_pipeline_chunks_payload() {
        let p = Pipeline::standard();
        let cfg = Config::default();
        let payload: Vec<u8> = vec![0xAB; 300]; // 300 = 223 + 77
        let ctx = p.run(&cfg, 7, payload).unwrap();
        assert_eq!(ctx.chunks.len(), 2);
        assert_eq!(ctx.chunks[0].len(), 223);
        assert_eq!(ctx.chunks[1].len(), 77);
        assert_eq!(ctx.frames.len(), 2);
        assert_eq!(ctx.frames[0].kind, FrameKind::Chunk);
        assert_eq!(ctx.frames[1].chunk_idx, 1);
        assert!(!ctx.zipped);
    }

    #[test]
    fn standard_pipeline_zips_large_payload() {
        let p = Pipeline::standard();
        let cfg = Config::for_profile(ConfigProfile::Balanced);
        // 70 MB > 60 MB boundary (2 MB/s x 30 s) -> auto-zip
        let payload: Vec<u8> = vec![0xAB; 70_000_000];
        let ctx = p.run(&cfg, 7, payload).unwrap();
        assert!(ctx.zipped);
        // Zipping 70 MB of 0xAB via deflate still yields ~311 KB -> ~1430
        // chunks, vs 313,902 raw chunks: a ~200x reduction.
        let raw_chunks = (70_000_000 + RS_K - 1) / RS_K;
        assert!(ctx.chunks.len() * 100 < raw_chunks);
        assert_eq!(ctx.frames.len(), ctx.chunks.len());
    }

    #[test]
    fn validate_stage_catches_bad_config() {
        let p = Pipeline::standard();
        let mut cfg = Config::default();
        cfg.default_throughput = 0.0;
        let err = p.run(&cfg, 1, vec![1, 2, 3]).unwrap_err();
        assert_eq!(err, PipelineError::InvalidConfig);
    }

    #[test]
    fn empty_payload_rejected() {
        let p = Pipeline::standard();
        let cfg = Config::default();
        let err = p.run(&cfg, 1, Vec::new()).unwrap_err();
        assert_eq!(err, PipelineError::EmptyPayload);
    }

    #[test]
    fn custom_pipeline_order() {
        let mut p = Pipeline::new();
        p.add_stage(Box::new(ChunkStage));
        p.add_stage(Box::new(EncodeStage));
        assert_eq!(p.stage_names(), vec!["chunk", "encode"]);
        let cfg = Config::default();
        let ctx = p.run(&cfg, 3, vec![0u8; 223]).unwrap();
        assert_eq!(ctx.frames.len(), 1);
    }

    #[test]
    fn frames_roundtrip_through_protocol() {
        let p = Pipeline::standard();
        let cfg = Config::default();
        let payload: Vec<u8> = (0..250).map(|i| i as u8).collect();
        let ctx = p.run(&cfg, 5, payload).unwrap();
        for f in &ctx.frames {
            let blocks = Protocol::render(f);
            let decoded = Protocol::decode(&blocks).unwrap();
            assert_eq!(decoded.payload, f.payload);
            assert_eq!(decoded.kind, FrameKind::Chunk);
            assert_eq!(decoded.session_id, 5);
        }
    }
}
