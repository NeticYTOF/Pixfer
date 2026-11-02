// pixfer web transfer checksum — FNV-1a 64-bit file verification.
// Mirrors `pixfer-core/src/transfer_checksum.rs`.

import { fnv1a64 } from "./dedup.js";

// The result of a transfer checksum verification.
class TransferChecksumResult {
  constructor(matched, expected, actual) {
    this.matched = matched;
    this.expected = expected;
    this.actual = actual;
  }
}

// Compute the FNV-1a 64-bit checksum of a byte slice (Buffer or Uint8Array).
export function fnv1a64Checksum(data) {
  const buf = Buffer.isBuffer(data) ? data : Buffer.from(data);
  return fnv1a64(buf);
}

// Verify that a received file matches the sender's checksum.
export function verifyTransfer(expected, received) {
  const actual = fnv1a64Checksum(received);
  return new TransferChecksumResult(expected === actual, expected, actual);
}

// Compute a transfer checksum from a set of chunk payloads (Buffers).
export function transferChecksum(chunks) {
  const stream = Buffer.concat(chunks);
  return fnv1a64Checksum(stream);
}