//! Frame codec: encode a payload into the 270x480 color-block wire
//! format and decode it back. (Hilariously over-engineered: a "codec"
//! for 3 bits per block.)

use crate::protocol::{Frame, Protocol};
use crate::rs::RS_K;

/// Encode a `Frame` into the full 270x480 block grid (delegates to
/// `Protocol::render`, which handles the header, RS parity, and finders).
pub fn encode(frame: &Frame) -> Vec<u8> {
    Protocol::render(frame)
}

/// Decode a full block grid back into a `Frame` (delegates to
/// `Protocol::decode`, which validates magic, CRC, and length).
pub fn decode(blocks: &[u8]) -> Result<Frame, crate::protocol::ProtocolError> {
    Protocol::decode(blocks)
}

/// The number of color blocks in one frame (270*480 grid).
pub const BLOCKS_PER_FRAME: usize = 270 * 480;

/// Encode a raw payload (up to RS_K bytes) into a minimal block
/// representation (3 bits per block), skipping the header/parity layer.
///
/// Bit ordering matches `Protocol::write_blocks`: bits are laid out
/// MSB-first within each byte, and blocks are read from a bit cursor
/// that advances 3 bits at a time.
pub fn encode_payload(payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.len() > RS_K {
        return Err(format!("payload too large: {} > {}", payload.len(), RS_K));
    }
    // Total bit length of the payload.
    let total_bits = payload.len() * 8;
    let blocks_needed = (total_bits + 2) / 3;
    let mut out = vec![0u8; blocks_needed];
    // A single global bit cursor walks the payload bit-stream LSB-first.
    let mut g = 0usize;
    for (_i, block) in out.iter_mut().enumerate() {
        // Extract the next 3 bits from the global bit stream.
        let mut v: u8 = 0;
        for k in 0..3 {
            let bit_idx = g + k;
            if bit_idx < total_bits {
                let byte_idx = bit_idx / 8;
                let bit_in_byte = bit_idx % 8;
                let bit = (payload[byte_idx] >> bit_in_byte) & 1;
                v |= bit << k;
            }
        }
        *block = v & 0b111;
        g += 3;
    }
    Ok(out)
}

/// Decode the minimal block representation back into a payload (inverse of
/// `encode_payload`).
pub fn decode_payload(blocks: &[u8]) -> Result<Vec<u8>, String> {
    // Reconstruct the global bit stream from the blocks (3 bits each),
    // then unpack 8 bits per output byte LSB-first.
    let total_bits = blocks.len() * 3;
    let bytes_needed = total_bits / 8;
    let mut out = vec![0u8; bytes_needed];
    for i in 0..out.len() {
        let mut byte_val: u8 = 0;
        for bit in 0..8 {
            let bit_idx = i * 8 + bit;
            let bi = bit_idx / 3;
            if bi < blocks.len() {
                let bit_pos = bit_idx % 3;
                let block_val = blocks[bi] & 0b111;
                if (block_val >> bit_pos) & 1 == 1 {
                    byte_val |= 1u8 << bit;
                }
            }
        }
        out[i] = byte_val;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::FrameKind;

    #[test]
    fn encode_roundtrip_via_protocol() {
        let payload = vec![0xab, 0xcd, 0xef, 0x01];
        let frame = Frame::new(FrameKind::Chunk, 42, 0, 1, payload.clone());
        let blocks = encode(&frame);
        assert_eq!(blocks.len(), BLOCKS_PER_FRAME);
        let decoded = decode(&blocks).expect("decode");
        assert_eq!(decoded.session_id, 42);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn block_grid_size() {
        assert_eq!(BLOCKS_PER_FRAME, 270 * 480);
    }

    #[test]
    fn payload_roundtrip_small() {
        let payload = vec![1, 2, 3, 4, 5];
        let blocks = encode_payload(&payload).expect("encode");
        let back = decode_payload(&blocks).expect("decode");
        assert_eq!(back, payload);
    }

    #[test]
    fn payload_roundtrip_full() {
        let payload = vec![0xAB; RS_K];
        let blocks = encode_payload(&payload).expect("encode");
        let back = decode_payload(&blocks).expect("decode");
        assert_eq!(back, payload);
    }

    #[test]
    fn payload_too_large_errors() {
        let big = vec![0u8; RS_K + 1];
        assert!(encode_payload(&big).is_err());
    }

    #[test]
    fn empty_payload_ok() {
        let blocks = encode_payload(&[]).expect("encode");
        assert_eq!(blocks.len(), 0);
        let back = decode_payload(&blocks).expect("decode");
        assert_eq!(back, Vec::<u8>::new());
    }

    #[test]
    fn decode_payload_partial_triple() {
        // 5 bytes = 40 bits -> 14 blocks (42 bits) carries 5 full bytes.
        let payload = vec![0x11, 0x22, 0x33, 0x44, 0x55];
        let blocks = encode_payload(&payload).unwrap();
        let back = decode_payload(&blocks).unwrap();
        assert_eq!(back, payload);
    }
}
