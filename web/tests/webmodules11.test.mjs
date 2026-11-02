// Behavioral tests for the new web modules: link_capacity,
// transfer_slicer, color_detector.
// Run: node tests/webmodules11.test.mjs (from web/)

import { estimateCapacity, isPlausible } from "../js/link_capacity.js";
import { slicePayload, chunkCount } from "../js/transfer_slicer.js";
import { detectColors, ColorDetector } from "../js/color_detector.js";
import { RS_K } from "../js/frame_codec.js";

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

// ---------- link_capacity.js ----------
{
  const e30 = estimateCapacity(30);
  check("capacity: 30fps = 30*RS_K", Math.abs(e30.bps - 30 * RS_K) < 1e-6, `got ${e30.bps}`);
  const e60 = estimateCapacity(60);
  check("capacity: 60fps > 30fps", e60.bps > e30.bps);
  check("capacity: payloadBytes is RS_K", e30.payloadBytes === RS_K);
  check("capacity: plausible", isPlausible(e30));

  const e15 = estimateCapacity(15);
  check("capacity: 15fps hand calc = 3345 B/s", Math.abs(e15.bps - 3345) < 1e-6, `got ${e15.bps}`);
}

// ---------- transfer_slicer.js ----------
{
  check("slicer: empty payload -> 0 chunks", slicePayload([]).length === 0);
  check("slicer: chunkCount(0) = 0", chunkCount(0) === 0);

  const small = slicePayload([1, 2, 3, 4, 5]);
  check("slicer: small payload 1 chunk", small.length === 1 && small[0].idx === 0 && JSON.stringify(small[0].data) === JSON.stringify([1, 2, 3, 4, 5]));

  const multi = slicePayload(new Array(500).fill(0xAB));
  check("slicer: 500 bytes -> 3 chunks", multi.length === 3, `got ${multi.length}`);
  check("slicer: first two chunks full RS_K", multi[0].data.length === RS_K && multi[1].data.length === RS_K);
  check("slicer: last chunk 54 bytes", multi[2].data.length === 54);

  for (const len of [0, 1, 223, 224, 500, 1000]) {
    const n = Math.max(0, Math.ceil(len / RS_K));
    check(`slicer: chunkCount(${len}) matches`, chunkCount(len) === n);
    check(`slicer: slicePayload(${len}).length matches`, slicePayload(new Array(len).fill(0)).length === n);
  }

  const seq = slicePayload(new Array(500).fill(0));
  let seqOk = true;
  seq.forEach((c, i) => { if (c.idx !== i) seqOk = false; });
  check("slicer: chunk indices sequential", seqOk);
}

// ---------- color_detector.js ----------
{
  const blocks = [0, 1, 2, 3, 4, 5, 6, 7];
  const r = detectColors(blocks);
  check("detector: all 8 colors present", r.hasColors === true);
  check("detector: each color counted once", r.counts.every((c) => c === 1));
  check("detector: dominant is 0 (first max)", r.dominant === 0);

  const empty = detectColors([]);
  check("detector: empty grid no colors", empty.hasColors === false && empty.dominant === -1);

  const d = new ColorDetector();
  d.observe([0, 0, 1, 1, 1]);
  check("detector: running counts", d.getCounts()[0] === 2 && d.getCounts()[1] === 3);
  check("detector: dominant is 1", d.dominant() === 1);

  const d2 = new ColorDetector();
  d2.observe([7, 7]);
  check("detector: count for color 7", d2.getCounts()[7] === 2);
  d2.reset();
  check("detector: reset clears histogram", d2.dominant() === -1);

  const fullGrid = new Array(270 * 480).fill(0);
  const r2 = detectColors(fullGrid);
  check("detector: full black grid", r2.hasColors === true && r2.dominant === 0 && r2.counts[0] === 270 * 480);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}