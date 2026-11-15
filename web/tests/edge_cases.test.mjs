// Edge-case regression tests: short last chunks (which encode to smaller
// QR versions v2..v10 and must still decode), frame-metadata validation
// guards, the Done frame kind, and control-frame round-trips.
// Run: node tests/edge_cases.test.mjs (from web/)

import * as core from "../js/core.js";
import { qrEncode } from "../js/qr.js";
import { qrDecodeFromPixels } from "../js/qrdecode.js";
import { suggestFlashPhase } from "../js/rx.js";

let pass = 0;
let fail = 0;
const failures = [];

function check(name, cond, detail) {
  if (cond) {
    pass++;
    console.log(`ok - ${name}`);
  } else {
    fail++;
    failures.push(name);
    console.log(`FAIL - ${name}${detail ? ` (${detail})` : ""}`);
  }
}

function qrPixels(qr, pxPerModule = 8) {
  const n = qr.size;
  const w = n * pxPerModule;
  const h = n * pxPerModule;
  const data = new Uint8Array(w * h * 4);
  for (let r = 0; r < n; r++) {
    for (let c = 0; c < n; c++) {
      const col = qr.data[r * n + c] ? 0 : 255;
      for (let dy = 0; dy < pxPerModule; dy++) {
        for (let dx = 0; dx < pxPerModule; dx++) {
          const idx = ((r * pxPerModule + dy) * w + (c * pxPerModule + dx)) * 4;
          data[idx] = col;
          data[idx + 1] = col;
          data[idx + 2] = col;
          data[idx + 3] = 255;
        }
      }
    }
  }
  return { data, w, h };
}

function roundTrip(frame) {
  const qr = qrEncode(frame);
  const { data, w, h } = qrPixels(qr);
  return { qr, dec: qrDecodeFromPixels(data, w, h) };
}

// ---------- 1) every last-chunk size class decodes ----------
// Full chunk frames are 246 B -> v11. The short LAST chunk can land on
// any smaller version down to v2 — the decoder's fallback list must
// cover all of them or transfers never complete.
{
  const N = 6; // chunkIdx 5 is the last chunk
  const sizes = [1, 30, 45, 80, 92, 120, 170, 190, 223];
  for (const pl of sizes) {
    const payload = new Uint8Array(pl);
    for (let i = 0; i < pl; i++) payload[i] = (i * 17 + 3) & 0xff;
    const frame = core.newFrame("Chunk", 4321, N - 1, N, payload);
    const { qr, dec } = roundTrip(frame);
    check(
      `last chunk ${pl} B (v${qr.version}) decodes`,
      dec.ok,
      dec.error
    );
    check(
      `last chunk ${pl} B payload is byte-exact`,
      dec.ok && dec.frame.payload.length === pl && dec.frame.payload.every((v, i) => v === payload[i])
    );
  }

  // a full mid chunk still round-trips (control, v11)
  const full = new Uint8Array(core.RS_K).fill(0x42);
  const { dec } = roundTrip(core.newFrame("Chunk", 4321, 2, N, full));
  check("full mid chunk (v11) decodes", dec.ok, dec.error);

  // a whole tiny file is ONE short chunk (idx 0 == last chunk)
  const tiny = new Uint8Array(17);
  for (let i = 0; i < 17; i++) tiny[i] = (i * 29 + 1) & 0xff;
  const { qr: q1, dec: d1 } = roundTrip(core.newFrame("Chunk", 999, 0, 1, tiny));
  check(`single-chunk tiny file (v${q1.version}) decodes`, d1.ok, d1.error);
  check(
    "tiny file payload is byte-exact",
    d1.ok && d1.frame.totalChunks === 1 && d1.frame.payload.every((v, i) => v === tiny[i])
  );
}

// ---------- 2) frame metadata validation ----------
{
  check("validChunkMeta: full mid chunk", core.validChunkMeta(0, 5, core.RS_K));
  check("validChunkMeta: full second-to-last chunk", core.validChunkMeta(3, 5, core.RS_K));
  check("validChunkMeta: short last chunk", core.validChunkMeta(4, 5, 1));
  check("validChunkMeta: short last chunk (RS_K)", core.validChunkMeta(4, 5, core.RS_K));
  check("validChunkMeta: accepts big preset mid chunks (643 B)", core.validChunkMeta(1, 5, 643));
  check("validChunkMeta: accepts max preset mid chunks (974 B)", core.validChunkMeta(1, 5, 974));
  check("validChunkMeta: rejects chunkIdx == total", !core.validChunkMeta(5, 5, core.RS_K));
  check("validChunkMeta: rejects negative index", !core.validChunkMeta(-1, 5, core.RS_K));
  check("validChunkMeta: rejects zero payload", !core.validChunkMeta(0, 5, 0));
  check("validChunkMeta: rejects oversize payload", !core.validChunkMeta(0, 5, core.MAX_PAYLOAD_BYTES + 1));
  check("validChunkMeta: rejects zero total", !core.validChunkMeta(0, 0, core.RS_K));
  check(
    "validChunkMeta: rejects allocation bomb (1e12 chunks)",
    !core.validChunkMeta(0, 1e12, core.RS_K)
  );
  check(
    "validChunkMeta: rejects non-integer index",
    !core.validChunkMeta(0.5, 5, core.RS_K)
  );
  check(
    "maxChunkCount bounds MAX_TRANSFER_BYTES",
    core.maxChunkCount() === Math.ceil(core.MAX_TRANSFER_BYTES / core.RS_K) &&
      core.maxChunkCount() * core.RS_K >= core.MAX_TRANSFER_BYTES
  );
}

// ---------- 3) control frames: Done kind + Ping/Pong ----------
{
  const { dec } = roundTrip(core.newFrame("Done", 777, 0, 3, null));
  check("Done frame round-trips", dec.ok, dec.error);
  check(
    "Done frame fields recovered",
    dec.ok && dec.frame.kind === "Done" && dec.frame.sessionId === 777 && dec.frame.totalChunks === 3
  );

  const p = roundTrip(core.newFrame("Ping", 4242, 0, 1234, null));
  check("Ping with a big totalChunks round-trips", p.dec.ok, p.dec.error);
  check(
    "Ping totalChunks recovered",
    p.dec.ok && p.dec.frame.totalChunks === 1234
  );

  const g = roundTrip(core.newFrame("Pong", 1234, 0, 1234, null));
  check("Pong round-trips", g.dec.ok, g.dec.error);

  // a corrupt mid-stream Chunk (index past the total) still ENCODES — the
  // receiver's validChunkMeta is what drops it; verify the guard agrees
  check(
    "guard drops decoded out-of-range chunk",
    !core.validChunkMeta(1234, 1234, 0)
  );
}

// ---------- 4) flash-phase suggestion (receiver guides the sender) ----------
{
  check("suggestFlashPhase: 30 fps stays at the 200 default", suggestFlashPhase(30) === 200);
  check("suggestFlashPhase: 15 fps → 300", suggestFlashPhase(15) === 300);
  check("suggestFlashPhase: 10 fps → 400", suggestFlashPhase(10) === 400);
  check("suggestFlashPhase: 5 fps → 800", suggestFlashPhase(5) === 800);
  check("suggestFlashPhase: 2 fps → 2000", suggestFlashPhase(2) === 2000);
  check("suggestFlashPhase: caps at 4000", suggestFlashPhase(1) === 4000);
  check("suggestFlashPhase: guards bad input", suggestFlashPhase(0) === 4000 && suggestFlashPhase(-5) === 4000);
  check("suggestFlashPhase: never below 200", suggestFlashPhase(120) === 200);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}
