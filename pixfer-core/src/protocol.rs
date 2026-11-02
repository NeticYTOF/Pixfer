//! Multicolor block-grid wire format.
//!
//! Each 4x4-px block carries 3 bits (R/G/B channels on/off = 8-color palette),
//! giving 3x the bandwidth of a monochrome scheme. Frames are self-contained:
//! QR-style sync finders + 23-byte header (magic, kind, session id, chunk index,
//! total, payload len, CRC32) + 32 RS parity bytes + 223-byte payload (parity-first
//! codeword layout, matching rs.rs).

use crate::rs::{ReedSolomon, RS_K, RS_PARITY};

pub const GRID_W: usize = 270;
pub const GRID_H: usize = 480;
pub const BLOCK_PX: u8 = 4;
pub const FINDER_SIZE: usize = 7;

pub const MAGIC: [u8; 4] = *b"PIXF";

// Header layout (23 bytes):
//  0..4   magic "PIXF"
//  4      kind
//  5..7   session_id (u16 LE)
//  7..11  chunk_idx (u32 LE)
//  11..15 total_chunks (u32 LE)
//  15..19 payload_len (u32 LE)
//  19..23 crc32 (u32 LE) over bytes 0..19 + payload
pub const HEADER_BYTES: usize = 23;

const FINDER_TL: (usize, usize) = (0, 0);
const FINDER_TR: (usize, usize) = (GRID_W - FINDER_SIZE, 0);
const FINDER_BL: (usize, usize) = (0, GRID_H - FINDER_SIZE);

const HEADER_START: usize = FINDER_SIZE * FINDER_SIZE; // 49
const PARITY_START: usize = HEADER_START + (HEADER_BYTES * 8 + 2) / 3; // 111
const PAYLOAD_START: usize = PARITY_START + (RS_PARITY * 8 + 2) / 3; // 197

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Ping = 0,
    Pong = 1,
    Chunk = 2,
    Ack = 3,
    Nak = 4,
    End = 5,
}

impl FrameKind {
    pub fn from_u8(v: u8) -> Option<FrameKind> {
        match v {
            0 => Some(FrameKind::Ping),
            1 => Some(FrameKind::Pong),
            2 => Some(FrameKind::Chunk),
            3 => Some(FrameKind::Ack),
            4 => Some(FrameKind::Nak),
            5 => Some(FrameKind::End),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub kind: FrameKind,
    pub session_id: u16,
    pub chunk_idx: u32,
    pub total_chunks: u32,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(
        kind: FrameKind,
        session_id: u16,
        chunk_idx: u32,
        total_chunks: u32,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            kind,
            session_id,
            chunk_idx,
            total_chunks,
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    BadMagic,
    BadCrc,
    BadLength,
    DecodeFailed,
}

const FINDER: [[u8; FINDER_SIZE]; FINDER_SIZE] = [
    [7, 7, 7, 7, 7, 7, 7],
    [7, 0, 0, 0, 0, 0, 7],
    [7, 0, 0, 0, 0, 0, 7],
    [7, 0, 7, 7, 7, 0, 7],
    [7, 0, 7, 7, 7, 0, 7],
    [7, 0, 0, 0, 0, 0, 7],
    [7, 7, 7, 7, 7, 7, 7],
];

pub struct Protocol;

impl Protocol {
    /// Build the 23-byte header, with CRC32 over bytes 0..19 + payload.
    pub fn header(f: &Frame) -> Vec<u8> {
        let mut head = Vec::with_capacity(HEADER_BYTES);
        head.extend_from_slice(&MAGIC);
        head.push(f.kind as u8);
        head.extend_from_slice(&f.session_id.to_le_bytes());
        head.extend_from_slice(&f.chunk_idx.to_le_bytes());
        head.extend_from_slice(&f.total_chunks.to_le_bytes());
        head.extend_from_slice(&(f.payload.len() as u32).to_le_bytes());
        let head19 = head.clone();
        let mut crc_input = head19.clone();
        crc_input.extend_from_slice(&f.payload);
        let crc = crc32fast::hash(&crc_input);
        head.extend_from_slice(&crc.to_le_bytes());
        head
    }

    /// Render a frame into the full 270x480 grid of 8-color block indices.
    pub fn render(f: &Frame) -> Vec<u8> {
        let mut blocks = vec![0u8; GRID_W * GRID_H];
        // Filler cycles all 8 palette colors so the time-averaged screen
        // color stays neutral (no visible flicker or dimming shift).
        for (i, b) in blocks.iter_mut().enumerate() {
            *b = (i % 8) as u8;
        }

        // Zero the data region (header + parity + payload) so the filler
        // colors don't OR-into the data bits and corrupt them on capture.
        let payload_blocks = (RS_K * 8 + 2) / 3;
        for i in HEADER_START..(PAYLOAD_START + payload_blocks) {
            blocks[i] = 0;
        }

        let head = Self::header(f);
        Self::write_blocks(&mut blocks, HEADER_START, &head);

        let mut payload = f.payload.clone();
        payload.resize(RS_K, 0);
        let rs = ReedSolomon::new();
        let coded = rs.encode(&payload); // [parity_0..31][payload_0..222]
        Self::write_blocks(&mut blocks, PARITY_START, &coded[..RS_PARITY]);
        Self::write_blocks(&mut blocks, PAYLOAD_START, &payload);

        // Finders are placed after the data so they win in the blocks the
        // top-right finder overlaps (its rows 0-1 sit inside the payload
        // region); RS-FEC corrects the few lost payload symbols.
        place_finder(&mut blocks, FINDER_TL);
        place_finder(&mut blocks, FINDER_TR);
        place_finder(&mut blocks, FINDER_BL);
        blocks
    }

    /// Decode a captured block grid back into a frame.
    pub fn decode(blocks: &[u8]) -> Result<Frame, ProtocolError> {
        if blocks.len() != GRID_W * GRID_H {
            return Err(ProtocolError::BadLength);
        }
        let header_blocks = (HEADER_BYTES * 8 + 2) / 3;
        let mut head = [0u8; HEADER_BYTES];
        Self::read_blocks(
            &blocks[HEADER_START..HEADER_START + header_blocks],
            &mut head,
        );
        if head[..4] != MAGIC {
            return Err(ProtocolError::BadMagic);
        }
        let kind = FrameKind::from_u8(head[4]).ok_or(ProtocolError::DecodeFailed)?;
        let session_id = u16::from_le_bytes(head[5..7].try_into().unwrap());
        let chunk_idx = u32::from_le_bytes(head[7..11].try_into().unwrap());
        let total_chunks = u32::from_le_bytes(head[11..15].try_into().unwrap());
        // Clamp: a corrupted header can carry a garbage payload_len; the
        // payload region physically holds at most RS_K bytes.
        let raw_len = u32::from_le_bytes(head[15..19].try_into().unwrap());
        let payload_len = raw_len.min(RS_K as u32) as usize;
        let crc_stored = u32::from_le_bytes(head[19..23].try_into().unwrap());

        let parity_blocks = (RS_PARITY * 8 + 2) / 3;
        let mut parity = vec![0u8; RS_PARITY];
        Self::read_blocks(
            &blocks[PARITY_START..PARITY_START + parity_blocks],
            &mut parity,
        );

        let payload_blocks = (RS_K * 8 + 2) / 3;
        let mut payload_full = vec![0u8; RS_K];
        Self::read_blocks(
            &blocks[PAYLOAD_START..PAYLOAD_START + payload_blocks],
            &mut payload_full,
        );

        // Layered integrity: RS-correct first, then verify CRC on corrected data.
        let mut codeword = parity.clone();
        codeword.extend_from_slice(&payload_full);
        let rs = ReedSolomon::new();
        let payload: Vec<u8> = match rs.decode(&codeword) {
            Ok(corrected) => corrected[..payload_len].to_vec(),
            Err(_) => {
                // Last-resort CRC check on the raw payload
                let mut ci = head[..19].to_vec();
                ci.extend_from_slice(&payload_full[..payload_len.min(payload_full.len())]);
                if crc32fast::hash(&ci) == crc_stored {
                    payload_full[..payload_len].to_vec()
                } else {
                    return Err(ProtocolError::BadCrc);
                }
            }
        };

        let mut ci = head[..19].to_vec();
        ci.extend_from_slice(&payload);
        if crc32fast::hash(&ci) != crc_stored {
            return Err(ProtocolError::BadCrc);
        }
        Ok(Frame {
            kind,
            session_id,
            chunk_idx,
            total_chunks,
            payload,
        })
    }

    /// Write bytes into a run of blocks, 3 bits per block
    /// (bit order within each block: R, G, B = values 1, 2, 4).
    fn write_blocks(blocks: &mut [u8], start: usize, data: &[u8]) {
        let mut bitpos = 0;
        for &byte in data {
            for bit in (0..8).rev() {
                let block_idx = start + bitpos / 3;
                let rem = bitpos % 3;
                if (byte >> bit) & 1 == 1 {
                    match rem {
                        0 => blocks[block_idx] |= 0b001, // R
                        1 => blocks[block_idx] |= 0b010, // G
                        _ => blocks[block_idx] |= 0b100, // B
                    }
                }
                bitpos += 1;
            }
        }
    }

    /// Read bytes out of a run of blocks (inverse of write_blocks).
    fn read_blocks(blocks: &[u8], out: &mut [u8]) {
        let bits_needed = out.len() * 8;
        let blocks_needed = (bits_needed + 2) / 3;
        check_run_len(blocks, blocks_needed);
        let bits: Vec<u8> = blocks
            .iter()
            .flat_map(|&b| [b & 1, (b >> 1) & 1, (b >> 2) & 1])
            .collect();
        for (i, obyte) in out.iter_mut().enumerate() {
            let mut v: u8 = 0;
            for j in 0..8 {
                if bits.get(i * 8 + j).copied().unwrap_or(0) == 1 {
                    v |= 1 << (7 - j);
                }
            }
            *obyte = v;
        }
    }
}

fn check_run_len(blocks: &[u8], needed: usize) {
    if cfg!(debug_assertions) && blocks.len() < needed {
        panic!(
            "block run too short: have {}, need {}",
            blocks.len(),
            needed
        );
    }
}

fn place_finder(blocks: &mut [u8], at: (usize, usize)) {
    let (c0, r0) = at;
    for r in 0..FINDER_SIZE {
        for c in 0..FINDER_SIZE {
            blocks[(r0 + r) * GRID_W + c0 + c] = FINDER[r][c];
        }
    }
}
