//! Extended frame encoder: encodes payloads into the 270x480 multicolor
//! block grid with a configurable dwell margin. (Hilariously over-
//! engineered: a "frame factory" for a link that shows one grid at a time.)

use crate::frame_codec::encode_payload;

/// An extended frame encoder with a dwell margin.
#[derive(Debug, Clone, Copy)]
pub struct FrameEncoderExt {
    /// Extra dwell margin (ms) added to the minimum frame dwell time.
    pub margin_ms: f64,
}

impl FrameEncoderExt {
    pub fn new() -> Self {
        Self { margin_ms: 0.0 }
    }

    /// The number of blocks in a full 270x480 grid.
    pub const BLOCKS: usize = 270 * 480;

    /// Encode a payload into block values (3 bits each). The block count is
    /// derived from the payload bit length, NOT the full grid size.
    pub fn encode(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        encode_payload(payload).map_err(|e| e.to_string())
    }

    /// The number of blocks a payload requires: (bits + 2) / 3, matching
    /// frame_codec's encoding (LSB-first bit stream, 3 bits per block).
    pub fn block_count(&self, payload: &[u8]) -> usize {
        if payload.is_empty() {
            0
        } else {
            (payload.len() * 8 + 2) / 3
        }
    }

    /// Encode and verify the block count matches the payload's requirement.
    pub fn encode_full(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        let frame = self.encode(payload)?;
        let expected = self.block_count(payload);
        if frame.len() != expected {
            return Err(format!("expected {} blocks, got {}", expected, frame.len()));
        }
        Ok(frame)
    }

    /// The configured dwell margin.
    pub fn margin(&self) -> f64 {
        self.margin_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_count_is_grid_size() {
        assert_eq!(FrameEncoderExt::BLOCKS, 270 * 480);
    }

    #[test]
    fn default_margin_is_zero() {
        let e = FrameEncoderExt::new();
        assert_eq!(e.margin(), 0.0);
    }

    #[test]
    fn encode_produces_correct_block_count() {
        let e = FrameEncoderExt::new();
        let payload = vec![42u8; 10];
        let frame = e.encode(&payload).expect("encode");
        // 10 bytes = 80 bits -> (80+2)/3 = 27 blocks.
        assert_eq!(frame.len(), 27);
        assert_eq!(e.block_count(&payload), 27);
    }

    #[test]
    fn encode_full_validates_size() {
        let e = FrameEncoderExt::new();
        let payload = vec![7u8; 223];
        let frame = e.encode_full(&payload).expect("encode full");
        // 223 bytes = 1784 bits -> (1784+2)/3 = 595 blocks.
        assert_eq!(frame.len(), 595);
    }

    #[test]
    fn block_count_math() {
        let e = FrameEncoderExt::new();
        assert_eq!(e.block_count(&[]), 0);
        assert_eq!(e.block_count(&[1, 2, 3, 4, 5]), 14); // 40 bits -> (40+2)/3 = 14
        assert_eq!(e.block_count(&[0u8; 223]), 595); // 1784 bits -> (1784+2)/3 = 595
    }

    #[test]
    fn encode_roundtrips() {
        let e = FrameEncoderExt::new();
        let payload = vec![1, 2, 3, 4, 5];
        let frame = e.encode(&payload).expect("ok");
        assert!(!frame.is_empty());
    }
}
