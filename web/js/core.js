// pixfer web protocol core — pure-JS mirror of the Rust `pixfer-core` crate.
//
// Multicolor block-grid wire format: each 4x4-px block carries 3 bits (R/G/B on/off =
// 8-color palette), 3x the bandwidth of a monochrome scheme. Frames are self-contained:
// QR-style sync finders + 23-byte header (magic "PIXF", kind, session id, chunk idx, total,
// payload len, CRC32) + 32 RS parity bytes + 223-byte payload (parity-first codeword).
//
// Keep this file in lockstep with `pixfer-core/src/{gf256,rs,protocol,session,transfer}.rs`.

// ---------- grid geometry ----------
export const GRID_W = 270;
export const GRID_H = 480;
export const BLOCK_PX = 4;
export const FINDER_SIZE = 7;
export const MAGIC = "PIXF";
export const HEADER_BYTES = 23;
export const RS_N = 255;
export const RS_K = 223;
export const RS_T = 16;
export const RS_PARITY = RS_N - RS_K; // 32

const FINDER_TL = [0, 0];
const FINDER_TR = [GRID_W - FINDER_SIZE, 0];
const FINDER_BL = [0, GRID_H - FINDER_SIZE];

export const HEADER_START = FINDER_SIZE * FINDER_SIZE; // 49
export const PARITY_START = HEADER_START + ((HEADER_BYTES * 8 + 2) / 3) | 0; // 111
export const PAYLOAD_START = PARITY_START + ((RS_PARITY * 8 + 2) / 3) | 0; // 197
export const HEADER_BLOCKS = ((HEADER_BYTES * 8 + 2) / 3) | 0; // 62
export const PARITY_BLOCKS = ((RS_PARITY * 8 + 2) / 3) | 0; // 86
export const PAYLOAD_BLOCKS = ((RS_K * 8 + 2) / 3) | 0; // 595

export const FINDER = [
  [7, 7, 7, 7, 7, 7, 7],
  [7, 0, 0, 0, 0, 0, 7],
  [7, 0, 0, 0, 0, 0, 7],
  [7, 0, 7, 7, 7, 0, 7],
  [7, 0, 7, 7, 7, 0, 7],
  [7, 0, 0, 0, 0, 0, 7],
  [7, 7, 7, 7, 7, 7, 7],
];

// 8-color palette for the 3-bit blocks (R=bit0, G=bit1, B=bit2).
export const PALETTE = [
  [0, 0, 0],       // 0b000 black
  [255, 0, 0],     // 0b001 red
  [0, 255, 0],     // 0b010 green
  [255, 255, 0],   // 0b011 yellow
  [0, 0, 255],     // 0b100 blue
  [255, 0, 255],   // 0b101 magenta
  [0, 255, 255],   // 0b110 cyan
  [255, 255, 255], // 0b111 white
];

// ---------- frame kinds ----------
export const FRAME_KIND = Object.freeze({
  Ping: 0,
  Pong: 1,
  Chunk: 2,
  Ack: 3,
  Nak: 4,
  End: 5,
});
const KIND_NAMES = ["Ping", "Pong", "Chunk", "Ack", "Nak", "End"];

export function frameKindFromU8(v) {
  return v >= 0 && v < KIND_NAMES.length ? KIND_NAMES[v] : null;
}

export function newFrame(kind, sessionId, chunkIdx, totalChunks, payload) {
  return {
    kind,
    sessionId,
    chunkIdx,
    totalChunks,
    payload: payload ? new Uint8Array(payload) : new Uint8Array(0),
  };
}

// ---------- GF(256) ----------
// GF(256) arithmetic over primitive polynomial 0x11D, precomputed exp/log tables.
function buildGf256() {
  const exp = new Uint16Array(512);
  const log = new Uint8Array(256);
  let x = 1;
  for (let i = 0; i < 255; i++) {
    exp[i] = x;
    log[x] = i;
    x = x << 1;
    if (x & 0x100) x ^= 0x11D;
  }
  for (let i = 255; i < 512; i++) exp[i] = exp[i - 255];
  return {
    exp,
    log,
    mul(a, b) {
      if (a === 0 || b === 0) return 0;
      return exp[(log[a] + log[b]) % 255];
    },
    div(a, b) {
      if (a === 0) return 0;
      let idx = (log[a] - log[b]) % 255;
      if (idx < 0) idx += 255;
      return exp[idx];
    },
  };
}
const GF = buildGf256();

// ---------- Reed-Solomon RS(255,223,16) ----------
// Systematic encoder (parity-first codeword [parity_0..31][msg_0..222]) + full decode
// pipeline (syndromes, Berlekamp-Massey, Chien search, Forney magnitudes).

export class ReedSolomon {
  constructor() {
    // g(x) = prod_{i=0}^{2T-1} (x - a^i), stored in DESCENDING order so gen[0] == 1.
    let gen = [1];
    for (let i = 0; i < 2 * RS_T; i++) {
      const root = GF.exp[i];
      const next = new Array(gen.length + 1).fill(0);
      for (let j = 0; j < gen.length; j++) {
        const c = gen[j];
        if (c !== 0) {
          // (x - root) * g: x*g shifts up, root*g stays; XOR contributions (GF addition).
          next[j] ^= c;
          next[j + 1] ^= GF.mul(c, root);
        }
      }
      gen = next;
    }
    this.gen = gen;
  }

  generator() {
    return this.gen;
  }

  // Systematic encode. Codeword layout: [parity_0..31][msg_0..222].
  encode(msg) {
    if (msg.length !== RS_K) {
      throw new Error(`expected ${RS_K} message bytes, got ${msg.length}`);
    }
    const dividend = new Uint8Array(RS_N);
    for (let i = 0; i < RS_K; i++) dividend[i + RS_PARITY] = msg[i];
    // Long division from the highest coefficient downward (254 -> 32).
    for (let i = RS_N - 1; i >= RS_PARITY; i--) {
      const c = dividend[i];
      if (c !== 0) {
        for (let j = 0; j < this.gen.length; j++) {
          const g = this.gen[j];
          if (g !== 0) dividend[i - j] ^= GF.mul(c, g);
        }
      }
    }
    const out = new Uint8Array(RS_N);
    out.set(dividend.subarray(0, RS_PARITY), 0); // parity first
    out.set(msg, RS_PARITY); // then payload
    return out;
  }

  // The 2T syndromes S_l = c(a^l) for l = 0..2T-1.
  syndromes(coded) {
    const s = new Uint8Array(2 * RS_T);
    for (let i = 0; i < 2 * RS_T; i++) {
      const x = GF.exp[i];
      let acc = 0;
      // c(x) = c_0 + c_1 x + ... + c_{N-1} x^{N-1}; Horner from highest coefficient down.
      for (let k = coded.length - 1; k >= 0; k--) {
        acc = GF.mul(acc, x) ^ coded[k];
      }
      s[i] = acc;
    }
    return s;
  }

  // Decode and correct up to RS_T symbol errors.
  // `coded` layout: [parity_0..31][payload_0..222].
  decode(coded) {
    if (coded.length !== RS_N) return { ok: false, error: "BadLength" };
    const s = this.syndromes(coded);
    if (s.every((v) => v === 0)) {
      return { ok: true, payload: new Uint8Array(coded.subarray(RS_PARITY)) };
    }
    const loc = this.berlekampMassey(s);
    const omega = this.omega(s, loc);

    // Chien search: the BM connection polynomial is C(x) = prod_j (1 - a^j x), so its roots
    // are x = a^{-j}. A root a^i therefore corresponds to error position j = (N - i) mod N.
    const errs = [];
    for (let i = 0; i < RS_N; i++) {
      const x = GF.exp[i];
      if (this.polyEval(loc, x) === 0) {
        errs.push((RS_N - i) % RS_N);
      }
    }
    if (errs.length > RS_T) return { ok: false, error: "TooManyErrors" };

    const out = new Uint8Array(coded);
    for (const pos of errs) {
      // Forney (b=0): evaluate at x = a^{-pos} = a^{(N - pos) mod N}, then rescale by a^pos.
      const x = GF.exp[(RS_N - pos) % RS_N];
      const num = this.polyEval(omega, x);
      const den = this.polyDerEval(loc, x);
      if (den === 0) return { ok: false, error: "CheckFailed" };
      const mag = GF.mul(GF.div(num, den), GF.exp[pos % RS_N]);
      out[pos] ^= mag;
    }
    // Recheck: if more errors exist than the code can correct, residual syndromes remain.
    if (this.syndromes(out).some((v) => v !== 0)) {
      return { ok: false, error: "TooManyErrors" };
    }
    return { ok: true, payload: new Uint8Array(out.subarray(RS_PARITY)) };
  }

  berlekampMassey(s) {
    let c = [1];
    let b = [1];
    let l = 0;
    let m = 1;
    let oldDiff = 1;
    for (let i = 0; i < s.length; i++) {
      let diff = s[i];
      for (let j = 1; j < c.length; j++) {
        if (i >= j && c[j] !== 0) diff ^= GF.mul(c[j], s[i - j]);
      }
      if (diff === 0) {
        m += 1;
      } else {
        const t = c.slice();
        const coef = GF.div(diff, oldDiff);
        while (b.length < c.length) b.push(0);
        while (c.length < b.length + m) c.push(0);
        for (let j = 0; j < b.length; j++) {
          if (b[j] !== 0) c[j + m] ^= GF.mul(coef, b[j]);
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

  // omega(x) = S(x) * Lambda(x), truncated to degree 2T-1.
  omega(s, loc) {
    const o = new Uint8Array(2 * RS_T);
    for (let i = 0; i < loc.length; i++) {
      const li = loc[i];
      if (li === 0) continue;
      for (let j = 0; j < s.length; j++) {
        const sj = s[j];
        if (sj === 0) continue;
        const k = i + j;
        if (k < o.length) o[k] ^= GF.mul(li, sj);
      }
    }
    return o;
  }

  // Evaluate a polynomial (ascending coefficients) at x (Horner from highest coefficient down).
  polyEval(poly, x) {
    let acc = 0;
    for (let k = poly.length - 1; k >= 0; k--) {
      acc = GF.mul(acc, x) ^ poly[k];
    }
    return acc;
  }

  // Evaluate the formal derivative at x (char 2: even powers vanish).
  polyDerEval(poly, x) {
    // P'(x) = sum_{j odd} c_j x^{j-1}; coefficient of x^k is c_{k+1} when k even.
    let acc = 0;
    for (let k = poly.length - 2; k >= 0; k--) {
      const cVal = k % 2 === 0 ? poly[k + 1] : 0;
      acc = GF.mul(acc, x) ^ cVal;
    }
    return acc;
  }
}

// ---------- block I/O (3 bits per block: R=0b001, G=0b010, B=0b100) ----------

function writeBlocks(blocks, start, data) {
  let bitpos = 0;
  for (let bi = 0; bi < data.length; bi++) {
    const byte = data[bi];
    for (let bit = 7; bit >= 0; bit--) {
      const blockIdx = start + (bitpos / 3) | 0;
      const rem = bitpos % 3;
      if ((byte >> bit) & 1) {
        if (rem === 0) blocks[blockIdx] |= 0b001; // R
        else if (rem === 1) blocks[blockIdx] |= 0b010; // G
        else blocks[blockIdx] |= 0b100; // B
      }
      bitpos++;
    }
  }
}

export function readBlocks(blocks, out) {
  const bitsNeeded = out.length * 8;
  const blocksNeeded = ((bitsNeeded + 2) / 3) | 0;
  if (blocks.length < blocksNeeded) {
    throw new Error(`block run too short: have ${blocks.length}, need ${blocksNeeded}`);
  }
  const bits = [];
  for (let i = 0; i < blocks.length; i++) {
    const b = blocks[i];
    bits.push(b & 1, (b >> 1) & 1, (b >> 2) & 1);
  }
  for (let i = 0; i < out.length; i++) {
    let v = 0;
    for (let j = 0; j < 8; j++) {
      if (bits[i * 8 + j] === 1) v |= 1 << (7 - j);
    }
    out[i] = v;
  }
}

function placeFinder(blocks, at) {
  const [c0, r0] = at;
  for (let r = 0; r < FINDER_SIZE; r++) {
    for (let c = 0; c < FINDER_SIZE; c++) {
      blocks[(r0 + r) * GRID_W + c0 + c] = FINDER[r][c];
    }
  }
}

// ---------- little-endian helpers ----------
function setU32le(buf, off, v) {
  buf[off] = v & 0xff;
  buf[off + 1] = (v >>> 8) & 0xff;
  buf[off + 2] = (v >>> 16) & 0xff;
  buf[off + 3] = (v >>> 24) & 0xff;
}

export function u32le(buf, off) {
  const v =
    buf[off] |
    (buf[off + 1] << 8) |
    (buf[off + 2] << 16) |
    (buf[off + 3] << 24);
  return v >>> 0;
}

// ---------- CRC32 (zlib/ETH, reflected poly 0xEDB88320) ----------
let crcTable = null;
export function crc32(bytes) {
  if (!crcTable) {
    crcTable = new Uint32Array(256);
    for (let i = 0; i < 256; i++) {
      let c = i;
      for (let k = 0; k < 8; k++) {
        c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      }
      crcTable[i] = c >>> 0;
    }
  }
  let crc = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) {
    crc = crcTable[(crc ^ bytes[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

// ---------- frame render/decode ----------
function buildHeader(frame) {
  const head = new Uint8Array(HEADER_BYTES);
  head.set([0x50, 0x49, 0x58, 0x46], 0); // "PIXF"
  head[4] = FRAME_KIND[frame.kind];
  head[5] = frame.sessionId & 0xff;
  head[6] = (frame.sessionId >>> 8) & 0xff;
  setU32le(head, 7, frame.chunkIdx);
  setU32le(head, 11, frame.totalChunks);
  setU32le(head, 15, frame.payload ? frame.payload.length : 0);
  const crcInputLen = 19 + (frame.payload ? frame.payload.length : 0);
  const ci = new Uint8Array(crcInputLen);
  ci.set(head.subarray(0, 19), 0);
  if (frame.payload && frame.payload.length) ci.set(frame.payload, 19);
  setU32le(head, 19, crc32(ci));
  return head;
}

// Render a frame into the full 270x480 grid of 8-color block indices.
export function renderBlocks(frame) {
  const blocks = new Uint8Array(GRID_W * GRID_H);
  // Filler cycles all 8 palette colors so the time-averaged screen color stays neutral.
  for (let i = 0; i < blocks.length; i++) blocks[i] = i % 8;

  // Zero the data region (header + parity + payload) so the filler colors don't OR-into the
  // data bits and corrupt them on capture.
  for (let i = HEADER_START; i < PAYLOAD_START + PAYLOAD_BLOCKS; i++) blocks[i] = 0;

  const head = buildHeader(frame);
  writeBlocks(blocks, HEADER_START, head);

  const payload = new Uint8Array(RS_K);
  if (frame.payload && frame.payload.length) {
    payload.set(frame.payload);
  }
  const rs = new ReedSolomon();
  const coded = rs.encode(payload); // [parity_0..31][payload_0..222]
  writeBlocks(blocks, PARITY_START, coded.subarray(0, RS_PARITY));
  writeBlocks(blocks, PAYLOAD_START, payload);

  // Finders go last so they win in the blocks the top-right finder overlaps (its rows 0-1
  // sit inside the payload region); RS-FEC corrects the few lost payload symbols.
  placeFinder(blocks, FINDER_TL);
  placeFinder(blocks, FINDER_TR);
  placeFinder(blocks, FINDER_BL);
  return blocks;
}

// Decode a captured block grid back into a frame.
export function decodeBlocks(blocks) {
  if (blocks.length !== GRID_W * GRID_H) return { ok: false, error: "BadLength" };
  const head = new Uint8Array(HEADER_BYTES);
  readBlocks(blocks.subarray(HEADER_START, HEADER_START + HEADER_BLOCKS), head);
  if (head[0] !== 0x50 || head[1] !== 0x49 || head[2] !== 0x58 || head[3] !== 0x46) {
    return { ok: false, error: "BadMagic" };
  }
  const kind = frameKindFromU8(head[4]);
  if (!kind) return { ok: false, error: "DecodeFailed" };
  const sessionId = (head[6] << 8) | head[5];
  const chunkIdx = u32le(head, 7);
  const totalChunks = u32le(head, 11);
  // Clamp: a corrupted header can carry a garbage payload_len; the payload region physically
  // holds at most RS_K bytes.
  const payloadLen = Math.min(u32le(head, 15), RS_K);
  const crcStored = u32le(head, 19);

  const parity = new Uint8Array(RS_PARITY);
  readBlocks(blocks.subarray(PARITY_START, PARITY_START + PARITY_BLOCKS), parity);
  const payloadFull = new Uint8Array(RS_K);
  readBlocks(blocks.subarray(PAYLOAD_START, PAYLOAD_START + PAYLOAD_BLOCKS), payloadFull);

  // Layered integrity: RS-correct first, then verify CRC on corrected data.
  const codeword = new Uint8Array(RS_N);
  codeword.set(parity, 0);
  codeword.set(payloadFull, RS_PARITY);
  const rs = new ReedSolomon();
  const dec = rs.decode(codeword);
  let payload;
  if (dec.ok) {
    payload = new Uint8Array(dec.payload.subarray(0, payloadLen));
  } else {
    // Last-resort CRC check on the raw payload
    const ci = new Uint8Array(19 + payloadLen);
    ci.set(head.subarray(0, 19), 0);
    ci.set(payloadFull.subarray(0, payloadLen), 19);
    if (crc32(ci) === crcStored) {
      payload = new Uint8Array(payloadFull.subarray(0, payloadLen));
    } else {
      return { ok: false, error: "BadCrc" };
    }
  }

  const ci = new Uint8Array(19 + payload.length);
  ci.set(head.subarray(0, 19), 0);
  ci.set(payload, 19);
  if (crc32(ci) !== crcStored) return { ok: false, error: "BadCrc" };

  return {
    ok: true,
    frame: { kind, sessionId, chunkIdx, totalChunks, payload },
  };
}

// ---------- transfer planning (mirrors transfer.rs) ----------
// Effective multicolor throughput estimate (~2 MB/s after FEC + sync overhead).
export const DEFAULT_THROUGHPUT = 2_000_000;
// Zip if estimated transfer takes longer than this many seconds.
export const ZIP_THRESHOLD_S = 30;

export function estimateSeconds(size) {
  return size / DEFAULT_THROUGHPUT;
}

export function needsZip(size) {
  return estimateSeconds(size) > ZIP_THRESHOLD_S;
}

// (shouldZip, estimated_seconds) for a file of `size` bytes.
export function transferPlan(size) {
  const est = estimateSeconds(size);
  return { shouldZip: est > ZIP_THRESHOLD_S, estSeconds: est };
}

// ---------- transfer session (mirrors session.rs) ----------
// Stop-and-wait ARQ with bounded retries, used by both ends.
const MAX_RETRIES = 3;

export class TransferSession {
  constructor(role, sessionId, totalChunks) {
    this.role = role;
    this.sessionId = sessionId;
    this.totalChunks = totalChunks;
    this.state = "WaitingPong";
    this.nextChunk = 0;
    this.pendingChunk = null;
    this.nakNext = false;
    this.pongSent = false;
    this.retries = new Map();
    this.received = new Set();
  }

  static tx(sessionId, totalChunks) {
    return new TransferSession("tx", sessionId, totalChunks);
  }

  static rx(sessionId, totalChunks) {
    return new TransferSession("rx", sessionId, totalChunks);
  }

  // The next frame this side should render. Returns null when idle.
  nextFrame() {
    if (this.role === "tx") {
      switch (this.state) {
        case "WaitingPong":
          return newFrame("Ping", this.sessionId, 0, this.totalChunks, null);
        case "Transferring":
          if (this.nextChunk < this.totalChunks) {
            const f = newFrame("Chunk", this.sessionId, this.nextChunk, this.totalChunks, null);
            this.pendingChunk = this.nextChunk;
            this.state = "AwaitingAck";
            return f;
          }
          this.state = "Done";
          return newFrame("End", this.sessionId, this.nextChunk, this.totalChunks, null);
        case "AwaitingAck":
          return this.pendingChunk !== null
            ? newFrame("Chunk", this.sessionId, this.pendingChunk, this.totalChunks, null)
            : null;
        default:
          return null;
      }
    }
    // rx role
    if (!this.pongSent) {
      this.pongSent = true;
      this.state = "Transferring";
      return newFrame("Pong", this.sessionId, 0, this.totalChunks, null);
    }
    if (this.pendingChunk !== null) {
      const kind = this.nakNext ? "Nak" : "Ack";
      return newFrame(kind, this.sessionId, this.pendingChunk, this.totalChunks, null);
    }
    return null;
  }

  // TX: peer answered our PING with a PONG — start the transfer.
  onPong() {
    if (this.role === "tx" && this.state === "WaitingPong") {
      this.state = "Transferring";
    }
  }

  // RX: peer sent a PING — mark the session started; nextFrame will emit the PONG.
  onPing() {
    if (this.role === "rx") this.state = "Transferring";
  }

  // RX: chunk verified OK — queue an ACK.
  onChunkOk(chunk) {
    if (this.role === "rx") {
      this.pendingChunk = chunk;
      this.nakNext = false;
      this.state = "AwaitingAck";
      this.received.add(chunk);
    }
  }

  // RX: chunk failed CRC/RS — queue a NAK.
  onChunkErr(chunk) {
    if (this.role === "rx") {
      this.pendingChunk = chunk;
      this.nakNext = true;
      this.state = "AwaitingAck";
    }
  }

  // TX: peer ACKed a chunk — advance.
  onAck(chunk) {
    if (this.role === "tx" && this.state === "AwaitingAck" && this.pendingChunk === chunk) {
      this.nextChunk += 1;
      this.pendingChunk = null;
      this.state = "Transferring"; // next_frame will now emit END when done
    }
  }

  // TX: peer NAKed a chunk — retransmit it.
  onNak(chunk) {
    if (this.role === "tx" && this.state === "AwaitingAck") {
      this.pendingChunk = chunk;
    }
  }

  // TX: no ACK arrived in time — retry the pending chunk, up to MAX_RETRIES.
  onTimeout() {
    if (this.role !== "tx" || this.state !== "AwaitingAck") return null;
    if (this.pendingChunk === null) return null;
    const chunk = this.pendingChunk;
    const n = (this.retries.get(chunk) || 0) + 1;
    this.retries.set(chunk, n);
    if (n > MAX_RETRIES) {
      this.state = "Failed";
      return null;
    }
    return newFrame("Chunk", this.sessionId, chunk, this.totalChunks, null);
  }

  // RX bookkeeping: has every chunk been received?
  markReceived(chunk) {
    this.received.add(chunk);
  }

  allReceived() {
    return this.received.size === this.totalChunks;
  }
}