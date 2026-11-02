// pixfer web dedup — skip files the receiver already has, using an FNV-1a
// 64-bit fingerprint as the identity. Mirrors `pixfer-core/src/dedup.rs`.

// 64-bit FNV-1a over a byte-like array.
export function fnv1a64(bytes) {
  // Use BigInt literals directly: a plain Number literal (0xcbf29ce484222325)
  // exceeds Number.MAX_SAFE_INTEGER and rounds, so we must not route it
  // through BigInt(Number).
  const OFFSET = 0xcbf29ce484222325n;
  const PRIME = 0x00000100000001b3n;
  let h = OFFSET;
  for (const b of bytes) {
    h ^= BigInt(b & 0xff);
    h = (h * PRIME) & 0xffffffffffffffffn;
  }
  return h;
}

// A file identity: FNV-1a 64-bit hash + size.
export class FileFingerprint {
  constructor(hash, size) {
    this.hash = hash; // BigInt
    this.size = size;
  }

  static ofBytes(bytes) {
    return new FileFingerprint(fnv1a64(bytes), bytes.length);
  }
}

// A set of fingerprints the receiver already holds.
export class DedupSet {
  constructor() {
    this.seen = new Map(); // hash -> size
  }

  add(fp) {
    this.seen.set(fp.hash, fp.size);
    return this;
  }

  has(fp) {
    return this.seen.has(fp.hash);
  }

  get size() {
    return this.seen.size;
  }
}

export const DedupDecision = Object.freeze({
  TRANSFER: "transfer",
  SKIP: "skip",
});

// Decide whether a file should be transferred.
export function decide(fp, rx) {
  return rx.has(fp) ? DedupDecision.SKIP : DedupDecision.TRANSFER;
}

// A pipeline-stage wrapper: only forward files not already present.
export class DedupStage {
  constructor(rx) {
    this.rx = rx;
    this.skipped = 0;
    this.transferred = 0;
  }

  process(fp) {
    const d = decide(fp, this.rx);
    if (d === DedupDecision.SKIP) this.skipped += 1;
    else this.transferred += 1;
    return d;
  }

  skippedCount() {
    return this.skipped;
  }

  transferredCount() {
    return this.transferred;
  }
}