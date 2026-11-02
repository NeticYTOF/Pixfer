//! Extended protocol: a higher-level protocol helper that wraps the block
//! grid codec with session framing and CRC checks. (Hilariously over-
//! engineered: a "protocol stack" for a link that physically moves a few
//! KB/s over a phone screen.)

use crate::frame_codec::{decode_payload, encode_payload};
use crate::protocol::{Frame, FrameKind, Protocol, ProtocolError};

/// A protocol-level error.
#[derive(Debug, Clone)]
pub struct ProtocolErrorExt {
    pub stage: &'static str,
    pub message: String,
}

/// A higher-level protocol helper.
#[derive(Default)]
pub struct ProtocolExt {
    session_id: u16,
}

impl ProtocolExt {
    pub fn new(session_id: u16) -> Self {
        Self { session_id }
    }

    /// The session id.
    pub fn session_id(&self) -> u16 {
        self.session_id
    }

    /// Encode a control frame (ping/pong/ack/nak/end).
    pub fn encode_control(&self, kind: FrameKind, payload: &[u8]) -> Vec<u8> {
        let frame = Frame {
            kind,
            session_id: self.session_id,
            chunk_idx: 0,
            total_chunks: 1,
            payload: payload.to_vec(),
        };
        Protocol::render(&frame)
    }

    /// Encode a chunk frame carrying up to RS_K payload bytes.
    pub fn encode_chunk(
        &self,
        chunk_idx: u32,
        total_chunks: u32,
        payload: &[u8],
    ) -> Result<Vec<u8>, ProtocolErrorExt> {
        let encoded = encode_payload(payload).map_err(|e| ProtocolErrorExt {
            stage: "encode",
            message: e,
        })?;
        let frame = Frame {
            kind: FrameKind::Chunk,
            session_id: self.session_id,
            chunk_idx,
            total_chunks,
            payload: encoded,
        };
        Ok(Protocol::render(&frame))
    }

    /// Decode a received block grid into a frame.
    pub fn decode(&self, blocks: &[u8]) -> Result<Frame, ProtocolErrorExt> {
        Protocol::decode(blocks).map_err(|e| ProtocolErrorExt {
            stage: "decode",
            message: match e {
                ProtocolError::BadMagic => "bad magic".to_string(),
                ProtocolError::BadCrc => "bad crc".to_string(),
                ProtocolError::BadLength => "bad length".to_string(),
                ProtocolError::DecodeFailed => "decode failed".to_string(),
            },
        })
    }

    /// Decode and return the payload bytes of a received frame.
    pub fn decode_payload(&self, blocks: &[u8]) -> Result<Vec<u8>, ProtocolErrorExt> {
        let frame = self.decode(blocks)?;
        if frame.kind != FrameKind::Chunk {
            return Err(ProtocolErrorExt {
                stage: "decode_payload",
                message: "frame is not a chunk".to_string(),
            });
        }
        decode_payload(&frame.payload).map_err(|e| ProtocolErrorExt {
            stage: "decode_payload",
            message: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_preserved() {
        let p = ProtocolExt::new(42);
        assert_eq!(p.session_id(), 42);
    }

    #[test]
    fn encode_control_roundtrip() {
        let p = ProtocolExt::new(7);
        let blocks = p.encode_control(FrameKind::Ping, &[]);
        assert!(!blocks.is_empty());
        let frame = p.decode(&blocks).expect("decode");
        assert_eq!(frame.kind, FrameKind::Ping);
        assert_eq!(frame.session_id, 7);
    }

    #[test]
    fn encode_chunk_roundtrip() {
        let p = ProtocolExt::new(1);
        let payload = vec![5u8; 30];
        let blocks = p.encode_chunk(2, 10, &payload).expect("encode");
        let frame = p.decode(&blocks).expect("decode");
        assert_eq!(frame.kind, FrameKind::Chunk);
        assert_eq!(frame.chunk_idx, 2);
        assert_eq!(frame.total_chunks, 10);
        let decoded = p.decode_payload(&blocks).expect("decode payload");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_payload_rejects_non_chunk() {
        let p = ProtocolExt::new(3);
        let blocks = p.encode_control(FrameKind::Ack, &[]);
        let err = p.decode_payload(&blocks).unwrap_err();
        assert_eq!(err.stage, "decode_payload");
    }

    #[test]
    fn oversize_chunk_errors() {
        let p = ProtocolExt::new(5);
        let big = vec![1u8; 300];
        let err = p.encode_chunk(0, 1, &big).unwrap_err();
        assert_eq!(err.stage, "encode");
    }
}
