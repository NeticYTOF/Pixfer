//! Display HAL stub: the native counterpart of the browser's `<canvas>`
//! emitter.

use crate::hal::DisplayHal;

pub struct StubDisplayHal {
    opened: bool,
    last_blocks: Vec<u8>,
}

impl StubDisplayHal {
    pub fn new() -> Self {
        Self {
            opened: false,
            last_blocks: Vec::new(),
        }
    }
    pub fn last(&self) -> &[u8] {
        &self.last_blocks
    }
}

impl DisplayHal for StubDisplayHal {
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
        self.last_blocks = blocks.to_vec();
        Ok(())
    }

    fn supports_multicolor(&self) -> bool {
        true
    }
}
