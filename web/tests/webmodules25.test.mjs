// Behavioral tests for web/js/align.js — finder detection and alignment
// recovery from a synthetic pixel buffer (no DOM needed).
// Run: node tests/webmodules25.test.mjs (from web/)

import { alignFromPixels } from "../js/align.js";
import * as core from "../js/core.js";

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

// White background RGBA buffer.
function makeBuffer(w, h) {
  const data = new Uint8Array(w * h * 4);
  for (let i = 0; i < w * h; i++) {
    data[i * 4] = 255;
    data[i * 4 + 1] = 255;
    data[i * 4 + 2] = 255;
    data[i * 4 + 3] = 255;
  }
  return data;
}

// Paint one 7x7 finder (gray ring) centered at (cx, cy).
function drawFinder(data, w, cx, cy) {
  for (let dy = -3; dy <= 3; dy++) {
    for (let dx = -3; dx <= 3; dx++) {
      if (core.FINDER[dy + 3][dx + 3] !== 7) continue;
      const px = cx + dx;
      const py = cy + dy;
      if (px < 0 || py < 0 || px >= w || py * w + px >= data.length / 4) return;
      const i = (py * w + px) * 4;
      data[i] = 128;
      data[i + 1] = 128;
      data[i + 2] = 128;
    }
  }
}

// ---------- happy path: three finders at known geometry ----------
{
  // Downscale-space geometry: desired on-screen pitch is 16 px (x) and
  // 20 px (y) at full video resolution, i.e. 4 and 5 px in the DS=4
  // downscale space.
  const pitchXds = 4;
  const pitchYds = 5;
  const tl = { cx: 20, cy: 20 };
  const tr = { cx: tl.cx + (core.GRID_W - core.FINDER_SIZE) * pitchXds, cy: 20 };
  const bl = { cx: 20, cy: tl.cy + (core.GRID_H - core.FINDER_SIZE) * pitchYds };
  const w = 1080;
  const h = 2400;

  const data = makeBuffer(w, h);
  drawFinder(data, w, tl.cx, tl.cy);
  drawFinder(data, w, tr.cx, tr.cy);
  drawFinder(data, w, bl.cx, bl.cy);

  const al = alignFromPixels(data, w, h);
  check("align: three finders detected", al.found === true);
  check("align: pitchX recovered", al.pitchX === 16, `got ${al.pitchX}`);
  check("align: pitchY recovered", al.pitchY === 20, `got ${al.pitchY}`);
  check("align: offsets recovered", al.offX === 12 && al.offY === 10, `got x=${al.offX} y=${al.offY}`);
  check("align: quality is high for clean squares", al.quality === 1, `got ${al.quality}`);
}

// ---------- degraded: only two finders ----------
{
  const w = 1080;
  const h = 300;
  const data = makeBuffer(w, h);
  drawFinder(data, w, 20, 20);
  drawFinder(data, w, 1072, 20);
  const al = alignFromPixels(data, w, h);
  check("align: two finders are not enough", al.found === false && al.pitchX === 0 && al.quality === 0);
}

// ---------- degraded: single finder ----------
{
  const w = 400;
  const h = 400;
  const data = makeBuffer(w, h);
  drawFinder(data, w, 20, 20);
  const al = alignFromPixels(data, w, h);
  check("align: single finder rejected", al.found === false);
}

// ---------- no finders ----------
{
  const w = 300;
  const h = 300;
  const data = makeBuffer(w, h);
  const al = alignFromPixels(data, w, h);
  check("align: blank buffer rejected", al.found === false && al.quality === 0);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}