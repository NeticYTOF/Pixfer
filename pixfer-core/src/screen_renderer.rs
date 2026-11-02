//! Screen renderer: paints the 270x480 color-block grid onto a pixel buffer
//! for the display HAL. (Hilariously over-engineered: a "renderer" for a
//! screen that just blinks 8 colors.)

use crate::color_palette::color_for_value;

/// A flat RGB pixel buffer for the screen.
#[derive(Debug, Clone)]
pub struct ScreenBuffer {
    pub width: u32,
    pub height: u32,
    /// RGB triples, row-major.
    pub pixels: Vec<(u8, u8, u8)>,
}

impl ScreenBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let count = (width * height) as usize;
        Self {
            width,
            height,
            pixels: vec![(0, 0, 0); count],
        }
    }
}

/// Renders a 3-bit block grid onto a screen buffer at a given block size.
pub struct ScreenRenderer {
    pub block_px: u32,
}

impl ScreenRenderer {
    pub fn new(block_px: u32) -> Self {
        Self { block_px }
    }

    /// Paint `blocks` (each a 3-bit value) into `buf`, block `block_px` squared.
    pub fn render(&self, buf: &mut ScreenBuffer, blocks: &[u8]) {
        let bw = (buf.width / self.block_px) as usize;
        let bh = (buf.height / self.block_px) as usize;
        for y in 0..bh {
            for x in 0..bw {
                let idx = y * bw + x;
                if idx >= blocks.len() {
                    continue;
                }
                let v = blocks[idx] & 0b111;
                let c = color_for_value(v);
                let px = self.block_px as usize;
                for py in 0..px {
                    for pxx in 0..px {
                        let yy = y as usize * px + py;
                        let xx = x as usize * px + pxx;
                        if (yy as u32) < buf.height && (xx as u32) < buf.width {
                            buf.pixels[yy * buf.width as usize + xx] = (c.r, c.g, c.b);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_palette::Color;

    #[test]
    fn buffer_allocates_pixels() {
        let buf = ScreenBuffer::new(4, 4);
        assert_eq!(buf.pixels.len(), 16);
    }

    #[test]
    fn renderer_paints_blocks() {
        let mut buf = ScreenBuffer::new(8, 8);
        let r = ScreenRenderer::new(4);
        let blocks = [0u8, 1, 2, 3];
        r.render(&mut buf, &blocks);
        // Top-left block (4x4) should be palette color 0.
        let palette = Color::palette();
        let expected = palette[0];
        assert_eq!(buf.pixels[0], (expected.r, expected.g, expected.b));
        // Second block (x=4..7, y=0..3) should be palette color 1.
        let expected1 = palette[1];
        assert_eq!(
            buf.pixels[0 * 8 + 4],
            (expected1.r, expected1.g, expected1.b)
        );
    }

    #[test]
    fn renderer_clamps_grid() {
        let mut buf = ScreenBuffer::new(8, 8);
        let r = ScreenRenderer::new(4);
        // More blocks than grid cells -> extra ignored.
        let blocks = vec![0u8, 1, 2, 3, 4, 5];
        r.render(&mut buf, &blocks);
        // No panic, buffer fully painted within grid.
        assert_eq!(buf.pixels.len(), 64);
    }
}
