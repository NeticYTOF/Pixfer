//! Mock HALs used by the integration tests: a scripted camera that replays
//! captured frames, and an emitting display that records what was rendered.

use crate::hal::{CameraFrame, CameraHal, DisplayHal};

/// A camera that replays a scripted sequence of frames.
pub struct MockCamera {
    frames: Vec<CameraFrame>,
    idx: usize,
    opened: bool,
}

impl MockCamera {
    pub fn scripted(frames: Vec<CameraFrame>) -> Self {
        Self {
            frames,
            idx: 0,
            opened: false,
        }
    }
    pub fn push_frame(&mut self, f: CameraFrame) {
        self.frames.push(f);
    }
}

impl CameraHal for MockCamera {
    fn open(&mut self) -> Result<(), String> {
        self.opened = true;
        Ok(())
    }
    fn close(&mut self) {
        self.opened = false;
        self.idx = 0;
    }
    fn poll_frame(&mut self) -> Option<CameraFrame> {
        if !self.opened || self.idx >= self.frames.len() {
            return None;
        }
        let f = self.frames[self.idx].clone();
        self.idx += 1;
        Some(f)
    }
    fn lock_exposure(&mut self) -> bool {
        true
    }
}

/// A display that records rendered grids for assertions.
pub struct MockDisplay {
    opened: bool,
    pub rendered: Vec<Vec<u8>>,
}

impl MockDisplay {
    pub fn new() -> Self {
        Self {
            opened: false,
            rendered: Vec::new(),
        }
    }
}

impl DisplayHal for MockDisplay {
    fn open(&mut self) -> Result<(), String> {
        self.opened = true;
        Ok(())
    }
    fn close(&mut self) {
        self.opened = false;
    }
    fn render(&mut self, blocks: &[u8]) -> Result<(), String> {
        if !self.opened {
            return Err("display not open".into());
        }
        self.rendered.push(blocks.to_vec());
        Ok(())
    }
    fn supports_multicolor(&self) -> bool {
        true
    }
}
