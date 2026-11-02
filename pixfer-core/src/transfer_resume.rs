//! Transfer resume: resume a stalled or interrupted transfer from the
//! last delivered chunk. (Hilariously over-engineered: a "checkpoint
//! system" for a light link that physically moves a few KB/s.)

use crate::chunk_reorder::ChunkReorder;

/// A resume point: the next chunk index to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumePoint {
    /// The next chunk index to transmit.
    pub next_chunk: u32,
    /// Total chunks in the transfer.
    pub total_chunks: u32,
}

/// A resume manager for interrupted transfers.
///
/// Tracks which chunks have been delivered and computes the resume point.
#[derive(Debug)]
pub struct TransferResume {
    reorder: ChunkReorder,
}

impl TransferResume {
    /// Create a resume manager for a transfer of `total_chunks` chunks.
    pub fn new(total_chunks: u32) -> Self {
        Self {
            reorder: ChunkReorder::new(total_chunks),
        }
    }

    /// Mark a chunk as delivered (received + verified).
    pub fn mark_delivered(&mut self, idx: u32) {
        self.reorder.receive(idx);
        // Advance the in-order resume point.
        self.reorder.drain_in_order();
    }

    /// The resume point: the next in-order chunk to send.
    pub fn resume_point(&self) -> Option<ResumePoint> {
        // The next in-order index is `emitted_count` (all lower indices
        // have been delivered).
        let next = self.reorder.emitted_count();
        let total = self.reorder.total;
        if total == 0 {
            return None;
        }
        Some(ResumePoint {
            next_chunk: next,
            total_chunks: total,
        })
    }

    /// Whether the transfer is fully delivered.
    pub fn is_complete(&self) -> bool {
        self.reorder.complete()
    }

    /// The number of chunks delivered so far.
    pub fn delivered_count(&self) -> u32 {
        self.reorder.emitted_count()
    }

    /// A JSON-ish status string for the UI.
    pub fn status(&self) -> String {
        if let Some(rp) = self.resume_point() {
            format!("resume from chunk {}/{}", rp.next_chunk, rp.total_chunks)
        } else {
            "no resume point".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_point_advances_with_delivered() {
        let mut r = TransferResume::new(5);
        assert_eq!(r.resume_point().unwrap().next_chunk, 0);
        r.mark_delivered(0);
        r.mark_delivered(1);
        assert_eq!(r.resume_point().unwrap().next_chunk, 2);
        r.mark_delivered(2);
        r.mark_delivered(3);
        r.mark_delivered(4);
        assert_eq!(r.resume_point().unwrap().next_chunk, 5);
        assert!(r.is_complete());
    }

    #[test]
    fn delivered_count_tracks() {
        let mut r = TransferResume::new(4);
        r.mark_delivered(0);
        r.mark_delivered(1);
        assert_eq!(r.delivered_count(), 2);
        assert_eq!(r.resume_point().unwrap().next_chunk, 2);
    }

    #[test]
    fn status_reports_resume_point() {
        let mut r = TransferResume::new(3);
        r.mark_delivered(0);
        let s = r.status();
        assert!(s.contains("resume from chunk 1/3"), "got: {s}");
    }

    #[test]
    fn total_zero_no_resume_point() {
        let r = TransferResume::new(0);
        assert!(r.resume_point().is_none());
        assert_eq!(r.status(), "no resume point");
    }

    #[test]
    fn out_of_order_delivery_still_advances_in_order() {
        let mut r = TransferResume::new(4);
        r.mark_delivered(2);
        r.mark_delivered(0);
        // 1 is missing, so resume stays at chunk 1.
        assert_eq!(r.resume_point().unwrap().next_chunk, 1);
        r.mark_delivered(1);
        assert_eq!(r.resume_point().unwrap().next_chunk, 3);
    }
}
