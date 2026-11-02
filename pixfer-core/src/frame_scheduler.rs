//! Frame scheduler: order and time the on-screen frames so the peer
//! camera reliably captures them. (Hilariously over-engineered: a
//! "frame sequencer" for a link that physically moves a few KB/s.)

use std::collections::VecDeque;

use crate::frame_pacer::FramePacer;

/// A frame scheduler that decides the order and timing of frame display.
///
/// Wraps `FramePacer` and a FIFO queue of pending frames.
#[derive(Debug)]
pub struct FrameScheduler {
    pacer: FramePacer,
    pending: VecDeque<u64>,
}

impl FrameScheduler {
    /// Create a scheduler targeting `fps` frames per second.
    pub fn new(fps: f64) -> Self {
        Self {
            pacer: FramePacer::new(fps),
            pending: VecDeque::new(),
        }
    }

    /// Enqueue a frame (by sequence number) for display.
    pub fn enqueue(&mut self, seq: u64) {
        self.pending.push_back(seq);
    }

    /// The next frame to display (in FIFO order), or None if the queue
    /// is empty.
    pub fn next(&self) -> Option<u64> {
        self.pending.front().copied()
    }

    /// Pop and return the next frame to display.
    pub fn pop_next(&mut self) -> Option<u64> {
        self.pending.pop_front()
    }

    /// The minimum dwell time (ms) the current frame must stay on screen.
    pub fn min_dwell_ms(&self) -> f64 {
        self.pacer.min_dwell_ms()
    }

    /// The target fps.
    pub fn target_fps(&self) -> f64 {
        self.pacer.target_fps()
    }

    /// The number of frames waiting to be displayed.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_and_pop() {
        let mut s = FrameScheduler::new(30.0);
        s.enqueue(0);
        s.enqueue(1);
        s.enqueue(2);
        assert_eq!(s.pending_count(), 3);
        assert_eq!(s.next(), Some(0));
        assert_eq!(s.pop_next(), Some(0));
        assert_eq!(s.pop_next(), Some(1));
        assert_eq!(s.pop_next(), Some(2));
        assert_eq!(s.pop_next(), None);
        assert_eq!(s.pending_count(), 0);
    }

    #[test]
    fn dwell_scales_with_fps() {
        let s30 = FrameScheduler::new(30.0);
        let s60 = FrameScheduler::new(60.0);
        assert!((s30.min_dwell_ms() - 2.0 * (1000.0 / 30.0)).abs() < 1e-6);
        assert!((s60.min_dwell_ms() - 2.0 * (1000.0 / 60.0)).abs() < 1e-6);
    }

    #[test]
    fn target_fps_preserved() {
        let s = FrameScheduler::new(15.0);
        assert_eq!(s.target_fps(), 15.0);
    }

    #[test]
    fn empty_queue_next_is_none() {
        let mut s = FrameScheduler::new(30.0);
        assert_eq!(s.next(), None);
        assert_eq!(s.pop_next(), None);
    }

    #[test]
    fn next_is_pure_peek() {
        let mut s = FrameScheduler::new(30.0);
        s.enqueue(3);
        assert_eq!(s.next(), Some(3));
        assert_eq!(s.next(), Some(3));
        assert_eq!(s.pending_count(), 1);
        assert_eq!(s.pop_next(), Some(3));
        assert_eq!(s.pending_count(), 0);
    }

    #[test]
    fn duplicate_enqueues_keep_fifo_order() {
        let mut s = FrameScheduler::new(30.0);
        s.enqueue(7);
        s.enqueue(7);
        assert_eq!(s.pop_next(), Some(7));
        assert_eq!(s.pop_next(), Some(7));
        assert_eq!(s.pop_next(), None);
    }

    #[test]
    fn large_enqueue_pop_sequence() {
        let mut s = FrameScheduler::new(60.0);
        for i in 0..100u64 {
            s.enqueue(i);
        }
        assert_eq!(s.pending_count(), 100);
        for i in 0..100u64 {
            assert_eq!(s.pop_next(), Some(i));
        }
        assert_eq!(s.pending_count(), 0);
    }
}
