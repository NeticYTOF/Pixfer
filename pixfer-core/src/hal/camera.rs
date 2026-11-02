//! Concrete camera HAL sketches (documented, not wired to a real driver in
//! this repo — the browser demo uses `getUserMedia`; a native build would
//! implement `CameraHal` against the platform camera API).

use crate::hal::{CameraFrame, CameraHal};

/// Placeholder driver stub showing the shape a native camera binding would
/// take (open, readout, exposure lock). Kept tiny on purpose.
pub struct StubCameraHal {
    opened: bool,
    frames: Vec<CameraFrame>,
    seq: u64,
}

impl StubCameraHal {
    pub fn new() -> Self {
        Self {
            opened: false,
            frames: Vec::new(),
            seq: 0,
        }
    }

    /// Monotonic open counter (useful for tests and diagnostics).
    pub fn last_seq(&self) -> u64 {
        self.seq
    }
}

impl CameraHal for StubCameraHal {
    fn open(&mut self) -> Result<(), String> {
        self.opened = true;
        self.seq += 1;
        Ok(())
    }

    fn close(&mut self) {
        self.opened = false;
        self.frames.clear();
    }

    fn poll_frame(&mut self) -> Option<CameraFrame> {
        self.frames.pop()
    }

    fn lock_exposure(&mut self) -> bool {
        true
    }
}
