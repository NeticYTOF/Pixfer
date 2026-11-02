// Behavioral tests for the new web modules: color_detector_ext,
// frame_encoder_ext.
// Run: node tests/webmodules21.test.mjs (from web/)

import { ColorDetectorExt, DetectionResult, detectColorsExt } from "../js/color_detector_ext.js";
import { FrameEncoderExt } from "../js/frame_encoder_ext.js";

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

// ---------- color_detector_ext.js ----------
{
  const r = detectColorsExt([]);
  check("det_ext: empty grid no colors", !r.hasColors && r.dominant === -1 && r.counts.every((c) => c === 0));

  const r2 = detectColorsExt([0, 1, 2, 3, 4, 5, 6, 7]);
  check("det_ext: all 8 colors counted once", r2.hasColors && r2.counts.every((c) => c === 1) && r2.dominant === 0);

  const r3 = detectColorsExt(new Array(270 * 480).fill(0));
  check("det_ext: full black grid dominant 0", r3.hasColors && r3.dominant === 0 && r3.counts[0] === 270 * 480);

  const d = new ColorDetectorExt();
  d.observe([0, 0, 1, 1, 1]);
  check("det_ext: running counts", d.getCounts()[0] === 2 && d.getCounts()[1] === 3);
  check("det_ext: dominant is 1", d.dominant() === 1);
  check("det_ext: total observed", d.total() === 5);
  d.reset();
  check("det_ext: reset clears", d.dominant() === -1 && d.total() === 0);

  const d2 = new ColorDetectorExt();
  d2.observe([9]); // 9 & 7 = 1
  check("det_ext: value wraps to 3 bits", d2.getCounts()[1] === 1);

  const dr = new DetectionResult();
  check("det_ext: result shape", dr.hasColors === false && dr.dominant === -1 && dr.counts.length === 8);
}

// ---------- frame_encoder_ext.js ----------
{
  const e = new FrameEncoderExt();
  check("enc_ext: BLOCKS is 270x480", FrameEncoderExt.BLOCKS === 270 * 480);
  check("enc_ext: default margin zero", e.margin() === 0.0);

  const r = e.encode(new Array(10).fill(42));
  check("enc_ext: encode returns ok", r.ok === true && r.blocks.length === 27, `got ${r.blocks ? r.blocks.length : "n/a"}`);
  check("enc_ext: blockCount math", e.blockCount([]) === 0
    && e.blockCount([1, 2, 3, 4, 5]) === 14
    && e.blockCount(new Array(223).fill(0)) === 595);

  const rf = e.encodeFull(new Array(223).fill(7));
  check("enc_ext: encodeFull validates size", rf.ok === true && rf.blocks.length === 595, `got ${rf.blocks ? rf.blocks.length : "n/a"}`);

  const re = e.encode(new Array(500).fill(0));
  check("enc_ext: oversize payload errors", re.ok === false && String(re.error).includes("too large"), `got ${re.error}`);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}