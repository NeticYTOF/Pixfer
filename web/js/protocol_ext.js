// pixfer web extended protocol — a higher-level protocol helper that wraps
// the block grid codec with session framing.
// Mirrors `pixfer-core/src/protocol_ext.rs`.

import { newFrame, renderBlocks, decodeBlocks, FRAME_KIND } from "./core.js";
import { encodePayload, decodePayload } from "./frame_codec.js";

// A protocol-level error.
export class ProtocolErrorExt {
  constructor(stage, message) {
    this.stage = stage;
    this.message = message;
  }
}

// A higher-level protocol helper.
export class ProtocolExt {
  constructor(sessionId) {
    this.sessionId = sessionId;
  }

  // The session id.
  sessionIdValue() {
    return this.sessionId;
  }

  // Encode a control frame (ping/pong/ack/nak/end).
  encodeControl(kind, payload) {
    const frame = newFrame(kind, this.sessionId, 0, 1, Array.from(payload || []));
    return renderBlocks(frame);
  }

  // Encode a chunk frame carrying up to RS_K payload bytes.
  encodeChunk(chunkIdx, totalChunks, payload) {
    let encoded;
    try {
      encoded = encodePayload(Array.from(payload));
    } catch (e) {
      throw new ProtocolErrorExt("encode", String(e));
    }
    const frame = newFrame("Chunk", this.sessionId, chunkIdx, totalChunks, Array.from(encoded));
    return renderBlocks(frame);
  }

  // Decode a received block grid into a frame.
  decode(blocks) {
    const r = decodeBlocks(blocks);
    if (!r.ok) throw new ProtocolErrorExt("decode", r.error || "decode failed");
    return r.frame;
  }

  // Decode and return the payload bytes of a received frame.
  decodePayload(blocks) {
    const frame = this.decode(blocks);
    if (frame.kind !== "Chunk") {
      throw new ProtocolErrorExt("decode_payload", "frame is not a chunk");
    }
    try {
      return decodePayload(Array.from(frame.payload));
    } catch (e) {
      throw new ProtocolErrorExt("decode_payload", String(e));
    }
  }
}