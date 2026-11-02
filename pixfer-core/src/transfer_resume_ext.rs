//! Extended transfer resume: a resumable transfer state that tracks which
//! chunks have been delivered, so a dropped light link can be continued.
//! (Hilariously over-engineered: a "resume point" for a transfer that takes
//! minutes to move a few KB.)

/// A resume point: which chunk index to continue from.
#[derive(Debug, Clone, Copy)]
pub struct ResumePointExt {
    /// The next chunk index to send.
    pub next_chunk: u32,
    /// The total chunk count.
    pub total_chunks: u32,
    /// The number of chunks already delivered.
    pub delivered: u32,
}

/// Tracks resumable transfer state.
#[derive(Debug)]
pub struct TransferResumeExt {
    next_chunk: u32,
    total_chunks: u32,
    delivered: u32,
}

impl TransferResumeExt {
    pub fn new(total_chunks: u32) -> Self {
        Self {
            next_chunk: 0,
            total_chunks,
            delivered: 0,
        }
    }

    /// Mark chunk `idx` as delivered and advance the resume point.
    pub fn mark_delivered(&mut self, idx: u32) {
        if idx < self.total_chunks && idx == self.next_chunk {
            self.delivered = self.delivered.saturating_add(1);
            self.next_chunk = self.next_chunk.saturating_add(1);
        }
    }

    /// The current resume point.
    pub fn resume_point(&self) -> ResumePointExt {
        ResumePointExt {
            next_chunk: self.next_chunk,
            total_chunks: self.total_chunks,
            delivered: self.delivered,
        }
    }

    /// Whether the transfer is complete.
    pub fn is_complete(&self) -> bool {
        self.next_chunk >= self.total_chunks
    }

    /// The number of chunks remaining.
    pub fn remaining(&self) -> u32 {
        self.total_chunks.saturating_sub(self.next_chunk)
    }

    /// The status string.
    pub fn status(&self) -> &'static str {
        if self.total_chunks == 0 {
            "empty"
        } else if self.is_complete() {
            "complete"
        } else {
            "in_progress"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_chunk_zero() {
        let r = TransferResumeExt::new(5);
        let p = r.resume_point();
        assert_eq!(p.next_chunk, 0);
        assert_eq!(p.total_chunks, 5);
        assert_eq!(p.delivered, 0);
        assert_eq!(r.status(), "in_progress");
    }

    #[test]
    fn advances_with_in_order_delivery() {
        let mut r = TransferResumeExt::new(5);
        r.mark_delivered(0);
        assert_eq!(r.resume_point().next_chunk, 1);
        r.mark_delivered(1);
        assert_eq!(r.resume_point().delivered, 2);
        assert_eq!(r.remaining(), 3);
    }

    #[test]
    fn out_of_order_ignored() {
        let mut r = TransferResumeExt::new(5);
        // Delivering chunk 2 first does nothing (we're at 0).
        r.mark_delivered(2);
        assert_eq!(r.resume_point().next_chunk, 0);
        assert_eq!(r.resume_point().delivered, 0);
    }

    #[test]
    fn completes_when_all_delivered() {
        let mut r = TransferResumeExt::new(3);
        for i in 0..3 {
            r.mark_delivered(i);
        }
        assert!(r.is_complete());
        assert_eq!(r.remaining(), 0);
        assert_eq!(r.status(), "complete");
    }

    #[test]
    fn zero_total_is_empty() {
        let r = TransferResumeExt::new(0);
        assert_eq!(r.status(), "empty");
        assert!(r.is_complete());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn resume_point_tracks_delivered() {
        let mut r = TransferResumeExt::new(4);
        r.mark_delivered(0);
        r.mark_delivered(1);
        let p = r.resume_point();
        assert_eq!(p.delivered, 2);
        assert_eq!(p.next_chunk, 2);
        assert_eq!(p.total_chunks, 4);
    }
}
