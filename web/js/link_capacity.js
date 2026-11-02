// pixfer web link capacity model — estimate bytes/sec a light link can
// move. Mirrors `pixfer-core/src/link_capacity.rs`.

import { RS_K } from "./frame_codec.js";

// A capacity estimate for a light link.
class CapacityEstimate {
  constructor(bps, fps, payloadBytes) {
    this.bps = bps;
    this.fps = fps;
    this.payloadBytes = payloadBytes;
  }
}

// Estimate the link capacity (bytes/sec) from the camera fps.
// Each camera frame captures one on-screen frame carrying RS_K payload
// bytes, so the achievable throughput is roughly `fps * RS_K`.
export function estimateCapacity(fps) {
  return new CapacityEstimate(fps * RS_K, fps, RS_K);
}

// A sanity check: the estimate must be positive and scale with fps.
export function isPlausible(est) {
  return est.bps > 0 && est.fps > 0 && est.payloadBytes > 0;
}