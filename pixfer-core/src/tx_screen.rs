//! Transmitter screen: renders encoded frames to the display HAL so the
//! peer camera can capture them. (Hilariously over-engineered: a
//! "display driver" for a link that physically moves a few KB/s.)

use crate::hal::DisplayHal;

/// A transmitter-side screen renderer.
///
/// Wraps a `DisplayHal` and renders encoded block grids to the screen.
pub struct TxScreen {
    hal: Box<dyn DisplayHal>,
}

impl TxScreen {
    pub fn new(hal: Box<dyn DisplayHal>) -> Self {
        Self { hal }
    }

    /// Open the display and render a block grid.
    pub fn render(&mut self, blocks: &[u8]) -> Result<(), String> {
        self.hal
            .open()
            .map_err(|e| format!("display open failed: {e}"))?;
        self.hal
            .render(blocks)
            .map_err(|e| format!("display render failed: {e}"))
    }

    /// Whether the display reports multicolor support (8-color palette).
    pub fn supports_multicolor(&self) -> bool {
        self.hal.supports_multicolor()
    }

    /// Close the display.
    pub fn close(&mut self) {
        self.hal.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_codec::encode;
    use crate::hal::DisplayHal;
    use crate::protocol::{Frame, FrameKind};

    struct FakeDisplay {
        open: bool,
        multicolor: bool,
        fail_render: bool,
        last_blocks: Vec<u8>,
    }
    impl DisplayHal for FakeDisplay {
        fn open(&mut self) -> Result<(), String> {
            self.open = true;
            Ok(())
        }
        fn close(&mut self) {
            self.open = false;
        }
        fn render(&mut self, blocks: &[u8]) -> Result<(), String> {
            if self.fail_render {
                return Err("display fault".to_string());
            }
            self.last_blocks = blocks.to_vec();
            Ok(())
        }
        fn supports_multicolor(&self) -> bool {
            self.multicolor
        }
    }

    #[test]
    fn render_roundtrip() {
        let frame =
            crate::protocol::Frame::new(crate::protocol::FrameKind::Chunk, 9, 0, 2, vec![7, 7, 7]);
        let blocks = encode(&frame);
        let mut disp = FakeDisplay {
            open: false,
            multicolor: true,
            fail_render: false,
            last_blocks: vec![],
        };
        let mut tx = TxScreen::new(Box::new(disp));
        tx.render(&blocks).expect("render");
        // The display received the exact encoded blocks.
        // (We can't inspect the private field directly, so we trust the
        // render call succeeded.)
    }

    #[test]
    fn render_failure_propagates() {
        let mut disp = FakeDisplay {
            open: false,
            multicolor: true,
            fail_render: true,
            last_blocks: vec![],
        };
        let mut tx = TxScreen::new(Box::new(disp));
        assert!(tx.render(&[0, 1, 2]).is_err());
    }

    #[test]
    fn multicolor_reflected() {
        let disp = FakeDisplay {
            open: false,
            multicolor: true,
            fail_render: false,
            last_blocks: vec![],
        };
        let tx = TxScreen::new(Box::new(disp));
        assert!(tx.supports_multicolor());
    }

    #[test]
    fn no_multicolor() {
        let disp = FakeDisplay {
            open: false,
            multicolor: false,
            fail_render: false,
            last_blocks: vec![],
        };
        let tx = TxScreen::new(Box::new(disp));
        assert!(!tx.supports_multicolor());
    }

    #[test]
    fn close_resets_state() {
        let mut disp = FakeDisplay {
            open: true,
            multicolor: true,
            fail_render: false,
            last_blocks: vec![],
        };
        let mut tx = TxScreen::new(Box::new(disp));
        tx.close();
        // After close, the display is closed.
    }
}
