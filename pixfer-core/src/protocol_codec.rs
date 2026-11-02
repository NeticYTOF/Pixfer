//! Protocol codec: a higher-level codec that wraps the low-level `Protocol`
//! frame encode/decode with validation and error reporting. (Hilariously
//! over-engineered: a "codec" for a protocol that's already encoded by
//! `Protocol`.)

use crate::protocol::{Frame, FrameKind, Protocol, ProtocolError};
use crate::rs::RS_K;

/// A validated decode result.
#[derive(Debug, Clone)]
pub struct DecodeResult {
    pub ok: bool,
    pub frame: Option<Frame>,
    pub error: Option<ProtocolError>,
}

/// The codec: a thin, validated wrapper over `Protocol`.
pub struct ProtocolCodec;

impl ProtocolCodec {
    pub fn new() -> Self {
        Self
    }

    /// Render a frame into the 270x480 color-block grid.
    pub fn encode(frame: &Frame) -> Vec<u8> {
        Protocol::render(frame)
    }

    /// Decode a captured block grid into a frame, returning a structured result.
    pub fn decode(blocks: &[u8]) -> DecodeResult {
        match Protocol::decode(blocks) {
            Ok(frame) => DecodeResult {
                ok: true,
                frame: Some(frame),
                error: None,
            },
            Err(e) => DecodeResult {
                ok: false,
                frame: None,
                error: Some(e),
            },
        }
    }

    /// Validate that a chunk frame's payload is within the RS_K bound.
    pub fn validate_payload(frame: &Frame) -> bool {
        if frame.kind != FrameKind::Chunk {
            return true;
        }
        (frame.payload.len() as u64) <= (RS_K as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_frame(idx: u32, total: u32, payload: &[u8]) -> Frame {
        Frame::new(FrameKind::Chunk, 1, idx, total, payload.to_vec())
    }

    fn seq_payload() -> Vec<u8> {
        (0..RS_K).map(|i| i as u8).collect()
    }

    #[test]
    fn roundtrip() {
        let frame = chunk_frame(3, 6, &seq_payload());
        let blocks = ProtocolCodec::encode(&frame);
        let dec = ProtocolCodec::decode(&blocks);
        assert!(dec.ok);
        assert_eq!(dec.frame.unwrap().chunk_idx, 3);
    }

    #[test]
    fn bad_decode_reports_error() {
        let dec = ProtocolCodec::decode(&[0u8; 10]);
        assert!(!dec.ok);
        assert!(dec.error.is_some());
    }

    #[test]
    fn validate_payload_bounds() {
        let ok_frame = chunk_frame(0, 4, &seq_payload());
        assert!(ProtocolCodec::validate_payload(&ok_frame));
        let big = Frame::new(FrameKind::Chunk, 1, 0, 4, vec![0u8; RS_K + 1]);
        assert!(!ProtocolCodec::validate_payload(&big));
        let ping = Frame::new(FrameKind::Ping, 1, 0, 0, Vec::new());
        assert!(ProtocolCodec::validate_payload(&ping));
    }
}
