// Flashing frame tests: inverseBlocks inverts 3-bit values, the inverted
// phase of a flashed frame still decodes, and makeEmitter alternates
// normal/inverse rendering at the flash period.
// Run: node tests/flash.test.mjs (from web/)

import * as core from "../js/core.js";
import { makeEmitter, QR_DARK_COLORS, QR_QUIET_MODULES, FLASH_PHASE_MS } from "../js/light.js";
import { qrEncode } from "../js/qr.js";

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

// ---------- 1) inverseBlocks is a 3-bit NOT and an involution ----------
{
  const blocks = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7]);
  const inv = core.inverseBlocks(blocks);
  check("inverseBlocks is 3-bit NOT", inv.join(",") === "7,6,5,4,3,2,1,0", inv.join(","));
  check("double inversion is identity",
    core.inverseBlocks(inv).join(",") === "0,1,2,3,4,5,6,7");
}

// ---------- 2) the receiver decodes a flashed frame from either phase ----------
{
  const frame = core.newFrame("Chunk", 1234, 3, 5, new Uint8Array(core.RS_K).fill(0x42));
  const normal = core.renderBlocks(frame);
  // Normal phase: camera captures the normal rendering; decode directly.
  const decNormal = core.decodeBlocks(normal);
  check("normal phase decodes", decNormal.ok);
  // Inverted phase: camera captures the inverted rendering. The receiver
  // inverts the sampled blocks before decoding (inverseBlocks is an
  // involution, so one inversion recovers the normal blocks).
  const inverted = core.inverseBlocks(normal);
  const decInv = core.decodeBlocks(core.inverseBlocks(inverted));
  check("inverted phase decodes", decInv.ok);
  check("inverted phase recovers frame fields",
    decInv.ok && decInv.frame.kind === "Chunk" &&
    decInv.frame.sessionId === 1234 &&
    decInv.frame.chunkIdx === 3 &&
    decInv.frame.totalChunks === 5 &&
    decInv.frame.payload.length === core.RS_K &&
    decInv.frame.payload[0] === 0x42);
}

// ---------- 3) idle screen must not decode in either phase ----------
{
  const idle = new Uint8Array(core.GRID_W * core.GRID_H).fill(7);
  check("idle screen fails decode (normal phase)", !core.decodeBlocks(idle).ok);
  check("idle screen fails decode (inverted phase)",
    !core.decodeBlocks(core.inverseBlocks(idle)).ok);
}

// ---------- 4) camera-sampling path: pixels -> thresholded blocks -> invert -> decode ----------
// Simulates a camera that captures the inverted phase of a flashing frame:
// build pixels from the INVERTED rendering, threshold-sample them (as
// light.js sampleBlocks does), then invert the sampled blocks before decode.
{
  const frame = core.newFrame("Ping", 7, 0, 1, null);
  const blocks = core.inverseBlocks(core.renderBlocks(frame)); // inverted-phase rendering
  const w = core.GRID_W * core.BLOCK_PX;
  const h = core.GRID_H * core.BLOCK_PX;
  const pix = new Uint8Array(w * h * 4);
  for (let r = 0; r < core.GRID_H; r++) {
    for (let c = 0; c < core.GRID_W; c++) {
      const col = core.PALETTE[blocks[r * core.GRID_W + c]];
      for (let dy = 0; dy < core.BLOCK_PX; dy++) {
        for (let dx = 0; dx < core.BLOCK_PX; dx++) {
          const idx = ((r * core.BLOCK_PX + dy) * w + (c * core.BLOCK_PX + dx)) * 4;
          pix[idx] = col[0];
          pix[idx + 1] = col[1];
          pix[idx + 2] = col[2];
          pix[idx + 3] = 255;
        }
      }
    }
  }
  // Threshold sampling (same math as light.js sampleBlocks, blockPx = BLOCK_PX).
  const sampled = new Uint8Array(core.GRID_W * core.GRID_H);
  for (let r = 0; r < core.GRID_H; r++) {
    for (let c = 0; c < core.GRID_W; c++) {
      const x = c * core.BLOCK_PX + Math.floor(core.BLOCK_PX / 2);
      const y = r * core.BLOCK_PX + Math.floor(core.BLOCK_PX / 2);
      const idx = (y * w + x) * 4;
      sampled[r * core.GRID_W + c] =
        (pix[idx] >= 128 ? 1 : 0) | (pix[idx + 1] >= 128 ? 2 : 0) | (pix[idx + 2] >= 128 ? 4 : 0);
    }
  }
  const decInv = core.decodeBlocks(core.inverseBlocks(sampled));
  check("camera-sampled inverted phase decodes Ping",
    decInv.ok && decInv.frame.kind === "Ping" && decInv.frame.sessionId === 7);
}

// ---------- 5) makeEmitter alternates normal/inverse rendering ----------
{
  const puts = [];
  const canvas = {
    width: core.GRID_W,
    height: core.GRID_H,
    getContext: () => ({
      createImageData: (w, h) => ({ data: new Uint8ClampedArray(w * h * 4), width: w, height: h }),
      putImageData: (img) => { puts.push(img); },
    }),
  };

  // Fake the clock so the phase toggles deterministically; fall back to real
  // waits if the patch is not possible.
  let fakeNow = 0;
  let useFake = false;
  try {
    performance.now = () => fakeNow;
    useFake = true;
  } catch (e) {
    useFake = false;
  }
  const rafQ = [];
  const origRaf = globalThis.requestAnimationFrame;
  globalThis.requestAnimationFrame = (cb) => { rafQ.push(cb); return rafQ.length; };
  function runRaf() {
    const q = rafQ.splice(0, rafQ.length);
    for (const cb of q) cb();
  }
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  const frame = core.newFrame("Pong", 42, 0, 1, null);
  const emitter = makeEmitter(canvas);
  emitter.setFrame(frame);
  runRaf(); // tick 1: paint normal phase

  // 1 px per block: canvas is exactly GRID_W x GRID_H.
  const bi = core.HEADER_START;
  const nv = core.renderBlocks(frame)[bi];
  const iv = nv ^ 7;
  const x = bi % core.GRID_W;
  const y = Math.floor(bi / core.GRID_W);
  check("flash phase 1 renders normal blocks",
    puts.length === 1 &&
    puts[0].data[x * 4] === core.PALETTE[nv][0] &&
    puts[0].data[x * 4 + 1] === core.PALETTE[nv][1] &&
    puts[0].data[x * 4 + 2] === core.PALETTE[nv][2]);

  if (useFake) {
    fakeNow = FLASH_PHASE_MS; // >= FLASH_PHASE_MS -> toggle to inverted phase
    runRaf();
  } else {
    await sleep(60);
    runRaf();
  }
  check("flash phase 2 renders inverted blocks",
    puts.length === 2 &&
    puts[1].data[x * 4] === core.PALETTE[iv][0] &&
    puts[1].data[x * 4 + 1] === core.PALETTE[iv][1] &&
    puts[1].data[x * 4 + 2] === core.PALETTE[iv][2]);

  globalThis.requestAnimationFrame = origRaf;
}

// ---------- 6) makeEmitter paints QR codes as a centered colored box ----------
{
  const W = 300;
  const H = 200;
  const rects = [];
  let cur = "";
  const canvas = {
    width: W,
    height: H,
    getContext: () => ({
      set fillStyle(v) { cur = v; },
      get fillStyle() { return cur; },
      fillRect: (x, y, w2, h2) => { rects.push({ x, y, w: w2, h: h2, style: cur }); },
    }),
  };

  let fakeNow = 0;
  try { performance.now = () => fakeNow; } catch (e) { /* real clock */ }
  const rafQ = [];
  const origRaf = globalThis.requestAnimationFrame;
  globalThis.requestAnimationFrame = (cb) => { rafQ.push(cb); return rafQ.length; };
  const runRaf = () => {
    const q = rafQ.splice(0, rafQ.length);
    for (const cb of q) cb();
  };

  const frame = core.newFrame("Ping", 9, 0, 1, null);
  const qr = qrEncode(frame);
  const n = qr.size;
  const emitter = makeEmitter(canvas);
  emitter.setQr(qr);
  runRaf(); // phase 1 paint (normal)

  // expected geometry: centered square box with a quiet zone
  const units = n + 2 * QR_QUIET_MODULES;
  const mp = Math.max(1, Math.floor(Math.min(W, H) / units));
  const box = mp * units;
  const ox = Math.floor((W - box) / 2);
  const oy = Math.floor((H - box) / 2);

  const bg1 = rects[0];
  check("qr paint: white background fills canvas (normal phase)",
    bg1 && bg1.x === 0 && bg1.y === 0 && bg1.w === W && bg1.h === H && bg1.style === "rgb(255,255,255)");

  const mods1 = rects.slice(1);
  check("qr paint: modules inside the centered box",
    mods1.length > 0 &&
    mods1.every((m) =>
      m.x >= ox + QR_QUIET_MODULES * mp && m.x + m.w <= ox + box - QR_QUIET_MODULES * mp &&
      m.y >= oy + QR_QUIET_MODULES * mp && m.y + m.h <= oy + box - QR_QUIET_MODULES * mp));

  // the TL finder corner module (0,0) is always dark
  const tl = mods1.find((m) => m.x === ox + QR_QUIET_MODULES * mp && m.y === oy + QR_QUIET_MODULES * mp);
  const paletteStyles = QR_DARK_COLORS.map((c) => `rgb(${c[0]},${c[1]},${c[2]})`);
  check("qr paint: finder module uses a dark palette color",
    !!tl && paletteStyles.includes(tl.style), tl && tl.style);

  const styles1 = new Set(mods1.map((m) => m.style));
  check("qr paint: dark modules are multicolored", styles1.size > 1, "styles: " + styles1.size);

  fakeNow = FLASH_PHASE_MS; // >= FLASH_PHASE_MS -> inverted phase
  const phase2Start = rects.length;
  runRaf(); // tick: flips the phase and repaints inverted
  const p2 = rects.slice(phase2Start);
  const bg2 = p2[0];
  check("qr paint: black background fills canvas (inverted phase)",
    bg2 && bg2.x === 0 && bg2.y === 0 && bg2.w === W && bg2.h === H && bg2.style === "rgb(0,0,0)");
  const mods2 = p2.slice(1);
  check("qr paint: inverted-phase modules are white",
    mods2.length > 0 && mods2.every((m) => m.style === "rgb(255,255,255)"));

  emitter.stop();
  globalThis.requestAnimationFrame = origRaf;
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}