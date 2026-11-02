// pixfer web camera alignment - detect the three QR-style finders in the
// camera view and recover the block pitch (video px per block) and offsets,
// so the sampler uses the true on-screen scale instead of a fixed guess.
// Mirrors the finder layout in `core.js` (FINDER_TL/TR/BL, 7x7 pattern).

import * as core from "./core.js";

const DS = 4; // downscale factor for detection

// Pure core: given an RGBA pixel buffer (w x h), find the 3 finders and
// return the alignment. Testable in node without a DOM.
export function alignFromPixels(data, w, h) {
  // 1) Gray-ring mask: pixels near (128,128,128) that are not white.
  const mask = new Uint8Array(w * h);
  for (let i = 0; i < w * h; i++) {
    const r = data[i * 4];
    const g = data[i * 4 + 1];
    const b = data[i * 4 + 2];
    const gray = Math.abs(r - 128) < 45 && Math.abs(g - 128) < 45 && Math.abs(b - 128) < 45;
    const white = r > 200 && g > 200 && b > 200;
    mask[i] = gray && !white ? 1 : 0;
  }

  // 2) Connected components of the gray mask (4-connected flood fill).
  const labels = new Int32Array(w * h).fill(-1);
  const comps = components(mask, w, h);

  // 3) Keep square-ish ring blobs: a 7x7 finder ring has fill ratio ~24/49.
  const squares = comps.filter(
    (c) =>
      c.count >= 20 &&
      c.fillRatio >= 0.3 &&
      c.fillRatio <= 0.68 &&
      Math.abs(c.w - c.h) <= 0.4 * Math.max(c.w, c.h) &&
      c.w >= 5
  );
  squares.sort((a, b) => b.count - a.count);
  const top = squares.slice(0, 3);
  if (top.length < 3) {
    return { found: false, pitchX: 0, pitchY: 0, offX: 0, offY: 0, quality: 0 };
  }

  // 4) Classify the 3 finders by geometry: TL = smallest (cx+cy);
  // TR = largest cx among the rest; BL = largest cy among the rest.
  const sorted = top.slice().sort((a, b) => (a.cx + a.cy) - (b.cx + b.cy));
  const tl = sorted[0];
  const rest = sorted.slice(1);
  const tr = rest[0].cx >= rest[1].cx ? rest[0] : rest[1];
  const bl = tr === rest[0] ? rest[1] : rest[0];

  // 5) Solve the transform. Block (0,0) top-left corner is the TL finder
  // corner; TR spans to block (GRID_W-7, 0); BL spans to block (0, GRID_H-7).
  const pitchX = ((tr.cx - tl.cx) / (core.GRID_W - core.FINDER_SIZE)) * DS;
  const pitchY = ((bl.cy - tl.cy) / (core.GRID_H - core.FINDER_SIZE)) * DS;
  const offX = (tl.cx - 3.5 * (pitchX / core.FINDER_SIZE)) * 1; // center minus half-finder
  const offY = (tl.cy - 3.5 * (pitchY / core.FINDER_SIZE)) * 1;

  // Quality: how square the finder blobs are (1 - aspect deviation), averaged.
  const sq = (c) => 1 - Math.abs(c.w - c.h) / Math.max(c.w, c.h, 1);
  const quality = (sq(tl) + sq(tr) + sq(bl)) / 3;

  const ok =
    pitchX >= 1 && pitchX <= 128 && pitchY >= 1 && pitchY <= 128 && pitchX > 0 && pitchY > 0;
  return {
    found: ok,
    pitchX,
    pitchY,
    offX,
    offY,
    quality,
  };
}

// DOM wrapper: downscale the video frame and run detection.
export function detectAlignment(video) {
  const vw = video.videoWidth || 640;
  const vh = video.videoHeight || 480;
  const w = Math.max(2, (vw / DS) | 0);
  const h = Math.max(2, (vh / DS) | 0);
  let cv = detectAlignment._cv;
  if (!cv) {
    cv = document.createElement("canvas");
    detectAlignment._cv = cv;
  }
  if (cv.width !== w || cv.height !== h) {
    cv.width = w;
    cv.height = h;
  }
  const ctx = cv.getContext("2d", { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(video, 0, 0, w, h);
  const d = ctx.getImageData(0, 0, w, h).data;
  const al = alignFromPixels(d, w, h);
  return al;
}

// 4-connected components with bounding boxes + fill ratios.
function components(mask, w, h) {
  const labels = new Int32Array(w * h).fill(-1);
  const out = [];
  for (let i = 0; i < w * h; i++) {
    if (!mask[i] || labels[i] !== -1) continue;
    const label = out.length;
    const comp = { count: 0, minx: w, miny: h, maxx: 0, maxy: 0, w: 0, h: 0, fillRatio: 0, cx: 0, cy: 0 };
    const stack = [i];
    labels[i] = label;
    while (stack.length) {
      const p = stack.pop();
      comp.count++;
      const x = p % w;
      const y = (p / w) | 0;
      if (x < comp.minx) comp.minx = x;
      if (y < comp.miny) comp.miny = y;
      if (x > comp.maxx) comp.maxx = x;
      if (y > comp.maxy) comp.maxy = y;
      const lbl = labels[p];
      if (x > 0 && mask[p - 1] && labels[p - 1] === -1) { labels[p - 1] = lbl; stack.push(p - 1); }
      if (x < w - 1 && mask[p + 1] && labels[p + 1] === -1) { labels[p + 1] = lbl; stack.push(p + 1); }
      if (y > 0 && mask[p - w] && labels[p - w] === -1) { labels[p - w] = lbl; stack.push(p - w); }
      if (y < h - 1 && mask[p + w] && labels[p + w] === -1) { labels[p + w] = lbl; stack.push(p + w); }
    }
    comp.w = comp.maxx - comp.minx + 1;
    comp.h = comp.maxy - comp.miny + 1;
    comp.fillRatio = comp.count / (comp.w * comp.h);
    comp.cx = (comp.minx + comp.maxx) / 2;
    comp.cy = (comp.miny + comp.maxy) / 2;
    out.push(comp);
  }
  return out;
}