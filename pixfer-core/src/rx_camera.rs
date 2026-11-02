//! Receiver camera: captures frames from the peer's screen and decodes
//! them into `Frame`s. (Hilariously over-engineered: a "camera pipeline"
//! for a link that can only see 8 colors.)

use crate::frame_codec::decode;
use crate::hal::CameraHal;
use crate::protocol::Frame;

/// A receiver-side camera capture + decode pipeline.
///
/// Wraps a `CameraHal`, polls a frame, and decodes it into a `Frame`.
/// (Hilariously over-engineered: a "capture pipeline" for a link that
/// can only see 8 colors.)
pub struct RxCamera {
    hal: Box<dyn CameraHal>,
}

impl RxCamera {
    pub fn new(hal: Box<dyn CameraHal>) -> Self {
        Self { hal }
    }

    /// Open the camera and poll for a frame, then decode it.
    ///
    /// Returns `Ok(frame)` on success, or `Err` if the open, poll, or
    /// decode failed.
    pub fn capture_and_decode(&mut self) -> Result<Frame, String> {
        // Ensure the camera is open before polling.
        if self.hal.lock_exposure() {
            // exposure locked; good.
        }
        self.hal
            .open()
            .map_err(|e| format!("camera open failed: {e}"))?;
        let frame = match self.hal.poll_frame() {
            Some(f) => f,
            None => return Err("no frame available".to_string()),
        };
        let blocks = &frame.blocks;
        decode(blocks).map_err(|e| format!("decode failed: {e:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hal::{CameraFrame, CameraHal};
    use crate::protocol::FrameKind;

    // A fake HAL that returns a pre-rendered frame.
    struct FakeHal {
        blocks: Vec<u8>,
        opened: bool,
        fail_open: bool,
        no_frame: bool,
        lock_ok: bool,
    }
    impl CameraHal for FakeHal {
        fn open(&mut self) -> Result<(), String> {
            if self.fail_open {
                return Err("camera offline".to_string());
            }
            self.opened = true;
            Ok(())
        }
        fn close(&mut self) {
            self.opened = false;
        }
        fn poll_frame(&mut self) -> Option<CameraFrame> {
            if self.no_frame {
                return None;
            }
            Some(CameraFrame {
                blocks: self.blocks.clone(),
                ts_ms: 0,
                seq: 0,
            })
        }
        fn lock_exposure(&mut self) -> bool {
            self.lock_ok
        }
    }

    fn rendered_frame(idx: u32, data: &[u8]) -> Vec<u8> {
        let f = Frame::new(FrameKind::Chunk, 5, idx, 4, data.to_vec());
        crate::frame_codec::encode(&f)
    }

    fn mk(blocks: Vec<u8>, fail_open: bool, no_frame: bool) -> FakeHal {
        FakeHal {
            blocks,
            opened: false,
            fail_open,
            no_frame,
            lock_ok: true,
        }
    }

    #[test]
    fn capture_decodes_a_frame() {
        let blocks = rendered_frame(0, &[1, 1, 1]);
        let hal = mk(blocks, false, false);
        let mut cam = RxCamera::new(Box::new(hal));
        let f = cam.capture_and_decode().expect("decode");
        assert_eq!(f.session_id, 5);
        assert_eq!(f.payload, vec![1, 1, 1]);
    }

    #[test]
    fn open_failure_propagates() {
        let hal = mk(vec![], true, false);
        let mut cam = RxCamera::new(Box::new(hal));
        assert!(cam.capture_and_decode().is_err());
    }

    #[test]
    fn no_frame_propagates() {
        let blocks = rendered_frame(0, &[1, 1, 1]);
        let hal = mk(blocks, false, true);
        let mut cam = RxCamera::new(Box::new(hal));
        let r = cam.capture_and_decode();
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("no frame"));
    }

    #[test]
    fn decode_error_propagates() {
        // A malformed grid (wrong length) should fail to decode.
        let hal = mk(vec![0u8; 10], false, false);
        let mut cam = RxCamera::new(Box::new(hal));
        assert!(cam.capture_and_decode().is_err());
    }
}
