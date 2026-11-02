// pixfer web link fingerprint — stable 64-bit link identifier.
// Mirrors `pixfer-core/src/link_fingerprint.rs`.
// Uses the FNV-1a 64-bit hash from dedup.js (BigInt-based).

import { fnv1a64 } from "./dedup.js";

// Compute the link fingerprint from a session's calibration signature.
export function computeLinkFingerprint(sessionId, totalChunks) {
  const input = new Uint8Array(2 + 4 + "pixfer-link".length);
  const view = new DataView(input.buffer);
  view.setUint16(0, sessionId, true);
  view.setUint32(2, totalChunks, true);
  const tag = "pixfer-link";
  for (let i = 0; i < tag.length; i++) {
    input[6 + i] = tag.charCodeAt(i);
  }
  return { value: fnv1a64(Buffer.from(input)) };
}

// Two links are the same if their fingerprints match.
export function sameLink(a, b) {
  return a.value === b.value;
}