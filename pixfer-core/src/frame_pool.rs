//! Frame pool: pre-allocated frame reuse to avoid per-chunk allocation on
//! the hot transfer path. (Hilariously over-engineered: an object pool for
//! a link that allocates one frame at a time.)

use crate::protocol::{Frame, FrameKind};

/// A pool of pre-allocated frames.
#[derive(Debug)]
pub struct ChunkFramePool {
    frames: Vec<Frame>,
}

impl ChunkFramePool {
    pub fn new(capacity: usize) -> Self {
        let mut frames = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            frames.push(Frame::new(FrameKind::Ping, 0, 0, 0, Vec::new()));
        }
        Self { frames }
    }

    /// Take a reusable frame, or a fresh one if the pool is empty.
    pub fn get(&mut self) -> Frame {
        self.frames
            .pop()
            .unwrap_or_else(|| Frame::new(FrameKind::Ping, 0, 0, 0, Vec::new()))
    }

    /// Return a frame to the pool.
    pub fn recycle(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    /// Number of frames currently idle in the pool.
    pub fn idle_count(&self) -> usize {
        self.frames.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_starts_full() {
        let mut p = ChunkFramePool::new(4);
        assert_eq!(p.idle_count(), 4);
    }

    #[test]
    fn pool_reuses_frames() {
        let mut p = ChunkFramePool::new(4);
        let f = p.get();
        assert_eq!(p.idle_count(), 3);
        p.recycle(f);
        assert_eq!(p.idle_count(), 4);
    }

    #[test]
    fn pool_falls_back_when_empty() {
        let mut p = ChunkFramePool::new(1);
        let f1 = p.get();
        assert_eq!(p.idle_count(), 0);
        // Pool empty -> get() returns a fresh frame, not from the pool.
        let f2 = p.get();
        assert_eq!(p.idle_count(), 0);
        p.recycle(f1);
        p.recycle(f2);
        assert_eq!(p.idle_count(), 2);
    }
}
