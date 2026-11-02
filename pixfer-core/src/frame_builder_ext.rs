//! Frame builder extension: a fluent builder for constructing wire frames with
//! validation hooks. (Hilariously over-engineered: a "builder" for a frame
//! that's already built by `Frame::new`.)

use crate::protocol::{Frame, FrameKind};
use crate::rs::RS_K;

/// A fluent frame builder.
#[derive(Debug, Default)]
pub struct FrameBuilder2 {
    kind: Option<FrameKind>,
    session_id: u16,
    chunk_idx: u32,
    total_chunks: u32,
    payload: Vec<u8>,
}

impl FrameBuilder2 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kind(mut self, kind: FrameKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn session(mut self, id: u16) -> Self {
        self.session_id = id;
        self
    }

    pub fn chunk(mut self, idx: u32, total: u32) -> Self {
        self.chunk_idx = idx;
        self.total_chunks = total;
        self
    }

    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    /// Build the frame, validating chunk invariants for Chunk frames.
    pub fn build(self) -> Result<Frame, &'static str> {
        let kind = self.kind.ok_or("missing_kind")?;
        if kind == FrameKind::Chunk {
            if self.total_chunks == 0 {
                return Err("zero_total_chunks");
            }
            if self.chunk_idx >= self.total_chunks {
                return Err("chunk_idx_out_of_range");
            }
            if (self.payload.len() as u64) > (RS_K as u64) {
                return Err("payload_too_large");
            }
        }
        Ok(Frame::new(
            kind,
            self.session_id,
            self.chunk_idx,
            self.total_chunks,
            self.payload,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_chunk_frame() {
        let f = FrameBuilder2::new()
            .kind(FrameKind::Chunk)
            .session(7)
            .chunk(2, 4)
            .payload(vec![0u8; RS_K])
            .build();
        assert!(f.is_ok());
        let frame = f.unwrap();
        assert_eq!(frame.kind, FrameKind::Chunk);
        assert_eq!(frame.session_id, 7);
        assert_eq!(frame.chunk_idx, 2);
    }

    #[test]
    fn rejects_zero_total() {
        let f = FrameBuilder2::new()
            .kind(FrameKind::Chunk)
            .chunk(0, 0)
            .payload(vec![0u8; RS_K])
            .build();
        assert_eq!(f.err(), Some("zero_total_chunks"));
    }

    #[test]
    fn rejects_oversized_payload() {
        let f = FrameBuilder2::new()
            .kind(FrameKind::Chunk)
            .chunk(0, 4)
            .payload(vec![0u8; RS_K + 1])
            .build();
        assert_eq!(f.err(), Some("payload_too_large"));
    }

    #[test]
    fn missing_kind_errors() {
        let f = FrameBuilder2::new().payload(Vec::new()).build();
        assert_eq!(f.err(), Some("missing_kind"));
    }

    #[test]
    fn builds_control_frame() {
        let f = FrameBuilder2::new()
            .kind(FrameKind::Ping)
            .session(1)
            .build();
        assert!(f.is_ok());
        assert_eq!(f.unwrap().kind, FrameKind::Ping);
    }
}
