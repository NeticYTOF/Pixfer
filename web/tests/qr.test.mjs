// Actual QR code tests: the VERSIONS table matches published byte
// capacities (level M), version selection, pixel round-trip (normal and
// inverted flash phases), RS correction of corrupted data modules, and a
// multicolor round-trip (dark modules rendered in the sender's palette).
// Run: node tests/qr.test.mjs (from web/)

import * as core from "../js/core.js";
import { qrEncode, qrCapacityBytes, buildReserved } from "../js/qr.js";
import { qrDecodeFromPixels } from "../js/qrdecode.js";
import { QR_DARK_COLORS } from "../js/light.js";

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
  const expected = { 1: 14, 2: 26, 9: 180, 10: 213, 11: 251, 12: 287, 13: 331 };
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
  check("chunk frame (246 B) encodes to v11 (61 modules)", r2.version === 11 && r2.size === 61,
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
  check("round-trip: normal phase decodes", dec.ok, dec.error);
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
  for (let i = 0; i < data.length; i += 4) {
    inv[i] = 255 - data[i];
    inv[i + 1] = 255 - data[i + 1];
    inv[i + 2] = 255 - data[i + 2];
    inv[i + 3] = 255;
  }
  const dec = qrDecodeFromPixels(inv, w, h);
  check("inverted phase decodes", dec.ok, dec.error);
  check("inverted phase recovers frame",
    dec.ok && dec.frame.kind === "Chunk" && dec.frame.sessionId === 1234);
  check("inverted phase flagged", dec.inverted === true);
}

// 5) RS correction: flip 20 DATA modules (reserved cells untouched) and
// the frame still decodes. v11-M blocks correct up to floor(30/2) = 15
// byte errors per block; 20 single-bit flips spread over 5 blocks is
// comfortably inside the bound.
{
  const payload = new Uint8Array(core.RS_K).fill(0x42);
  const frame = core.newFrame("Chunk", 777, 0, 2, payload);
  const qr = qrEncode(frame);
  const { data, w, h } = qrPixels(qr);
  const n = qr.size;
  const reserved = buildReserved(qr.version);
  const dataCells = [];
  for (let i = 0; i < n * n; i++) if (!reserved[i]) dataCells.push(i);
  for (let i = 0; i < 20; i++) {
    // coprime stride: distinct data cells
    const modIdx = dataCells[(i * 1013) % dataCells.length];
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
  check("corrupted QR still decodes (RS correction)", dec.ok, dec.error);
  check("corrupted QR recovers payload", dec.ok && dec.frame.payload[0] === 0x42 && dec.frame.sessionId === 777);
}

// 6) Multicolor round-trip: dark modules rendered in the sender's dark
// palette (as light.js paints them) still decode — the color carries no
// data, only the dark/light classification matters (isDarkPixel).
{
  const payload = new Uint8Array(core.RS_K).fill(0x42);
  const frame = core.newFrame("Chunk", 555, 1, 3, payload);
  const qr = qrEncode(frame);
  const n = qr.size;
  const px = 8;
  const quiet = 4; // quiet-zone modules, as light.js renders
  const w = (n + 2 * quiet) * px;
  const h = w;
  const data = new Uint8Array(w * h * 4);
  for (let r = -quiet; r < n + quiet; r++) {
    for (let c = -quiet; c < n + quiet; c++) {
      let col;
      if (r < 0 || c < 0 || r >= n || c >= n) col = [255, 255, 255];
      else if (qr.data[r * n + c]) col = QR_DARK_COLORS[(r * 7 + c * 11) % QR_DARK_COLORS.length];
      else col = [255, 255, 255];
      for (let dy = 0; dy < px; dy++) {
        for (let dx = 0; dx < px; dx++) {
          const idx = (((r + quiet) * px + dy) * w + ((c + quiet) * px + dx)) * 4;
          data[idx] = col[0];
          data[idx + 1] = col[1];
          data[idx + 2] = col[2];
          data[idx + 3] = 255;
        }
      }
    }
  }
  const dec = qrDecodeFromPixels(data, w, h);
  check("multicolor QR decodes", dec.ok, dec.error);
  check("multicolor QR recovers frame",
    dec.ok && dec.frame.kind === "Chunk" && dec.frame.sessionId === 555 && dec.frame.payload[0] === 0x42);
}

// 7) Frame-size presets: the sender's "big" (v20, 643 B) and "max"
// (v25, 974 B) frames round-trip through pixels like the standard ones.
{
  const presets = [
    [643, 20],
    [974, 25],
  ];
  for (const [pl, ver] of presets) {
    const payload = new Uint8Array(pl);
    for (let i = 0; i < pl; i++) payload[i] = (i * 29 + 7) & 0xff;
    const frame = core.newFrame("Chunk", 8100, 2, 9, payload);
    const qr = qrEncode(frame);
    check(`big frame ${pl} B encodes to v${ver}`, qr.version === ver, `v=${qr.version} n=${qr.size}`);
    const { data, w, h } = qrPixels(qr);
    const dec = qrDecodeFromPixels(data, w, h);
    check(`big frame ${pl} B decodes`, dec.ok, dec.error);
    check(
      `big frame ${pl} B payload is byte-exact`,
      dec.ok && dec.frame.payload.length === pl && dec.frame.payload.every((v, i) => v === payload[i])
    );
  }
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}
