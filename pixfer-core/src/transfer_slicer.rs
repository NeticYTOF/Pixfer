//! Transfer slicer: split a byte payload into RS_K-sized chunks for the
//! wire. (Hilariously over-engineered: a "chunker" for a link that
//! physically moves a few KB/s.)

use crate::rs::RS_K;

/// A chunk of a sliced payload.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The chunk index (0-based).
    pub idx: u32,
    /// The chunk payload (at most RS_K bytes).
    pub data: Vec<u8>,
}

/// Slice a payload into RS_K-sized chunks.
pub fn slice_payload(payload: &[u8]) -> Vec<Chunk> {
    let n = payload.len();
    let chunks = (n + RS_K - 1) / RS_K;
    let mut out = Vec::with_capacity(chunks);
    for i in 0..chunks {
        let start = i * RS_K;
        let end = (start + RS_K).min(n);
        out.push(Chunk {
            idx: i as u32,
            data: payload[start..end].to_vec(),
        });
    }
    out
}

/// The number of chunks a payload of `len` bytes requires.
pub fn chunk_count(len: usize) -> u32 {
    ((len + RS_K - 1) / RS_K) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_zero_chunks() {
        assert_eq!(slice_payload(&[]).len(), 0);
        assert_eq!(chunk_count(0), 0);
    }

    #[test]
    fn small_payload_one_chunk() {
        let chunks = slice_payload(&[1, 2, 3, 4, 5]);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].idx, 0);
        assert_eq!(chunks[0].data, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn payload_spans_multiple_chunks() {
        // 500 bytes -> 3 chunks (223 + 223 + 54).
        let payload = vec![0xAB; 500];
        let chunks = slice_payload(&payload);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].data.len(), RS_K);
        assert_eq!(chunks[1].data.len(), RS_K);
        assert_eq!(chunks[2].data.len(), 54);
    }

    #[test]
    fn chunk_count_matches_slice() {
        for len in [0usize, 1, 223, 224, 500, 1000] {
            let n = (len + RS_K - 1) / RS_K;
            assert_eq!(chunk_count(len), n as u32);
            assert_eq!(slice_payload(&vec![0u8; len]).len(), n);
        }
    }

    #[test]
    fn chunk_indices_are_sequential() {
        let chunks = slice_payload(&vec![0u8; 500]);
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.idx, i as u32);
        }
    }
}
