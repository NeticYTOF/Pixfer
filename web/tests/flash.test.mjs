// Flashing frame tests: inverseBlocks inverts 3-bit values, the inverted
// phase of a flashed frame still decodes, and makeEmitter alternates
// normal/inverse rendering at the flash period.
// Run: node tests/flash.test.mjs (from web/)

import * as core from "../js/core.js";
import { makeEmitter } from "../js/light.js";

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
    fakeNow = 50; // >= FLASH_PHASE_MS (50 ms) -> toggle to inverted phase
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

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}