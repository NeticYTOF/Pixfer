// pixfer web transfer slicer — split a byte payload into RS_K-sized
// chunks. Mirrors `pixfer-core/src/transfer_slicer.rs`.

import { RS_K } from "./frame_codec.js";

// A chunk of a sliced payload.
class Chunk {
  constructor(idx, data) {
    this.idx = idx;
    this.data = data;
  }
}

// Slice a payload into RS_K-sized chunks.
export function slicePayload(payload) {
  const n = payload.length;
  const chunks = Math.max(0, Math.ceil(n / RS_K));
  const out = [];
  for (let i = 0; i < chunks; i++) {
    const start = i * RS_K;
    const end = Math.min(start + RS_K, n);
    out.push(new Chunk(i, payload.slice(start, end)));
  }
  return out;
}

// The number of chunks a payload of `len` bytes requires.
export function chunkCount(len) {
  return Math.ceil(len / RS_K);
}