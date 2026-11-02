// Actual QR code tests: the VERSIONS table matches published byte
// capacities (level M), version selection, pixel round-trip (normal and
// inverted flash phases), and RS correction of corrupted modules.
// Run: node tests/qr.test.mjs (from web/)

import * as core from "../js/core.js";
import { qrEncode, qrCapacityBytes } from "../js/qr.js";
import { qrDecodeFromPixels } from "../js/qrdecode.js";

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

// 1) VERSIONS table matches the published byte capacities (EC level M).
// (ISO/IEC 18004 published maximum data capacity, byte mode.)
{
  const expected = { 1: 14, 2: 26, 9: 176, 10: 194, 11: 208, 12: 232, 13: 260 };
  for (const [v, cap] of Object.entries(expected)) {
    const got = qrCapacityBytes(+v);
    check(`QR v${v}-M capacity == ${cap}`, got === cap, `got ${got}`);
  }
}

// 2) Version selection: control frames (23 B) and chunk frames (246 B).
{
  const pong = core.newFrame("Pong", 42, 0, 1, null);
  const r1 = qrEncode(pong);
  check("control frame (23 B) encodes to v2 (25 modules)", r1.version === 2 && r1.size === 25,
    `v=${r1.version} n=${r1.size}`);

  const payload = new Uint8Array(core.RS_K).fill(0x42);
  const chunk = core.newFrame("Chunk", 1234, 3, 5, payload);
  const r2 = qrEncode(chunk);
  check("chunk frame (246 B) encodes to v13 (69 modules)", r2.version === 13 && r2.size === 69,
    `v=${r2.version} n=${r2.size}`);
}

// Build a black-on-white pixel image for a QR code (pxPerModule px per module).
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

// 3) Pixel round-trip: encode → pixels → decode.
{
  const payload = new Uint8Array(core.RS_K).fill(0x42);
  const frame = core.newFrame("Chunk", 1234, 3, 5, payload);
  const qr = qrEncode(frame);
  const { data, w, h } = qrPixels(qr);
  const dec = qrDecodeFromPixels(data, w, h);
  check("round-trip: normal phase decodes", dec.ok);
  check("round-trip: frame fields recovered",
    dec.ok &&
    dec.frame.kind === "Chunk" &&
    dec.frame.sessionId === 1234 &&
    dec.frame.chunkIdx === 3 &&
    dec.frame.totalChunks === 5 &&
    dec.frame.payload.length === core.RS_K &&
    dec.frame.payload[0] === 0x42);
}

// 4) Inverted flash phase: white-on-black QR still decodes.
{
  const payload = new Uint8Array(core.RS_K).fill(0x42);
  const frame = core.newFrame("Chunk", 1234, 3, 5, payload);
  const qr = qrEncode(frame);
  const { data, w, h } = qrPixels(qr);
  const inv = new Uint8Array(data.length);
  for (let i = 0; i < data.length; i++) inv[i] = 255 - data[i];
  const dec = qrDecodeFromPixels(inv, w, h);
  check("inverted phase decodes", dec.ok);
  check("inverted phase recovers frame",
    dec.ok && dec.frame.kind === "Chunk" && dec.frame.sessionId === 1234);
  check("inverted phase flagged", dec.inverted === true);
}

// 5) RS correction: flip 20 modules (v13-M corrects up to t=44) and the
// frame still decodes.
{
  const payload = new Uint8Array(core.RS_K).fill(0x42);
  const frame = core.newFrame("Chunk", 777, 0, 2, payload);
  const qr = qrEncode(frame);
  const { data, w, h } = qrPixels(qr);
  const n = qr.size;
  for (let i = 0; i < 20; i++) {
    const modIdx = (i * 97) % (n * n); // coprime stride: distinct modules
    const r = (modIdx / n) | 0;
    const c = modIdx % n;
    for (let dy = 0; dy < 8; dy++) {
      for (let dx = 0; dx < 8; dx++) {
        const idx = ((r * 8 + dy) * w + (c * 8 + dx)) * 4;
        const v = data[idx];
        data[idx] = data[idx + 1] = data[idx + 2] = v === 0 ? 255 : 0;
      }
    }
  }
  const dec = qrDecodeFromPixels(data, w, h);
  check("corrupted QR still decodes (RS correction)", dec.ok);
  check("corrupted QR recovers payload", dec.ok && dec.frame.payload[0] === 0x42 && dec.frame.sessionId === 777);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}