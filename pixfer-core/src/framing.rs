//! Frame construction and validation: a fluent `FrameBuilder`, a
//! `FrameValidator` (magic, kind, length, CRC), and a `FramePool` for
//! reusing frame structs. (Hilariously over-engineered: a full framing
//! toolkit for a two-device light link.)

use std::collections::BTreeMap;

use crate::protocol::{Frame, FrameKind, Protocol, ProtocolError};
use crate::rs::RS_K;

/// Fluent frame builder with strict validation on build.
#[derive(Debug, Default)]
pub struct FrameBuilder {
    kind: Option<FrameKind>,
    session_id: u16,
    chunk_idx: u32,
    total_chunks: u32,
    payload: Vec<u8>,
}

impl FrameBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kind(mut self, k: FrameKind) -> Self {
        self.kind = Some(k);
        self
    }

    pub fn session_id(mut self, id: u16) -> Self {
        self.session_id = id;
        self
    }

    pub fn chunk_idx(mut self, i: u32) -> Self {
        self.chunk_idx = i;
        self
    }

    pub fn total_chunks(mut self, t: u32) -> Self {
        self.total_chunks = t;
        self
    }

    pub fn payload(mut self, p: Vec<u8>) -> Self {
        self.payload = p;
        self
    }

    /// Build the frame, validating as we go.
    pub fn build(self) -> Result<Frame, ProtocolError> {
        let kind = self.kind.ok_or(ProtocolError::DecodeFailed)?;
        if self.payload.len() > RS_K {
            return Err(ProtocolError::BadLength);
        }
        Ok(Frame::new(
            kind,
            self.session_id,
            self.chunk_idx,
            self.total_chunks,
            self.payload,
        ))
    }

    /// Build a Chunk frame for the given chunk index.
    pub fn chunk(
        session_id: u16,
        idx: u32,
        total: u32,
        payload: Vec<u8>,
    ) -> Result<Frame, ProtocolError> {
        Self::new()
            .kind(FrameKind::Chunk)
            .session_id(session_id)
            .chunk_idx(idx)
            .total_chunks(total)
            .payload(payload)
            .build()
    }
}

/// Validates frames and captured block grids.
pub struct FrameValidator;

impl FrameValidator {
    /// Check a decoded frame: valid kind (guaranteed by the enum), payload
    /// length <= RS_K, and total >= chunk_idx+1 for Chunk frames.
    pub fn validate(f: &Frame) -> Result<(), ProtocolError> {
        if f.payload.len() > RS_K {
            return Err(ProtocolError::BadLength);
        }
        if matches!(f.kind, FrameKind::Chunk | FrameKind::Ack | FrameKind::Nak) {
            if f.total_chunks > 0 && f.chunk_idx >= f.total_chunks {
                return Err(ProtocolError::BadLength);
            }
        }
        Ok(())
    }

    /// Decode + validate a captured grid.
    pub fn validate_blocks(blocks: &[u8]) -> Result<Frame, ProtocolError> {
        Protocol::decode(blocks)
    }
}

/// A small object pool so hot loops don't reallocate `Frame`s.
#[derive(Default)]
pub struct FramePool {
    free: Vec<Frame>,
    used: BTreeMap<u32, Frame>,
}

impl FramePool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&mut self) -> Frame {
        self.free
            .pop()
            .unwrap_or_else(|| Frame::new(FrameKind::Ping, 0, 0, 0, Vec::new()))
    }

    pub fn release(&mut self, f: Frame) {
        self.free.push(f);
    }

    pub fn track(&mut self, idx: u32, f: Frame) {
        self.used.insert(idx, f);
    }

    pub fn get_tracked(&self, idx: u32) -> Option<&Frame> {
        self.used.get(&idx)
    }

    pub fn len_used(&self) -> usize {
        self.used.len()
    }

    pub fn len_free(&self) -> usize {
        self.free.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_builds_chunk_frame() {
        let f = FrameBuilder::chunk(5, 3, 8, vec![1, 2, 3]).unwrap();
        assert_eq!(f.kind, FrameKind::Chunk);
        assert_eq!(f.session_id, 5);
        assert_eq!(f.chunk_idx, 3);
        assert_eq!(f.total_chunks, 8);
        assert_eq!(f.payload, vec![1, 2, 3]);
    }

    #[test]
    fn builder_requires_kind() {
        let err = FrameBuilder::new().session_id(1).build().unwrap_err();
        assert_eq!(err, ProtocolError::DecodeFailed);
    }

    #[test]
    fn builder_rejects_oversized_payload() {
        let big = vec![0u8; RS_K + 1];
        let err = FrameBuilder::new()
            .kind(FrameKind::Chunk)
            .payload(big)
            .build()
            .unwrap_err();
        assert_eq!(err, ProtocolError::BadLength);
    }

    #[test]
    fn validator_accepts_good_frame() {
        let f = FrameBuilder::chunk(1, 0, 4, vec![9]).unwrap();
        assert_eq!(FrameValidator::validate(&f), Ok(()));
    }

    #[test]
    fn validator_rejects_bad_chunk_idx() {
        let f = FrameBuilder::new()
            .kind(FrameKind::Chunk)
            .session_id(1)
            .chunk_idx(4)
            .total_chunks(4)
            .payload(vec![9])
            .build()
            .unwrap();
        assert_eq!(FrameValidator::validate(&f), Err(ProtocolError::BadLength));
    }

    #[test]
    fn validator_decodes_grid() {
        let f = FrameBuilder::chunk(7, 0, 2, vec![42]).unwrap();
        let blocks = Protocol::render(&f);
        let decoded = FrameValidator::validate_blocks(&blocks).unwrap();
        assert_eq!(decoded.payload, vec![42]);
        assert_eq!(decoded.chunk_idx, 0);
    }

    #[test]
    fn pool_reuses_frames() {
        let mut pool = FramePool::new();
        let f = pool.get();
        pool.release(f);
        assert_eq!(pool.len_free(), 1);
        pool.track(3, FrameBuilder::chunk(1, 3, 5, vec![7]).unwrap());
        assert_eq!(pool.get_tracked(3).unwrap().chunk_idx, 3);
        assert_eq!(pool.len_used(), 1);
        assert!(pool.get_tracked(99).is_none());
    }
}
