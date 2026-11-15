// qrdecode.js — decode actual QR codes from camera frames.
//
// Pipeline: downscale video (DS=4) → detect the 3 black QR finder squares →
// recover module pitch/offset (affine; the QR is square, frontal view) →
// read version info (v>=7) → sample the N×N module grid → resolve the mask
// from the format bits → unmask + zigzag read → per-block QR Reed-Solomon
// correction → parse the 23-byte PIXF header + payload.
//
// Flash-phase aware: if the format bits won't verify at normal polarity,
// the modules are flipped and decoding is retried (the sender flashes
// black-on-white and white-on-black alternately).
//
// 2-finder fallback: if only 2 finders are visible, the third is estimated
// from the square-geometry of a QR code.

import * as core from "./core.js";
import {
  VERSIONS,
  EC_LEVEL,
  MASKS,
  bch15,
  bch18,
  buildReserved,
  gfExp,
  gfMul,
  gfDiv,
} from "./qr.js";

const DS = 4; // downscale factor for detection

// Every version the pixfer protocol can emit, most-likely first. Full
// chunk frames (246 B) encode to v11; the short LAST chunk can land
// anywhere from v10 down to v2 (a 1-byte payload is a 24-byte frame);
// control frames are v2. Versions below 7 carry no version-info blocks,
// so they can only be found through this list.
const FALLBACK_VERSIONS = [11, 10, 9, 8, 7, 6, 5, 4, 3, 2];

// ---------- dark/light classification + connected components ----------

// A pixel is "dark" when it is luminance-dark OR strongly saturated with a
// low minimum channel. The sender renders dark QR modules in saturated
// colors; those keep one channel near 0 even after camera gain and
// white-balance shifts, so colored modules still classify as dark.
function isDarkPixel(r, g, b) {
  if ((r + g + b) / 3 < 128) return true;
  const mx = Math.max(r, g, b);
  const mn = Math.min(r, g, b);
  return mx - mn >= 80 && mn <= 128;
}

function blackMask(data, w, h) {
  const mask = new Uint8Array(w * h);
  for (let i = 0; i < w * h; i++) {
    mask[i] = isDarkPixel(data[i * 4], data[i * 4 + 1], data[i * 4 + 2]) ? 1 : 0;
  }
  return mask;
}

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

// Finder squares: 7x7 black QR finders are read as ring blobs (fill ratio
// ~24/49 ≈ 0.49) in the downscale space; the 3x3 black centers are solid
// squares (fill ratio 1.0) and are excluded by the ratio filter.
function squareBlobs(comps) {
  const squares = comps.filter(
    (c) =>
      c.count >= 20 &&
      c.fillRatio >= 0.3 &&
      c.fillRatio <= 0.68 &&
      Math.abs(c.w - c.h) <= 0.4 * Math.max(c.w, c.h) &&
      c.w >= 5
  );
  squares.sort((a, b) => b.count - a.count);
  return squares;
}

export function detectFinders(data, w, h) {
  // Finder rings first as dark blobs (normal phase: colored/black rings on
  // white); retry with a white mask for the inverted flash phase.
  let squares = squareBlobs(components(blackMask(data, w, h), w, h));
  if (squares.length < 2) {
    // Inverted flash phase: the QR is white-on-black, so the finders are
    // white rings on a black background. Retry with a white mask.
    const white = new Uint8Array(w * h);
    for (let i = 0; i < w * h; i++) {
      const g = (data[i * 4] + data[i * 4 + 1] + data[i * 4 + 2]) / 3;
      white[i] = g >= 128 ? 1 : 0;
    }
    squares = squareBlobs(components(white, w, h));
  }
  if (squares.length < 2) return { found: false, tl: null, tr: null, bl: null, estimated: false };

  const size = (c) => (c.w + c.h) / 2;
  const sizeFits = (a, b) => Math.abs(size(a) - size(b)) <= 0.15 * Math.max(size(a), size(b));

  // Triple pass: pick the three same-sized blobs forming the TL+TR+BL
  // right-isosceles corner, scoring by geometric error (minimal wins). A
  // data blob can mimic one ring, but three blobs almost never line up as
  // a tight corner — so error, not blob size, picks the finders.
  let best = null;
  let bestErr = Infinity;
  let bestSize = 0;
  for (const a of squares) {
    for (const b of squares) {
      if (b === a) continue;
      if (b.cx - a.cx <= 0) continue;
      const rowErr = Math.abs(b.cy - a.cy) / (b.cx - a.cx);
      if (rowErr > 0.12) continue; // TR: right of TL, same row
      if (!sizeFits(a, b)) continue;
      for (const c of squares) {
        if (c === a || c === b) continue;
        if (c.cy - a.cy <= 0) continue;
        const colErr = Math.abs(c.cx - a.cx) / (c.cy - a.cy);
        if (colErr > 0.12) continue; // BL: below TL, same column
        if (!sizeFits(a, c) || !sizeFits(b, c)) continue;
        const distX = b.cx - a.cx;
        const distY = c.cy - a.cy;
        const squareErr = Math.abs(distX - distY) / Math.max(distX, distY);
        if (squareErr > 0.1) continue; // square frame
        const spread =
          (Math.max(size(a), size(b), size(c)) - Math.min(size(a), size(b), size(c))) /
          Math.max(size(a), size(b), size(c));
        const err = rowErr + colErr + squareErr + spread;
        const totalSize = size(a) + size(b) + size(c);
        if (err < bestErr - 1e-9 || (err < bestErr + 1e-9 && totalSize > bestSize)) {
          bestErr = err;
          bestSize = totalSize;
          best = { tl: a, tr: b, bl: c };
        }
      }
    }
  }
  if (best) return { found: true, tl: best.tl, tr: best.tr, bl: best.bl, estimated: false };

  // Pair pass (partial view): two same-sized finders + a square-geometry
  // estimate of the third.
  const sorted = squares.slice().sort((x, y) => (x.cx + x.cy) - (y.cx + y.cy));
  const a = sorted[0];
  for (const b of sorted.slice(1)) {
    if (!sizeFits(a, b)) continue;
    const dx = Math.abs(a.cx - b.cx);
    const dy = Math.abs(a.cy - b.cy);
    if (dy <= 0.25 * dx) {
      // Same row → TL + TR; estimate BL below TL at the same distance.
      const tl = a.cx <= b.cx ? a : b;
      const tr = a.cx <= b.cx ? b : a;
      return {
        found: true,
        tl,
        tr,
        bl: { cx: tl.cx, cy: tl.cy + dx, w: tl.w, h: tl.h, count: tl.count, fillRatio: tl.fillRatio },
        estimated: true,
      };
    }
    if (dx <= 0.25 * dy) {
      // Same column → TL + BL; estimate TR to the right of TL.
      const tl = a.cy <= b.cy ? a : b;
      const bl = a.cy <= b.cy ? b : a;
      return {
        found: true,
        tl,
        bl,
        tr: { cx: tl.cx + dy, cy: tl.cy, w: tl.w, h: tl.h, count: tl.count, fillRatio: tl.fillRatio },
        estimated: true,
      };
    }
    // Diagonal pair — cannot anchor a square estimate.
  }
  return { found: false, tl: null, tr: null, bl: null, estimated: false };
}

// ---------- pure decode: RGBA pixels → pixfer frame ----------

// Sample one module (r, c) through the affine grid g = { o, ux, uy }.
// Reads the single pixel at the module center — at phone-to-phone
// distance modules land at 2-4 px, where any wider footprint crosses
// module boundaries and mixes neighbours (measured: 4-sample averaging
// zeroed the 2.9 px-module decode rate). Math.round centers the sample
// instead of the half-pixel floor bias.
function makeSampler(data, w, h, g) {
  const { o, ux, uy } = g;
  return (modRow, modCol) => {
    const x = Math.max(0, Math.min(w - 1, Math.round(o.x + (modCol + 0.5) * ux.x + (modRow + 0.5) * uy.x)));
    const y = Math.max(0, Math.min(h - 1, Math.round(o.y + (modCol + 0.5) * ux.y + (modRow + 0.5) * uy.y)));
    const idx = (y * w + x) * 4;
    return isDarkPixel(data[idx], data[idx + 1], data[idx + 2]) ? 1 : 0;
  };
}

function readVersionWord(sample, n, block) {
  let word = 0;
  for (let i = 0; i < 18; i++) {
    const bit =
      block === 0
        ? sample((i / 3) | 0, n - 11 + (i % 3)) // above TR finder
        : sample(n - 11 + (i % 3), (i / 3) | 0); // right of BL finder
    word = (word << 1) | bit;
  }
  const v = word >>> 12;
  return bch18(v) === word ? v : 0;
}

// Read one of the two 15-bit format-info copies, raw. Verification and
// error tolerance happen in resolveMask via bestFormatMask.
function readFormatWord(mods, n, copy) {
  let word = 0;
  for (let i = 0; i < 15; i++) {
    let bit;
    if (copy === 0) {
      if (i <= 5) bit = mods[8 * n + i];
      else if (i === 6) bit = mods[8 * n + 7];
      else if (i === 7) bit = mods[8 * n + 8];
      else if (i === 8) bit = mods[7 * n + 8];
      else bit = mods[(14 - i) * n + 8];
    } else {
      if (i < 8) bit = mods[(n - 1) * n + (n - 8 + i)];
      else bit = mods[(n - 7 + (i - 8)) * n + (n - 1)];
    }
    word = (word << 1) | bit;
  }
  return word;
}

// Match received format words against every valid EC-level-M format word
// (bch15 already applies the 0x5412 transmit mask). The BCH(15,5) format
// code has minimum distance 7, so a match within 3 bit errors is
// unambiguous — a noisy capture still resolves the mask.
function bestFormatMask(words) {
  let bestMask = -1;
  let bestDist = 4; // tolerance is 3 bit errors
  for (let mask = 0; mask < 8; mask++) {
    const cand = bch15(mask); // EC level M = 00 -> data5 == mask
    for (const word of words) {
      let dist = 0;
      let x = (word ^ cand) & 0x7fff;
      while (x) {
        x &= x - 1;
        dist++;
      }
      if (dist < bestDist) {
        bestDist = dist;
        bestMask = mask;
      }
    }
  }
  return bestMask;
}

// Read data bits in the standard zigzag order, skipping reserved cells.
function readZigzagBits(mods, reserved, n) {
  const bits = [];
  let upward = true;
  for (let c = n - 1; c > 0; c -= 2) {
    if (c === 6) c--; // skip the timing column
    for (let i = 0; i < n; i++) {
      const r = upward ? n - 1 - i : i;
      for (let k = 0; k < 2; k++) {
        const x = c - k;
        if (x < 0 || x >= n) continue;
        if (!reserved[r * n + x]) bits.push(mods[r * n + x]);
      }
    }
    upward = !upward;
  }
  return bits;
}

// ---------- QR Reed-Solomon decode (variable t) ----------

function polyEval(poly, x) {
  let acc = 0;
  for (let k = poly.length - 1; k >= 0; k--) {
    acc = gfMul(acc, x) ^ poly[k];
  }
  return acc;
}

function polyDerEval(poly, x) {
  let acc = 0;
  for (let k = poly.length - 2; k >= 0; k--) {
    const cVal = k % 2 === 0 ? poly[k + 1] : 0;
    acc = gfMul(acc, x) ^ cVal;
  }
  return acc;
}

function berlekampMassey(s) {
  let c = [1];
  let b = [1];
  let l = 0;
  let m = 1;
  let oldDiff = 1;
  for (let i = 0; i < s.length; i++) {
    let diff = s[i];
    for (let j = 1; j < c.length; j++) {
      if (i >= j && c[j] !== 0) diff ^= gfMul(c[j], s[i - j]);
    }
    if (diff === 0) {
      m += 1;
    } else {
      const t = c.slice();
      const coef = gfDiv(diff, oldDiff);
      while (b.length < c.length) b.push(0);
      while (c.length < b.length + m) c.push(0);
      for (let j = 0; j < b.length; j++) {
        if (b[j] !== 0) c[j + m] ^= gfMul(coef, b[j]);
      }
      if (2 * l <= i) {
        l = i + 1 - l;
        b = t;
        m = 1;
        oldDiff = diff;
      } else {
        m += 1;
      }
    }
  }
  return c;
}

function omegaPoly(s, loc) {
  const o = new Uint8Array(Math.max(s.length, loc.length - 1));
  for (let i = 0; i < loc.length; i++) {
    const li = loc[i];
    if (li === 0) continue;
    for (let j = 0; j < s.length; j++) {
      const sj = s[j];
      if (sj === 0) continue;
      const k = i + j;
      if (k < o.length) o[k] ^= gfMul(li, sj);
    }
  }
  return o;
}

// `coded` layout: [parity_0..t-1][data_0..] — parity occupies the low
// polynomial coefficients (matches qrRsParity's systematic encode, where
// div[j] holds the coefficient of x^j). The generator has t roots
// (α^0..α^(t-1)), so t syndromes correct up to floor(t/2) symbol errors.
function qrRsDecode(coded, t) {
  const n = coded.length;
  const s = new Uint8Array(t);
  for (let j = 0; j < t; j++) {
    const x = gfExp(j);
    let acc = 0;
    for (let k = n - 1; k >= 0; k--) acc = gfMul(acc, x) ^ coded[k];
    s[j] = acc;
  }
  let dirty = false;
  for (let j = 0; j < t; j++) if (s[j] !== 0) { dirty = true; break; }
  if (!dirty) return { ok: true, payload: new Uint8Array(coded.subarray(t)) };
  const loc = berlekampMassey(s); // Λ(x) = Σ loc[k]·x^k, loc[0] = 1
  const omega = omegaPoly(s, loc); // Ω = S·Λ mod x^t
  let errs = 0;
  const out = new Uint8Array(coded);
  for (let i = 0; i < 255; i++) {
    if (polyEval(loc, gfExp(i)) !== 0) continue;
    // Locator root at α^i → error at position p with X = α^-p, so
    // p = (255 - i) % 255 in the codeword's low-to-high powers.
    const p = (255 - i) % 255;
    if (p >= n) return { ok: false, error: "CheckFailed" };
    const num = polyEval(omega, gfExp(i));
    const den = polyDerEval(loc, gfExp(i));
    if (den === 0) return { ok: false, error: "CheckFailed" };
    out[p] ^= gfMul(gfDiv(num, den), gfExp(p));
    errs++;
  }
  if (errs > (t >> 1)) return { ok: false, error: "TooManyErrors" };
  // Residual syndromes mean the correction failed (too many errors).
  for (let j = 0; j < t; j++) {
    const x = gfExp(j);
    let acc = 0;
    for (let k = n - 1; k >= 0; k--) acc = gfMul(acc, x) ^ out[k];
    if (acc !== 0) return { ok: false, error: "TooManyErrors" };
  }
  return { ok: true, payload: new Uint8Array(out.subarray(t)) };
}

// ---------- header parsing (mirrors core.js buildHeader layout) ----------

function parseFrame(bytes) {
  if (bytes.length < core.HEADER_BYTES) return null;
  const head = bytes.subarray(0, core.HEADER_BYTES);
  if (head[0] !== 0x50 || head[1] !== 0x49 || head[2] !== 0x58 || head[3] !== 0x46) return null; // "PIXF"
  const kind = core.frameKindFromU8(head[4]);
  if (!kind) return null;
  const sessionId = (head[6] << 8) | head[5];
  const chunkIdx = core.u32le(head, 7);
  const totalChunks = core.u32le(head, 11);
  const payloadLen = Math.min(core.u32le(head, 15), bytes.length - core.HEADER_BYTES);
  const crcStored = core.u32le(head, 19);
  const payload = bytes.subarray(core.HEADER_BYTES, core.HEADER_BYTES + payloadLen);
  const ci = new Uint8Array(19 + payloadLen);
  ci.set(head.subarray(0, 19), 0);
  ci.set(payload, 19);
  if (core.crc32(ci) !== crcStored) return null;
  return { kind, sessionId, chunkIdx, totalChunks, payload: new Uint8Array(payload) };
}

// ---------- main decode ----------

// Sample an N×N module grid with a sampler function.
function sampleGrid(n, sample) {
  const mods = new Uint8Array(n * n);
  for (let r = 0; r < n; r++) {
    for (let c = 0; c < n; c++) {
      mods[r * n + c] = sample(r, c);
    }
  }
  return mods;
}

// Read the mask from the format bits (best match over both copies),
// retrying on the inverted phase when the normal polarity doesn't verify
// (flashed white-on-black capture).
function resolveMask(mods, n) {
  let mask = bestFormatMask([readFormatWord(mods, n, 0), readFormatWord(mods, n, 1)]);
  let inverted = false;
  if (mask < 0) {
    for (let i = 0; i < mods.length; i++) mods[i] ^= 1;
    inverted = true;
    mask = bestFormatMask([readFormatWord(mods, n, 0), readFormatWord(mods, n, 1)]);
  }
  return { mask, inverted };
}

// Affine module-grid geometry from the three finder centers: module (r,c)
// center = o + (c+0.5)*ux + (r+0.5)*uy. The step vectors ux/uy absorb
// hand-held rotation and anisotropic scale, so a few degrees of tilt
// decode exactly. (The pair pass places the estimated third finder at the
// exact perpendicular spot, so all three centers exist either way.)
function geometryForVersion(f, version) {
  const { tl, tr, bl } = f;
  const n = 17 + 4 * version;
  const ux = { x: (tr.cx - tl.cx) / (n - 7), y: (tr.cy - tl.cy) / (n - 7) };
  const uy = { x: (bl.cx - tl.cx) / (n - 7), y: (bl.cy - tl.cy) / (n - 7) };
  const o = {
    x: tl.cx - 3.5 * (ux.x + uy.x),
    y: tl.cy - 3.5 * (ux.y + uy.y),
  };
  return { version, n, o, ux, uy };
}

// Pure entry: decode from a downscale pixel buffer (w x h, RGBA).
// Warm start: between 30 fps frames a steady phone barely moves, so try
// the last successful affine grid BEFORE the flood-fill finder detection
// (the most expensive step). Keyed by buffer size — the DS=4 and DS=2
// buffers have different coordinate scales. Any stale hit fails the
// format/RS/CRC checks, so a wrong warm start can never yield a frame;
// it just falls through to the full path.
const warmBySize = new Map();

export function qrDecodeFromPixels(data, w, h) {
  const warm = warmBySize.get(w * h);
  if (warm) {
    const warmSample = makeSampler(data, w, h, warm);
    const warmMods = sampleGrid(warm.n, warmSample);
    const { mask: wm, inverted: wi } = resolveMask(warmMods, warm.n);
    if (wm >= 0) {
      const res = decodeVersion(warmMods, warm.n, warm.version, wm);
      if (res.ok) return { ok: true, frame: res.frame, inverted: wi };
    }
  }

  const f = detectFinders(data, w, h);
  if (!f.found) return { ok: false, error: "FindersNotFound" };
  const { tl, tr, bl } = f;

  // First pass: assume the chunk version (v13, N=69) to read the
  // version-info blocks; the version read corrects the assumption.
  const g0 = geometryForVersion(f, 13);
  const sample0 = makeSampler(data, w, h, g0);
  const vRead1 = readVersionWord(sample0, 17 + 4 * 13, 0);
  const vRead2 = vRead1 || readVersionWord(sample0, 17 + 4 * 13, 1);
  const versionInfoOk = vRead2 > 0;

  if (versionInfoOk) {
    const version = vRead2;
    const g = geometryForVersion(f, version);
    const sample = makeSampler(data, w, h, g);
    const mods = sampleGrid(g.n, sample);
    const { mask, inverted } = resolveMask(mods, g.n);
    if (mask >= 0) {
      const res = decodeVersion(mods, g.n, version, mask);
      if (res.ok) {
        warmBySize.set(w * h, g);
        return { ok: true, frame: res.frame, inverted };
      }
    }
    // Fall through to the fallback list: the sampled "version word" can
    // pass the 18-bit BCH check by chance on codes that carry no version
    // info (v2-v6), so it must never be trusted over a full decode.
  }

  // Version info unreadable: resample per candidate version until RS-decode
  // succeeds.
  for (const v of FALLBACK_VERSIONS) {
    const g = geometryForVersion(f, v);
    const sample = makeSampler(data, w, h, g);
    const mods = sampleGrid(g.n, sample);
    const { mask, inverted } = resolveMask(mods, g.n);
    if (mask < 0) continue;
    const res = decodeVersion(mods, g.n, v, mask);
    if (res.ok) {
      warmBySize.set(w * h, g);
      return { ok: true, frame: res.frame, inverted };
    }
  }
  return { ok: false, error: "VersionFallbackFailed" };
}

// Reserved-module maps are pure per version — cache them so the hot
// fallback path stops rebuilding the same map every frame.
const reservedByVer = new Map();
function reservedFor(version) {
  let r = reservedByVer.get(version);
  if (!r) {
    r = buildReserved(version);
    reservedByVer.set(version, r);
  }
  return r;
}

// Decode with a specific version: unmask, zigzag read, RS correct, parse.
function decodeVersion(mods, n, version, mask) {
  const reserved = reservedFor(version);
  const maskFn = MASKS[mask];
  for (let r = 0; r < n; r++) {
    for (let c = 0; c < n; c++) {
      if (!reserved[r * n + c] && maskFn(r, c)) mods[r * n + c] ^= 1;
    }
  }
  const bits = readZigzagBits(mods, reserved, n);
  if (bits.length === 0) return { ok: false, error: "NoDataBits" };

  // De-interleave into per-block data + EC codewords, RS-correct each.
  const row = VERSIONS[version - 1];
  const groups = row[3 + EC_LEVEL];
  const t = row[2][EC_LEVEL];
  const defs = [];
  for (const [count, dataPerBlock] of groups) {
    for (let i = 0; i < count; i++) defs.push(dataPerBlock);
  }
  const dataCwTotal = groups.reduce((s, g) => s + g[0] * g[1], 0);
  const nCw = dataCwTotal + defs.length * t;
  const cw = new Uint8Array(nCw);
  for (let i = 0; i < nCw; i++) {
    let v = 0;
    for (let j = 0; j < 8; j++) if (bits[i * 8 + j]) v |= 1 << (7 - j);
    cw[i] = v;
  }
  const maxData = Math.max(...defs);
  const dataBlocks = defs.map(() => []);
  const ecBlocks = defs.map(() => []);
  let idx = 0;
  for (let i = 0; i < maxData; i++) {
    for (let b = 0; b < defs.length; b++) {
      if (i < defs[b]) dataBlocks[b].push(cw[idx++]);
    }
  }
  for (let i = 0; i < t; i++) {
    for (let b = 0; b < defs.length; b++) ecBlocks[b].push(cw[idx++]);
  }
  const payloadParts = [];
  for (let b = 0; b < defs.length; b++) {
    // qrRsDecode expects [parity_0..t-1][data_0..] — parity occupies the
    // low polynomial coefficients, matching qrRsParity's encode.
    const coded = new Uint8Array(defs[b] + t);
    coded.set(ecBlocks[b], 0);
    coded.set(dataBlocks[b], t);
    const dec = qrRsDecode(coded, t);
    if (!dec.ok) return { ok: false, error: dec.error };
    payloadParts.push(dec.payload);
  }
  const dataBytesTotal = payloadParts.reduce((s, p) => s + p.length, 0);
  const dataBytes = new Uint8Array(dataBytesTotal);
  let off = 0;
  for (const p of payloadParts) {
    dataBytes.set(p, off);
    off += p.length;
  }

  // Strip the QR byte-mode prefix (4-bit mode + 8/16-bit length) from the
  // head of the data codewords: the PIXF frame starts right after it,
  // mid-codeword (12 or 20 bits in), so slice at bit granularity.
  const prefixBits = 4 + (version <= 9 ? 8 : 16);
  const frameBits = [];
  for (let i = 0; i < dataBytes.length; i++) {
    for (let j = 7; j >= 0; j--) frameBits.push((dataBytes[i] >> j) & 1);
  }
  const nFrameBytes = Math.max(0, Math.floor((frameBits.length - prefixBits) / 8));
  const frameBytes = new Uint8Array(nFrameBytes);
  for (let i = 0; i < nFrameBytes; i++) {
    let v = 0;
    for (let j = 0; j < 8; j++) if (frameBits[prefixBits + i * 8 + j]) v |= 1 << (7 - j);
    frameBytes[i] = v;
  }

  const frame = parseFrame(frameBytes);
  if (!frame) return { ok: false, error: "HeaderBad" };
  return { ok: true, frame };
}

// ---------- DOM wrapper ----------

// Decode a QR code from a live video frame. Scales are tried from
// fast/coarse to slow/fine: a code that fills a healthy part of the view
// decodes at 1/4 scale, but at phone-to-phone distance the QR can cover
// only ~20% of the frame — under 2 camera pixels per module — and only
// the full-resolution pass resolves it. Buffers above ~1.2M pixels are
// skipped (the flood-fill would outrun the frame budget); 1080p streams
// still get the 1/4 and 1/2 passes, 720p streams get all three.
export function qrDecodeVideo(video) {
  const vw = video.videoWidth || 640;
  const vh = video.videoHeight || 480;
  let dec = { ok: false, error: "NoDecode" };
  let ranFullRes = false;
  for (const ds of [4, 2, 1]) {
    if ((vw / ds) * (vh / ds) > 1200000) continue;
    if (ds === 1) ranFullRes = true;
    dec = decodeAtScale(video, vw, vh, ds);
    if (dec.ok) return dec;
  }
  // Big streams (1080p) skipped the full-res pass under the buffer cap:
  // decode a centered square crop at native resolution instead. The
  // viewfinder brackets guide the user to center the code, so a small QR
  // still lands in the crop with full module detail at a fraction of the
  // pixel cost (~350k px for 1080p).
  if (!ranFullRes) {
    dec = decodeAtCenterCrop(video, vw, vh);
    if (dec.ok) return dec;
  }
  return dec;
}

function decodeAtCenterCrop(video, vw, vh) {
  const side = Math.max(2, Math.round(Math.min(vw, vh) * 0.55));
  const x0 = ((vw - side) / 2) | 0;
  const y0 = ((vh - side) / 2) | 0;
  let cv = decodeAtCenterCrop._cv;
  if (!cv) {
    cv = document.createElement("canvas");
    decodeAtCenterCrop._cv = cv;
  }
  if (cv.width !== side || cv.height !== side) {
    cv.width = side;
    cv.height = side;
  }
  const ctx = cv.getContext("2d", { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(video, x0, y0, side, side, 0, 0, side, side);
  const d = ctx.getImageData(0, 0, side, side).data;
  return qrDecodeFromPixels(d, side, side);
}

const scaleCanvases = new Map();
function decodeAtScale(video, vw, vh, ds) {
  const w = Math.max(2, (vw / ds) | 0);
  const h = Math.max(2, (vh / ds) | 0);
  let cv = scaleCanvases.get(ds);
  if (!cv) {
    cv = document.createElement("canvas");
    scaleCanvases.set(ds, cv);
  }
  if (cv.width !== w || cv.height !== h) {
    cv.width = w;
    cv.height = h;
  }
  const ctx = cv.getContext("2d", { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(video, 0, 0, w, h);
  const d = ctx.getImageData(0, 0, w, h).data;
  return qrDecodeFromPixels(d, w, h);
}