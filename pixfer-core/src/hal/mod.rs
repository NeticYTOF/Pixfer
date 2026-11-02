//! Hardware-abstraction layer for the light link: `CameraHal` and
//! `DisplayHal` traits, with mock implementations for tests and the
//! browser demo. This is the seam where a real Rust product would bind
//! to actual camera/display drivers.

pub mod camera;
pub mod display;
pub mod mock;

/// A camera frame: raw block-grid sample + metadata.
#[derive(Debug, Clone)]
pub struct CameraFrame {
    pub blocks: Vec<u8>,
    pub ts_ms: u64,
    pub seq: u64,
}

/// The camera HAL: open/close + pull the latest frame.
pub trait CameraHal: Send {
    fn open(&mut self) -> Result<(), String>;
    fn close(&mut self);
    fn poll_frame(&mut self) -> Option<CameraFrame>;
    /// Attempt to lock auto-exposure / auto-gain (no-op where unsupported).
    fn lock_exposure(&mut self) -> bool;
}

/// The display HAL: render a block grid to the screen.
pub trait DisplayHal: Send {
    fn open(&mut self) -> Result<(), String>;
    fn close(&mut self);
    fn render(&mut self, blocks: &[u8]) -> Result<(), String>;
    /// True if the display supports the full 8-color palette (multicolor).
    fn supports_multicolor(&self) -> bool;
}
