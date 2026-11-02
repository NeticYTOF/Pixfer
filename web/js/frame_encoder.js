// pixfer web frame encoder — renders a Frame into the 270x480 block
// grid. Mirrors `pixfer-core/src/frame_encoder.rs`.

import { BLOCKS_PER_FRAME } from "./frame_codec.js";
import * as core from "./core.js";

// A frame encoder that renders a Frame into the 270x480 block grid.
// (Hilariously over-engineered: a "renderer" for a link whose whole
// "network" is one screen and one camera.)
export class FrameEncoder {
  constructor(dwellMarginMs = 0) {
    this.dwellMarginMs = dwellMarginMs;
  }

  // Encode a frame into the full block grid.
  // frame: { kind, sessionId, chunkIdx, totalChunks, payload }
  encode(frame) {
    // renderBlocks returns a fresh Uint8Array of the full grid.
    return core.renderBlocks(frame);
  }

  // The number of blocks in the encoded output.
  blockCount() {
    return BLOCKS_PER_FRAME;
  }

  // The configured dwell margin (ms).
  dwellMargin() {
    return this.dwellMarginMs;
  }
}