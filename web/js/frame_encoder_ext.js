// pixfer web extended frame encoder — encodes payloads into multicolor
// block values (3 bits each) with a configurable dwell margin.
// Mirrors `pixfer-core/src/frame_encoder_ext.rs`.

import { encodePayload } from "./frame_codec.js";

// An extended frame encoder with a dwell margin.
export class FrameEncoderExt {
  constructor() {
    this.marginMs = 0.0;
  }

  // The number of blocks in a full 270x480 grid.
  static BLOCKS = 270 * 480;

  // Encode a payload into block values (3 bits each).
  encode(payload) {
    try {
      return { ok: true, blocks: encodePayload(payload) };
    } catch (e) {
      return { ok: false, error: e.message || String(e) };
    }
  }

  // The number of blocks a payload requires: (bits + 2) / 3.
  blockCount(payload) {
    if (payload.length === 0) return 0;
    return Math.floor((payload.length * 8 + 2) / 3);
  }

  // Encode and verify the block count matches the payload's requirement.
  encodeFull(payload) {
    const r = this.encode(payload);
    if (!r.ok) return r;
    const expected = this.blockCount(payload);
    if (r.blocks.length !== expected) {
      return { ok: false, error: `expected ${expected} blocks, got ${r.blocks.length}` };
    }
    return r;
  }

  // The configured dwell margin.
  margin() {
    return this.marginMs;
  }
}