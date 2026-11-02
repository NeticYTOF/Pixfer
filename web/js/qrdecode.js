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

// Versions the pixfer protocol actually uses: control frames fit v2,
// chunk frames (246 bytes) fit v13. Fallback order for when the version
// info is corrupted.
const FALLBACK_VERSIONS = [13, 2, 10, 12, 14];

// ---------- black-mask + connected components ----------

function blackMask(data, w, h) {
  const mask = new Uint8Array(w * h);
  for (let i = 0; i < w * h; i++) {
    const g = (data[i * 4] + data[i * 4 + 1] + data[i * 4 + 2]) / 3;
    mask[i] = g < 128 ? 1 : 0;
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
  const top = squares.slice(0, 3);
  if (top.length < 2) return { found: false, tl: null, tr: null, bl: null, estimated: false };
  const sorted = top.slice().sort((a, b) => (a.cx + a.cy) - (b.cx + b.cy));
  let tl, tr, bl;
  let estimated = false;
  if (top.length === 3) {
    tl = sorted[0];
    const rest = sorted.slice(1);
    tr = rest[0].cx >= rest[1].cx ? rest[0] : rest[1];
    bl = tr === rest[0] ? rest[1] : rest[0];
  } else if (top.length === 2) {
    // Only two finders: estimate the third from square-geometry.
    const a = sorted[0], b = sorted[1];
    if (Math.abs(a.cy - b.cy) <= 0.25 * Math.abs(a.cx - b.cx)) {
      // Same row → TL + TR; estimate BL below TL at the same distance.
      tl = a;
      tr = b;
      bl = { cx: a.cx, cy: a.cy + Math.abs(b.cx - a.cx), w: a.w, h: a.h, count: a.count, fillRatio: a.fillRatio };
      estimated = true;
    } else {
      // Same column → TL + BL; estimate TR to the right of TL.
      tl = a;
      bl = b;
      tr = { cx: a.cx + Math.abs(b.cy - a.cy), cy: a.cy, w: a.w, h: a.h, count: a.count, fillRatio: a.fillRatio };
      estimated = true;
    }
  }
  return { found: true, tl, tr, bl, estimated };
}

// ---------- pure decode: RGBA pixels → pixfer frame ----------

// Sample one module (r, c) from the downscale pixel buffer.
function makeSampler(data, w, h, offX, offY, pitchX, pitchY) {
  return (modRow, modCol) => {
    const x = Math.max(0, Math.min(w - 1, (offX + (modCol + 0.5) * pitchX | 0)));
    const y = Math.max(0, Math.min(h - 1, (offY + (modRow + 0.5) * pitchY | 0)));
    const idx = (y * w + x) * 4;
    const g = (data[idx] + data[idx + 1] + data[idx + 2]) / 3;
    return g < 128 ? 1 : 0;
  };
}

function readVersionWord(sample, n, block) {
  let word = 0;
  for (let i = 0; i < 18; i++) {
    const bit =
      block === 0
        ? sample((i / 6) | 0, n - 9 + (i % 6)) // above TR finder
        : sample(n - 7 + ((i / 3) | 0), i % 3); // right of BL finder
    word = (word << 1) | bit;
  }
  const v = word >>> 12;
  return bch18(v) === word ? v : 0;
}

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
  const data5 = (word & 0x1f) ^ 0x12;
  return bch15(data5) === word ? data5 : -1;
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

// `coded` layout: [parity_0..t-1][data_0..]. Corrects up to t symbol
// errors (QR error-correction level M).
function qrRsDecode(coded, t) {
  const n = coded.length;
  const s = new Uint8Array(2 * t);
  for (let i = 0; i < 2 * t; i++) {
    const x = gfExp(i);
    let acc = 0;
    for (let k = n - 1; k >= 0; k--) acc = gfMul(acc, x) ^ coded[k];
    s[i] = acc;
  }
  if (s.every((v) => v === 0)) {
    return { ok: true, payload: new Uint8Array(coded.subarray(t)) };
  }
  const loc = berlekampMassey(s);
  const omega = omegaPoly(s, loc);
  const errs = [];
  for (let i = 0; i < n; i++) {
    if (polyEval(loc, gfExp(i)) === 0) errs.push((n - i) % n);
  }
  if (errs.length > t) return { ok: false, error: "TooManyErrors" };
  const out = new Uint8Array(coded);
  for (const pos of errs) {
    const x = gfExp((n - pos) % n);
    const num = polyEval(omega, x);
    const den = polyDerEval(loc, x);
    if (den === 0) return { ok: false, error: "CheckFailed" };
    out[pos] ^= gfMul(gfDiv(num, den), gfExp(pos % n));
  }
  // Residual syndromes mean more errors than t.
  for (let i = 0; i < 2 * t; i++) {
    const x = gfExp(i);
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

// Read the mask from the format bits, retrying on the inverted phase when
// the normal polarity doesn't verify (flashed white-on-black capture).
function resolveMask(mods, n) {
  let d5 = readFormatWord(mods, n, 0);
  if (d5 < 0) d5 = readFormatWord(mods, n, 1);
  let inverted = false;
  if (d5 < 0) {
    for (let i = 0; i < mods.length; i++) mods[i] ^= 1;
    inverted = true;
    d5 = readFormatWord(mods, n, 0);
    if (d5 < 0) d5 = readFormatWord(mods, n, 1);
  }
  if (d5 < 0) return { mask: -1, inverted };
  return { mask: d5 & 7, inverted };
}

// Geometry (pitch + offsets) for a candidate version from the finder centers.
function geometryForVersion(f, data, w, h, version) {
  const { tl, tr, bl, estimated } = f;
  const n = 17 + 4 * version;
  let pX, pY, oX, oY;
  if (!estimated) {
    pX = (tr.cx - tl.cx) / (n - 7);
    pY = (bl.cy - tl.cy) / (n - 7);
  } else if (tr !== null && bl === null) {
    // TL + TR visible: square geometry → pY = pX.
    pX = (tr.cx - tl.cx) / (n - 7);
    pY = pX;
  } else {
    // TL + BL visible: square geometry → pX = pY.
    pY = (bl.cy - tl.cy) / (n - 7);
    pX = pY;
  }
  oX = tl.cx - 3.5 * pX; // left edge of module 0
  oY = tl.cy - 3.5 * pY;
  return { n, pX, pY, oX, oY };
}

// Pure entry: decode from a downscale pixel buffer (w x h, RGBA).
export function qrDecodeFromPixels(data, w, h) {
  const f = detectFinders(data, w, h);
  if (!f.found) return { ok: false, error: "FindersNotFound" };
  const { tl, tr, bl } = f;

  // First pass: assume the chunk version (v13, N=69) to read the
  // version-info blocks; the version read corrects the assumption.
  const g0 = geometryForVersion(f, data, w, h, 13);
  const sample0 = makeSampler(data, w, h, g0.oX, g0.oY, g0.pX, g0.pY);
  const vRead1 = readVersionWord(sample0, 17 + 4 * 13, 0);
  const vRead2 = vRead1 || readVersionWord(sample0, 17 + 4 * 13, 1);
  const versionInfoOk = vRead2 > 0;

  if (versionInfoOk) {
    const version = vRead2;
    const g = geometryForVersion(f, data, w, h, version);
    const sample = makeSampler(data, w, h, g.oX, g.oY, g.pX, g.pY);
    const mods = sampleGrid(g.n, sample);
    const { mask, inverted } = resolveMask(mods, g.n);
    if (mask < 0) return { ok: false, error: "FormatBad" };
    const res = decodeVersion(mods, g.n, version, mask);
    if (!res.ok) return { ok: false, error: res.error, inverted };
    return { ok: true, frame: res.frame, inverted };
  }

  // Version info unreadable: resample per candidate version until RS-decode
  // succeeds.
  for (const v of FALLBACK_VERSIONS) {
    const g = geometryForVersion(f, data, w, h, v);
    const sample = makeSampler(data, w, h, g.oX, g.oY, g.pX, g.pY);
    const mods = sampleGrid(g.n, sample);
    const { mask, inverted } = resolveMask(mods, g.n);
    if (mask < 0) continue;
    const res = decodeVersion(mods, g.n, v, mask);
    if (res.ok) return { ok: true, frame: res.frame, inverted };
  }
  return { ok: false, error: "VersionFallbackFailed" };
}

// Decode with a specific version: unmask, zigzag read, RS correct, parse.
function decodeVersion(mods, n, version, mask) {
  const reserved = buildReserved(version);
  const maskFn = MASKS[mask];
  for (let r = 0; r < n; r++) {
    for (let c = 0; c < n; c++) {
      if (!reserved[r * n + c] && maskFn(r, c)) mods[r * n + c] ^= 1;
    }
  }
  const bits = readZigzagBits(mods, reserved, n);
  if (bits.length === 0) return { ok: false, error: "NoDataBits" };

  // Byte-mode prefix: 4-bit mode + 8-bit length (v<=9) or 16-bit (v>=10).
  const prefixBits = 4 + (version <= 9 ? 8 : 16);
  const dataBits = bits.slice(prefixBits);
  const nBytes = Math.floor(dataBits.length / 8);
  const bytes = new Uint8Array(nBytes);
  for (let i = 0; i < nBytes; i++) {
    let v = 0;
    for (let j = 0; j < 8; j++) if (dataBits[i * 8 + j]) v |= 1 << (7 - j);
    bytes[i] = v;
  }

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
    const coded = new Uint8Array(defs[b] + t);
    coded.set(dataBlocks[b], 0);
    coded.set(ecBlocks[b], defs[b]);
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

  const frame = parseFrame(dataBytes);
  if (!frame) return { ok: false, error: "HeaderBad" };
  return { ok: true, frame };
}

// ---------- DOM wrapper ----------

// Decode a QR code from a live video frame: downscale (DS=4) then run the
// pure pixel decode.
export function qrDecodeVideo(video) {
  const vw = video.videoWidth || 640;
  const vh = video.videoHeight || 480;
  const w = Math.max(2, (vw / DS) | 0);
  const h = Math.max(2, (vh / DS) | 0);
  let cv = qrDecodeVideo._cv;
  if (!cv) {
    cv = document.createElement("canvas");
    qrDecodeVideo._cv = cv;
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