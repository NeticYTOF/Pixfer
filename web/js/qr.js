// qr.js — actual QR code encoder (byte mode, EC level M).
//
// Encodes a pixfer frame ([23-byte PIXF header][payload]) into a real QR
// matrix: black/white modules, 7x7 finder patterns + separators, timing
// patterns, 5x5 alignment patterns, format info (BCH 15,5), version info
// (BCH 18,6) for v>=7, mask selection by penalty, and QR Reed-Solomon
// error correction. The output is scannable by any standard QR scanner.
//
// Keep the header layout in lockstep with core.js (buildHeader).

import * as core from "./core.js";

// ---------- GF(256) (primitive polynomial 0x11D) ----------
// Tables live at module scope so the QR decoder (qrdecode.js) can share
// them via the exported gfExp/gfMul/gfDiv helpers.
const GF_EXP = new Uint16Array(512);
const GF_LOG = new Uint8Array(256);
{
  let x = 1;
  for (let i = 0; i < 255; i++) {
    GF_EXP[i] = x;
    GF_LOG[x] = i;
    x = x << 1;
    if (x & 0x100) x ^= 0x11d;
  }
  for (let i = 255; i < 512; i++) GF_EXP[i] = GF_EXP[i - 255];
}

export function gfExp(i) {
  return GF_EXP[i];
}
export function gfMul(a, b) {
  if (a === 0 || b === 0) return 0;
  return GF_EXP[(GF_LOG[a] + GF_LOG[b]) % 255];
}
export function gfDiv(a, b) {
  if (b === 0) return 0;
  let idx = (GF_LOG[a] - GF_LOG[b]) % 255;
  if (idx < 0) idx += 255;
  return GF_EXP[idx];
}

// EC levels: L=0, M=1, Q=2, H=3. Camera reads at a distance: M is the
// right balance of redundancy vs. payload.
export const EC_LEVEL = 1; // M

// Standard QR version table (ISO/IEC 18004, tables 9-11):
// [version, totalCodewords, [ecL,ecM,ecQ,ecH], groupsL, groupsM, groupsQ, groupsH, alignmentCenters]
export const VERSIONS = [
  [1, 26, [7, 10, 13, 17], [[1, 19]], [[1, 16]], [[1, 13]], [[1, 9]], []],
  [2, 44, [10, 16, 22, 28], [[1, 34]], [[1, 28]], [[1, 22]], [[2, 8]], []],
  [3, 70, [15, 26, 36, 44], [[1, 55]], [[1, 44]], [[2, 17]], [[2, 13]], []],
  [4, 100, [10, 18, 26, 32], [[2, 40]], [[2, 32]], [[2, 24]], [[4, 9]], [6, 26]],
  [5, 134, [13, 24, 36, 44], [[2, 54]], [[2, 43]], [[2, 15], [2, 16]], [[2, 11], [2, 12]], [6, 34]],
  [6, 172, [18, 16, 24, 28], [[2, 68]], [[4, 27]], [[4, 19]], [[4, 15]], [6, 38]],
  [7, 196, [20, 18, 18, 26], [[2, 78]], [[4, 31]], [[4, 14], [2, 16]], [[4, 13], [1, 14]], [6, 22, 38]],
  [8, 242, [24, 22, 22, 26], [[3, 48], [1, 50]], [[3, 38], [1, 40]], [[6, 13], [2, 16]], [[6, 10], [2, 13]], [6, 24, 42]],
  [9, 292, [30, 22, 20, 24], [[2, 116]], [[4, 36], [1, 38]], [[4, 16], [4, 17]], [[4, 12], [4, 13]], [6, 26, 46]],
  [10, 346, [18, 26, 24, 20], [[3, 68], [1, 70]], [[4, 43], [1, 44]], [[6, 19], [2, 20]], [[10, 11], [1, 12]], [6, 28, 50]],
  [11, 404, [20, 30, 22, 24], [[4, 81]], [[4, 50], [1, 54]], [[10, 18]], [[5, 10], [6, 15]], [6, 30, 54]],
  [12, 466, [32, 22, 22, 19], [[2, 123], [1, 124]], [[6, 36], [2, 37]], [[10, 17], [2, 18]], [[2, 9], [14, 10]], [6, 32, 58]],
  [13, 532, [26, 22, 18, 22], [[4, 107]], [[8, 37], [1, 38]], [[12, 15], [4, 16]], [[12, 11], [4, 12]], [6, 34, 62]],
  [14, 581, [20, 22, 27, 21], [[1, 76], [5, 77]], [[5, 36], [5, 37]], [[3, 21], [9, 22]], [[1, 10], [17, 11]], [6, 36, 66]],
  [15, 655, [22, 24, 30, 27], [[5, 87], [1, 88]], [[5, 41], [5, 42]], [[11, 17], [6, 18]], [[1, 13], [15, 14]], [6, 38, 70]],
  [16, 733, [24, 25, 34, 30], [[5, 98], [1, 99]], [[9, 41], [2, 42]], [[11, 20], [5, 21]], [[13, 13], [6, 14]], [6, 40, 74]],
  [17, 815, [24, 24, 28, 25], [[4, 92], [3, 93]], [[13, 39]], [[4, 14], [17, 15]], [[23, 9], [2, 10]], [6, 42, 78]],
  [18, 901, [23, 24, 25, 24], [[7, 90], [1, 91]], [[11, 40], [3, 41]], [[13, 15], [10, 16]], [[24, 10], [1, 11]], [6, 44, 82]],
  [19, 991, [22, 23, 24, 26], [[6, 88], [3, 89]], [[13, 39], [3, 40]], [[23, 17]], [[15, 8], [19, 9]], [6, 46, 86]],
  [20, 1085, [25, 24, 22, 22], [[7, 95], [2, 98]], [[11, 39], [6, 40]], [[18, 16], [9, 17]], [[17, 10], [15, 11]], [6, 48, 90]],
  [21, 1156, [25, 26, 22, 22], [[4, 103], [5, 104]], [[17, 42]], [[20, 17], [7, 18]], [[34, 10]], [6, 50, 94]],
  [22, 1258, [28, 26, 26, 30], [[2, 111], [7, 112]], [[10, 43], [8, 44]], [[17, 20], [8, 21]], [[12, 9], [25, 10]], [6, 52, 98]],
  [23, 1364, [25, 25, 24, 27], [[4, 88], [8, 89]], [[20, 43]], [[24, 16], [10, 17]], [[16, 11], [19, 12]], [6, 54, 102]],
  [24, 1474, [26, 27, 26, 28], [[2, 96], [10, 97]], [[10, 43], [11, 44]], [[8, 17], [26, 18]], [[20, 11], [17, 12]], [6, 56, 106]],
  [25, 1588, [28, 26, 28, 28], [[8, 104], [4, 105]], [[12, 43], [11, 44]], [[10, 18], [24, 19]], [[2, 11], [38, 12]], [6, 58, 110]],
  [26, 1706, [28, 26, 35, 36], [[6, 103], [7, 104]], [[13, 42], [12, 43]], [[19, 14], [16, 15]], [[38, 7]], [6, 60, 114]],
  [27, 1828, [28, 27, 29, 29], [[9, 102], [5, 104]], [[16, 43], [10, 44]], [[39, 18], [4, 19]], [[40, 13], [2, 24]], [6, 62, 118]],
  [28, 1921, [30, 26, 28, 28], [[11, 107], [3, 108]], [[11, 42], [17, 43]], [[29, 19], [16, 20]], [[14, 14], [31, 15]], [6, 64, 122]],
  [29, 2051, [30, 27, 28, 28], [[4, 106], [11, 107]], [[9, 43], [20, 44]], [[16, 16], [35, 17]], [[1, 12], [53, 13]], [6, 66, 126]],
  [30, 2185, [30, 26, 29, 28], [[7, 106], [9, 107]], [[22, 44], [9, 45]], [[23, 16], [31, 17]], [[53, 13], [4, 14]], [6, 68, 130]],
  [31, 2323, [30, 26, 32, 30], [[6, 106], [11, 107]], [[30, 44], [3, 45]], [[5, 13], [52, 14]], [[17, 11], [43, 12]], [6, 70, 134]],
  [32, 2465, [32, 26, 38, 32], [[1, 106], [17, 107]], [[34, 44], [1, 45]], [[1, 11], [62, 12]], [[61, 10], [5, 11]], [6, 72, 138]],
  [33, 2611, [30, 26, 36, 33], [[11, 107], [8, 108]], [[34, 44], [3, 45]], [[59, 11], [6, 12]], [[69, 9], [1, 10]], [6, 74, 142]],
  [34, 2761, [32, 27, 38, 33], [[5, 113], [14, 114]], [[23, 45], [15, 46]], [[27, 10], [41, 11]], [[5, 8], [69, 9]], [6, 76, 146]],
  [35, 2876, [32, 27, 40, 35], [[4, 74], [26, 75]], [[28, 45], [12, 46]], [[34, 14], [54, 15]], [[74, 9], [32, 10]], [6, 78, 150]],
  [36, 3034, [31, 26, 41, 36], [[30, 76], [2, 77]], [[66, 29]], [[26, 14], [66, 15]], [[46, 9], [64, 10]], [6, 80, 154]],
  [37, 3196, [33, 27, 42, 37], [[8, 77], [25, 78]], [[48, 29], [20, 30]], [[94, 15], [1, 16]], [[54, 9], [61, 10]], [6, 82, 158]],
  [38, 3362, [31, 27, 44, 39], [[28, 77], [7, 78]], [[28, 29], [43, 30]], [[82, 15], [17, 16]], [[48, 9], [71, 10]], [6, 84, 162]],
  [39, 3532, [30, 27, 49, 48], [[20, 117], [4, 118]], [[38, 45], [11, 46]], [[66, 15], [37, 16]], [[8, 9], [115, 10]], [6, 86, 166]],
  [40, 3706, [29, 26, 52, 49], [[8, 77], [30, 78]], [[22, 30], [54, 31]], [[46, 15], [61, 16]], [[121, 10], [6, 11]], [6, 88, 170]],
];

// Byte-mode capacity (bytes) for a version at EC level M.
export function qrCapacityBytes(version) {
  const row = VERSIONS[version - 1];
  // groupsL/M/Q/H sit at indices 3, 4, 5, 6.
  const groups = row[3 + EC_LEVEL];
  let dataCw = 0;
  for (const [count, dataPerBlock] of groups) dataCw += count * dataPerBlock;
  const lenBits = version <= 9 ? 8 : 16;
  return Math.floor((dataCw * 8 - 4 - lenBits) / 8);
}

function pickVersion(dataLen) {
  for (let v = 1; v <= 40; v++) {
    if (qrCapacityBytes(v) >= dataLen) return v;
  }
  throw new Error("data too large for QR v40 (EC level M)");
}

// ---------- pixfer frame → bytes (23-byte PIXF header + payload) ----------
function setU32le(buf, off, v) {
  buf[off] = v & 0xff;
  buf[off + 1] = (v >>> 8) & 0xff;
  buf[off + 2] = (v >>> 16) & 0xff;
  buf[off + 3] = (v >>> 24) & 0xff;
}

// Same header layout as core.js buildHeader.
export function frameBytes(frame) {
  const payload = frame.payload && frame.payload.length ? frame.payload : new Uint8Array(0);
  const head = new Uint8Array(core.HEADER_BYTES);
  head.set([0x50, 0x49, 0x58, 0x46], 0); // "PIXF"
  head[4] = core.FRAME_KIND[frame.kind];
  head[5] = frame.sessionId & 0xff;
  head[6] = (frame.sessionId >>> 8) & 0xff;
  setU32le(head, 7, frame.chunkIdx);
  setU32le(head, 11, frame.totalChunks);
  setU32le(head, 15, payload.length);
  const ci = new Uint8Array(19 + payload.length);
  ci.set(head.subarray(0, 19), 0);
  if (payload.length) ci.set(payload, 19);
  setU32le(head, 19, core.crc32(ci));
  const out = new Uint8Array(core.HEADER_BYTES + payload.length);
  out.set(head, 0);
  out.set(payload, core.HEADER_BYTES);
  return out;
}

// ---------- QR Reed-Solomon (variable EC length, roots a^0..a^{t-1}) ----------
function qrRsParity(data, t) {
  let gen = [1];
  for (let i = 0; i < t; i++) {
    const root = gfExp(i);
    const next = new Array(gen.length + 1).fill(0);
    for (let j = 0; j < gen.length; j++) {
      const c = gen[j];
      if (c !== 0) {
        next[j] ^= c;
        next[j + 1] ^= gfMul(c, root);
      }
    }
    gen = next;
  }
  // Systematic encode: codeword = [parity_0..t-1][data_0..n-1].
  const n = data.length;
  const div = new Uint8Array(n + t);
  div.set(data, t);
  for (let i = div.length - 1; i >= t; i--) {
    const c = div[i];
    if (c !== 0) {
      for (let j = 0; j < gen.length; j++) {
        if (gen[j] !== 0) div[i - j] ^= gfMul(c, gen[j]);
      }
    }
  }
  return div.subarray(0, t);
}

// ---------- bit stream ----------
function encodeBits(bytes, version) {
  const bits = [];
  const pushBits = (value, count, bitIndex = 0) => {
    for (let i = 0; i < count; i++) bits.push((value >> (count - 1 - i)) & 1);
  };
  pushBits(0b0100, 4); // byte mode
  pushBits(bytes.length, version <= 9 ? 8 : 16);
  for (const b of bytes) pushBits(b, 8);
  // Terminator (up to 4 zero bits), pad to byte boundary, then alternate
  // 0xEC / 0x11 pad bytes up to the data-codeword capacity.
  for (let i = 0; i < 4 && bits.length % 8 !== 0; i++) bits.push(0);
  while (bits.length % 8 !== 0) bits.push(0);
  const row = VERSIONS[version - 1];
  const groups = row[3 + EC_LEVEL];
  const dataCw = groups.reduce((s, g) => s + g[0] * g[1], 0);
  let padToggle = 0;
  while (bits.length < dataCw * 8) {
    const padByte = padToggle ? 0x11 : 0xEC; // 0xEC = 11101100, 0x11 = 00010101
    for (let j = 7; j >= 0; j--) bits.push((padByte >> j) & 1);
    padToggle ^= 1;
  }
  return bits;
}

// ---------- block split / RS / interleave ----------
function blockDefs(version) {
  const row = VERSIONS[version - 1];
  const groups = row[3 + EC_LEVEL];
  const defs = [];
  for (const [count, dataPerBlock] of groups) {
    for (let i = 0; i < count; i++) defs.push(dataPerBlock);
  }
  return defs;
}

function ecPerBlock(version) {
  return VERSIONS[version - 1][2][EC_LEVEL];
}

function encodeCodewords(bits, version) {
  // bits -> data codewords (byte stream)
  const nCw = bits.length / 8;
  const dataCw = new Uint8Array(nCw);
  for (let i = 0; i < nCw; i++) {
    let v = 0;
    for (let j = 0; j < 8; j++) if (bits[i * 8 + j]) v |= 1 << (7 - j);
    dataCw[i] = v;
  }
  const defs = blockDefs(version);
  const t = ecPerBlock(version);
  // Split data into blocks, RS-encode, interleave data then EC (round-robin).
  const dataBlocks = [];
  const ecBlocks = [];
  let off = 0;
  for (const dpb of defs) {
    dataBlocks.push(dataCw.subarray(off, off + dpb));
    off += dpb;
  }
  const ecParity = dataBlocks.map((db) => qrRsParity(new Uint8Array(db), t));
  const maxData = Math.max(...defs);
  const out = new Uint8Array(dataCw.length + defs.length * t);
  let idx = 0;
  for (let i = 0; i < maxData; i++) {
    for (let b = 0; b < defs.length; b++) {
      if (i < defs[b]) out[idx++] = dataBlocks[b][i];
    }
  }
  for (let i = 0; i < t; i++) {
    for (let b = 0; b < defs.length; b++) out[idx++] = ecParity[b][i];
  }
  return out;
}

// ---------- matrix ----------
const FINDER = [
  [1, 1, 1, 1, 1, 1, 1],
  [1, 0, 0, 0, 0, 0, 1],
  [1, 0, 1, 1, 1, 0, 1],
  [1, 0, 1, 1, 1, 0, 1],
  [1, 0, 1, 1, 1, 0, 1],
  [1, 0, 0, 0, 0, 0, 1],
  [1, 1, 1, 1, 1, 1, 1],
];
const ALIGN = [
  [1, 1, 1, 1, 1],
  [1, 0, 0, 0, 1],
  [1, 0, 1, 0, 1],
  [1, 0, 0, 0, 1],
  [1, 1, 1, 1, 1],
];

export const MASKS = [
  (r, c) => (r + c) % 2 === 0,
  (r, c) => r % 2 === 0,
  (r, c) => c % 3 === 0,
  (r, c) => (r + c) % 3 === 0,
  (r, c) => ((r >> 1) + ((c / 3) | 0)) % 2 === 0,
  (r, c) => ((r * c) % 2 + (r * c) % 3) % 2 === 0,
  (r, c) => (((r * c) % 2 + (r + c) % 3) % 2) === 0,
  (r, c) => (((r + c) % 2 + (r * c) % 5) % 2) === 0,
];

export function bch15(data5) {
  let rem = data5 << 10;
  for (let i = 0; i < 5; i++) {
    rem = (rem << 1) ^ (((rem >>> 14) & 1) === 1 ? 0x5d : 0);
  }
  return ((rem & 0x7fe) | data5) ^ 0x5412;
}

export function bch18(version) {
  let rem = version << 12;
  for (let i = 0; i < 12; i++) {
    rem = (rem << 1) ^ (((rem >>> 17) & 1) === 1 ? 0x1f25 : 0);
  }
  return (version << 12) | (rem & 0x1fff);
}

class QrMatrix {
  constructor(size) {
    this.size = size;
    this.data = new Uint8Array(size * size); // 1 = dark module
    this.reserved = new Uint8Array(size * size);
  }
  set(c, r, dark) {
    if (r < 0 || c < 0 || r >= this.size || c >= this.size) return;
    this.data[r * this.size + c] = dark ? 1 : 0;
    this.reserved[r * this.size + c] = 1;
  }
  get(c, r) {
    return this.data[r * this.size + c];
  }
  isReserved(c, r) {
    return this.reserved[r * this.size + c] === 1;
  }
}

function drawFinders(m) {
  const n = m.size;
  const putPattern = (pat, c0, r0) => {
    for (let r = 0; r < pat.length; r++) {
      for (let c = 0; c < pat.length; c++) {
        m.set(c0 + c, r0 + r, pat[r][c] === 1);
      }
    }
    // Separators (white, reserved) around each finder.
    for (let i = -1; i <= pat.length; i++) {
      m.set(c0 - 1, r0 + i, false); // left
      m.set(c0 + pat.length, r0 + i, false); // right
      m.set(c0 + i, r0 - 1, false); // top
      m.set(c0 + i, r0 + pat.length, false); // bottom
    }
  };
  putPattern(FINDER, 0, 0); // TL
  putPattern(FINDER, n - 7, 0); // TR
  putPattern(FINDER, 0, n - 7); // BL
}

function drawTiming(m) {
  const n = m.size;
  for (let i = 8; i <= n - 9; i++) {
    const dark = i % 2 === 0;
    if (!m.isReserved(i, 6)) m.set(i, 6, dark);
    if (!m.isReserved(6, i)) m.set(6, i, dark);
  }
}

function drawAlignments(m, version) {
  const centers = VERSIONS[version - 1][7];
  const n = m.size;
  for (const cy of centers) {
    for (const cx of centers) {
      // Skip the three centers that coincide with finder corners.
      if ((cx === 6 && cy === 6) || (cx === n - 7 && cy === 6) || (cx === 6 && cy === n - 7)) continue;
      for (let r = -2; r <= 2; r++) {
        for (let c = -2; c <= 2; c++) {
          m.set(cx + c, cy + r, ALIGN[r + 2][c + 2] === 1);
        }
      }
    }
  }
}

function drawDarkModule(m) {
  m.set(8, m.size - 7, true); // always-dark module below the BL finder
}

function placeVersionInfo(m, version) {
  if (version < 7) return;
  const bits18 = bch18(version);
  const n = m.size;
  for (let i = 0; i < 18; i++) {
    const bit = (bits18 >> (17 - i)) & 1;
    // Block above the TR finder: rows 0-2, cols n-9..n-4 (3 rows x 6 cols).
    if (!m.isReserved(n - 9 + (i % 6), (i / 6) | 0)) {
      m.set(n - 9 + (i % 6), (i / 6) | 0, bit === 1);
    }
    // Block right of the BL finder: cols 0-2, rows n-7..n-5.
    if (!m.isReserved(i % 3, n - 7 + ((i / 3) | 0))) {
      m.set(i % 3, n - 7 + ((i / 3) | 0), bit === 1);
    }
  }
}

function placeFormatBits(m, mask) {
  const data5 = (0b00 << 3) | mask; // EC level M = 00
  const fmt = bch15(data5);
  const n = m.size;
  for (let i = 0; i < 15; i++) {
    const bit = (fmt >> (14 - i)) & 1;
    // Copy A: around the TL finder.
    if (i <= 5) m.set(i, 8, bit === 1); // row 8, cols 0-5
    else if (i === 6) m.set(7, 8, bit === 1); // (col 7, row 8)
    else if (i === 7) m.set(8, 8, bit === 1); // (col 8, row 8)
    else if (i === 8) m.set(8, 7, bit === 1); // (col 8, row 7)
    else m.set(8, 14 - i, bit === 1); // col 8, rows 5..0 (bit 9 -> row 5 ... bit 14 -> row 0)
    // Copy B: above the TR finder / right of the BL finder.
    if (i < 8) m.set(n - 8 + i, n - 1, bit === 1); // row n-1, cols n-8..n-1
    else m.set(n - 1, n - 7 + (i - 8), bit === 1); // col n-1, rows n-7..n-1
  }
}

// Standard QR data placement: zigzag through 2-column strips from the
// bottom-right, skipping the timing column (col 6), alternating direction.
function placeData(m, codewords) {
  const n = m.size;
  const bits = [];
  for (const b of codewords) {
    for (let j = 7; j >= 0; j--) bits.push((b >> j) & 1);
  }
  let bi = 0;
  let upward = true;
  for (let c = n - 1; c > 0; c -= 2) {
    if (c === 6) c--; // skip timing column
    for (let i = 0; i < n; i++) {
      const r = upward ? n - 1 - i : i;
      for (let k = 0; k < 2; k++) {
        const x = c - k;
        if (x < 0 || x >= n) continue;
        if (!m.isReserved(x, r) && bi < bits.length) {
          m.set(x, r, bits[bi++] === 1);
        }
      }
    }
    upward = !upward;
  }
}

// ---------- penalty scoring (N1-N4) ----------
function penalty(m) {
  const n = m.size;
  let score = 0;
  const row = new Array(n);
  const col = new Array(n);
  // N1: runs of >= 5 identical modules.
  for (let r = 0; r < n; r++) {
    let run = 1;
    for (let c = 1; c < n; c++) {
      if (m.get(c, r) === m.get(c - 1, r)) run++;
      else {
        if (run >= 5) score += 3 + (run - 5);
        run = 1;
      }
    }
    if (run >= 5) score += 3 + (run - 5);
  }
  for (let c = 0; c < n; c++) {
    let run = 1;
    for (let r = 1; r < n; r++) {
      if (m.get(c, r) === m.get(c, r - 1)) run++;
      else {
        if (run >= 5) score += 3 + (run - 5);
        run = 1;
      }
    }
    if (run >= 5) score += 3 + (run - 5);
  }
  // N2: 2x2 blocks of same color.
  for (let r = 0; r < n - 1; r++) {
    for (let c = 0; c < n - 1; c++) {
      const v = m.get(c, r);
      if (v === m.get(c + 1, r) && v === m.get(c, r + 1) && v === m.get(c + 1, r + 1)) score += 3;
    }
  }
  // N3: 1:1:3:1:1 (10111) or 1:3:1:1:1 (11101) patterns.
  for (let r = 0; r < n; r++) {
    for (let c = 0; c + 4 < n; c++) {
      const m0 = m.get(c, r), m1 = m.get(c + 1, r), m2 = m.get(c + 2, r), m3 = m.get(c + 3, r), m4 = m.get(c + 4, r);
      const is10111 = m0 === 1 && m1 === 0 && m2 === 1 && m3 === 1 && m4 === 1;
      const is11101 = m0 === 1 && m1 === 1 && m2 === 1 && m3 === 0 && m4 === 1;
      if (is10111 || is11101) score += 40;
    }
    for (let c = 0; c < n; c++) {
      if (r + 4 < n) {
        const m0 = m.get(c, r), m1 = m.get(c, r + 1), m2 = m.get(c, r + 2), m3 = m.get(c, r + 3), m4 = m.get(c, r + 4);
        const is10111 = m0 === 1 && m1 === 0 && m2 === 1 && m3 === 1 && m4 === 1;
        const is11101 = m0 === 1 && m1 === 1 && m2 === 1 && m3 === 0 && m4 === 1;
        if (is10111 || is11101) score += 40;
      }
    }
  }
  // N4: darkness deviation from 50%.
  let dark = 0;
  for (let i = 0; i < m.data.length; i++) dark += m.data[i];
  const pct = Math.round((dark / m.data.length) * 100);
  score += 10 * (Math.abs(pct - 50) / 50);
  return score;
}

// Reserved-module map for a version: every cell the encoder treats as
// non-data (finders + separators, timing, alignment, format/version info,
// dark module). The decoder uses this to know exactly which zigzag cells
// carry data codewords.
export function buildReserved(version) {
  const n = 17 + 4 * version;
  const m = new QrMatrix(n);
  drawFinders(m);
  drawTiming(m);
  drawAlignments(m, version);
  drawDarkModule(m);
  // Format info regions (mirror of placeFormatBits).
  for (let i = 0; i < 15; i++) {
    if (i <= 5) m.set(i, 8);
    else if (i === 6) m.set(7, 8);
    else if (i === 7) m.set(8, 8);
    else if (i === 8) m.set(8, 7);
    else m.set(8, 14 - i);
    if (i < 8) m.set(n - 8 + i, n - 1);
    else m.set(n - 1, n - 7 + (i - 8));
  }
  if (version >= 7) {
    for (let i = 0; i < 18; i++) {
      m.set(n - 9 + (i % 6), (i / 6) | 0);
      m.set(i % 3, n - 7 + ((i / 3) | 0));
    }
  }
  return m.reserved;
}

// ---------- public encoder ----------
// Encode a pixfer frame into an actual QR code (EC level M).
// Returns { version, size, data } where data is 0/1 modules (1 = dark).
export function qrEncode(frame) {
  const bytes = frameBytes(frame);
  const version = pickVersion(bytes.length);
  const n = 17 + 4 * version;
  const codewords = encodeCodewords(encodeBits(bytes, version), version);

  let best = null;
  let bestScore = Infinity;
  let bestMask = 0;
  for (let mask = 0; mask < 8; mask++) {
    const m = new QrMatrix(n);
    drawFinders(m);
    drawTiming(m);
    drawAlignments(m, version);
    drawDarkModule(m);
    // Apply mask to data modules (only where not reserved).
    const maskFn = MASKS[mask];
    const mn = new QrMatrix(n);
    mn.data.set(m.data);
    mn.reserved.set(m.reserved);
    placeData(mn, codewords);
    for (let r = 0; r < n; r++) {
      for (let c = 0; c < n; c++) {
        if (!mn.reserved[r * n + c] && maskFn(r, c)) {
          mn.data[r * n + c] ^= 1;
        }
      }
    }
    placeFormatBits(mn, mask);
    placeVersionInfo(mn, version);
    const score = penalty(mn);
    if (score < bestScore) {
      bestScore = score;
      bestMask = mask;
      best = mn;
    }
  }
  return { version, size: n, data: best.data };
}