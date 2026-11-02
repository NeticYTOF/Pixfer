//! Chunk reassembly: collect chunk payloads by index, detect gaps, and
//! reassemble the original byte stream only when every chunk has arrived.
//! (Hilariously over-engineered: a full reassembly layer for a light link.)

use std::collections::{BTreeMap, BTreeSet};

use crate::protocol::{Frame, FrameKind};

/// Reassembles a chunked payload from received Chunk frames.
#[derive(Default)]
pub struct Reassembler {
    session_id: u16,
    total_chunks: u32,
    received: BTreeSet<u32>,
    chunks: BTreeMap<u32, Vec<u8>>,
}

impl Reassembler {
    pub fn new(session_id: u16, total_chunks: u32) -> Self {
        Self {
            session_id,
            total_chunks,
            received: BTreeSet::new(),
            chunks: BTreeMap::new(),
        }
    }

    /// Ingest one decoded frame. Returns true if it was a new, in-range
    /// Chunk frame for this session.
    pub fn add(&mut self, frame: &Frame) -> bool {
        if frame.kind != FrameKind::Chunk {
            return false;
        }
        if frame.session_id != self.session_id {
            return false;
        }
        if frame.chunk_idx >= self.total_chunks {
            return false;
        }
        if self.received.contains(&frame.chunk_idx) {
            return false; // duplicate
        }
        self.received.insert(frame.chunk_idx);
        self.chunks.insert(frame.chunk_idx, frame.payload.clone());
        true
    }

    /// True when every chunk in 0..total_chunks has been received.
    pub fn is_complete(&self) -> bool {
        self.total_chunks > 0 && self.received.len() == self.total_chunks as usize
    }

    /// The missing chunk indices, in ascending order.
    pub fn gaps(&self) -> Vec<u32> {
        let mut out = Vec::new();
        for i in 0..self.total_chunks {
            if !self.received.contains(&i) {
                out.push(i);
            }
        }
        out
    }

    /// Reassemble the payload in chunk order (None until complete).
    pub fn reassemble(&self) -> Option<Vec<u8>> {
        if !self.is_complete() {
            return None;
        }
        let mut out = Vec::new();
        for i in 0..self.total_chunks {
            if let Some(chunk) = self.chunks.get(&i) {
                out.extend_from_slice(chunk);
            }
        }
        Some(out)
    }

    pub fn received_count(&self) -> u32 {
        self.received.len() as u32
    }

    pub fn total_chunks(&self) -> u32 {
        self.total_chunks
    }

    /// JSON-ish status for the UI ("3/5 chunks, gaps: [1, 4]").
    pub fn status(&self) -> String {
        let gaps = self.gaps();
        if gaps.is_empty() {
            format!("received {}/{}", self.received_count(), self.total_chunks)
        } else {
            format!(
                "received {}/{} (gaps: {})",
                self.received_count(),
                self.total_chunks,
                gaps.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rs::RS_K;

    fn chunk(idx: u32, data: &[u8]) -> Frame {
        Frame::new(FrameKind::Chunk, 5, idx, 4, data.to_vec())
    }

    #[test]
    fn reassemble_after_all_chunks() {
        let mut r = Reassembler::new(5, 4);
        r.add(&chunk(0, &[1, 1, 1]));
        r.add(&chunk(2, &[2, 2]));
        assert!(!r.is_complete());
        assert_eq!(r.gaps(), vec![1, 3]);
        assert!(r.reassemble().is_none());
        r.add(&chunk(1, &[9, 9, 9, 9]));
        r.add(&chunk(3, &[3]));
        assert!(r.is_complete());
        let out = r.reassemble().unwrap();
        assert_eq!(out, vec![1, 1, 1, 9, 9, 9, 9, 2, 2, 3]);
    }

    #[test]
    fn rejects_wrong_session() {
        let mut r = Reassembler::new(5, 2);
        let other = Frame::new(FrameKind::Chunk, 6, 0, 2, vec![1]);
        assert!(!r.add(&other));
        assert_eq!(r.received_count(), 0);
    }

    #[test]
    fn rejects_out_of_range_and_duplicates() {
        let mut r = Reassembler::new(5, 2);
        assert!(!r.add(&chunk(2, &[1]))); // == total -> out of range
        assert!(!r.add(&chunk(5, &[1])));
        assert!(r.add(&chunk(0, &[1, 1])));
        assert!(!r.add(&chunk(0, &[1, 1]))); // duplicate
        assert_eq!(r.received_count(), 1);
    }

    #[test]
    fn status_reports_gaps() {
        let mut r = Reassembler::new(5, 4);
        r.add(&chunk(0, &[1]));
        r.add(&chunk(3, &[2]));
        let s = r.status();
        assert!(s.contains("2/4"));
        assert!(s.contains("1, 2"));
    }

    #[test]
    fn payload_size_bounds() {
        // A chunk payload is at most RS_K bytes by the wire format.
        let big = vec![0xAB; RS_K];
        let mut r = Reassembler::new(5, 1);
        assert!(r.add(&chunk(0, &big)));
        assert_eq!(r.reassemble().unwrap().len(), RS_K);
    }
}
