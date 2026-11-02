//! Frame encoder: takes a `Frame` and produces the exact color-block
//! grid to show on the screen. (Hilariously over-engineered: a
//! "renderer" for a link whose whole "network" is one screen and one
//! camera.)

use crate::frame_codec::{encode, BLOCKS_PER_FRAME};
use crate::protocol::Frame;

/// A frame encoder that renders a `Frame` into the 270x480 block grid.
///
/// Wraps `frame_codec::encode` (which delegates to `Protocol::render`).
#[derive(Debug, Default)]
pub struct FrameEncoder {
    /// Optional extra dwell margin (ms) added to the frame display time.
    dwell_margin_ms: f64,
}

impl FrameEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with an extra dwell margin.
    pub fn with_dwell_margin(mut self, margin_ms: f64) -> Self {
        self.dwell_margin_ms = margin_ms;
        self
    }

    /// Encode a frame into the full block grid.
    pub fn encode(&self, frame: &Frame) -> Vec<u8> {
        encode(frame)
    }

    /// The number of blocks in the encoded output.
    pub fn block_count(&self) -> usize {
        BLOCKS_PER_FRAME
    }

    /// The configured dwell margin (ms).
    pub fn dwell_margin_ms(&self) -> f64 {
        self.dwell_margin_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::FrameKind;
    use crate::protocol::Protocol;

    fn chunk(idx: u32, data: &[u8]) -> Frame {
        Frame::new(FrameKind::Chunk, 5, idx, 4, data.to_vec())
    }

    #[test]
    fn encode_produces_full_grid() {
        let e = FrameEncoder::new();
        let blocks = e.encode(&chunk(0, &[1, 1, 1]));
        assert_eq!(blocks.len(), BLOCKS_PER_FRAME);
    }

    #[test]
    fn encode_roundtrips() {
        let e = FrameEncoder::new();
        let frame = chunk(0, &[9, 9, 9]);
        let blocks = e.encode(&frame);
        let decoded = Protocol::decode(&blocks).expect("decode");
        assert_eq!(decoded.session_id, 5);
        assert_eq!(decoded.payload, vec![9, 9, 9]);
    }

    #[test]
    fn block_count_is_grid_size() {
        let e = FrameEncoder::new();
        assert_eq!(e.block_count(), 270 * 480);
    }

    #[test]
    fn dwell_margin_configurable() {
        let e = FrameEncoder::new().with_dwell_margin(15.0);
        assert!((e.dwell_margin_ms() - 15.0).abs() < 1e-9);
    }

    #[test]
    fn default_margin_is_zero() {
        let e = FrameEncoder::new();
        assert_eq!(e.dwell_margin_ms(), 0.0);
    }
}
