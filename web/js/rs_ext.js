// pixfer web extended Reed-Solomon — a higher-level RS wrapper with batch
// encode/verify helpers.
// Mirrors `pixfer-core/src/rs_ext.rs`.

import { RS_K, RS_N, RS_PARITY } from "./core.js";

// A batch RS operation result.
export class RsBatchResult {
  constructor({ ok = true, errors = [] } = {}) {
    this.ok = ok;
    this.errors = errors;
  }
}

// A higher-level RS helper.
export class RSExt {
  constructor() {}

  // Encode a message into a full RS(255,223) codeword (parity-first).
  // The web mirror computes parity as a structural placeholder: the real
  // GF(256) math lives in the Rust core; here we pad to RS_N.
  encode(msg) {
    if (msg.length !== RS_K) {
      throw new Error("bad length");
    }
    const cw = new Array(RS_N).fill(0);
    cw.fill(msg, RS_PARITY);
    return cw;
  }

  // Verify a codeword: re-encode the data portion and compare parity.
  verify(codeword) {
    if (codeword.length !== RS_N) {
      throw new Error("bad length");
    }
    const parity = codeword.slice(0, RS_PARITY);
    const data = codeword.slice(RS_PARITY);
    const expected = this.encode(data);
    const expectedParity = expected.slice(0, RS_PARITY);
    if (JSON.stringify(parity) !== JSON.stringify(expectedParity)) {
      throw new Error("check failed");
    }
  }

  // Batch-encode a set of messages; returns encoded codewords + a result.
  encodeBatch(msgs) {
    const out = [];
    const errors = [];
    msgs.forEach((m, i) => {
      try {
        out.push(this.encode(m));
      } catch (e) {
        errors.push(`msg ${i}: ${e.message}`);
      }
    });
    return { codewords: out, result: new RsBatchResult({ ok: errors.length === 0, errors }) };
  }

  // The number of RS parity bytes.
  parityBytes() {
    return RS_PARITY;
  }
}