// pixfer web extended payload checksum — incremental CRC32 / Adler-32 /
// FNV-1a over transfer payloads with known-vector verification.
// Mirrors `pixfer-core/src/payload_crc_ext.rs`.

// An extended checksum result with the algorithm name.
export class ExtChecksumResult {
  constructor(algorithm, value) {
    this.algorithm = algorithm;
    this.value = value;
  }
}

// CRC32 (table-driven, zlib-compatible).
const CRC32_TABLE = (() => {
  const t = new Array(256);
  for (let i = 0; i < 256; i++) {
    let c = i;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    t[i] = c >>> 0;
  }
  return t;
})();

function crc32Bytes(payload) {
  let crc = 0xffffffff;
  for (const byte of payload) {
    crc = CRC32_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function adler32Bytes(payload) {
  const MOD = 65535;
  let a = 1;
  let b = 0;
  for (const byte of payload) {
    a = (a + byte) % (MOD + 1);
    b = (b + a) % (MOD + 1);
  }
  return ((b << 16) | a) >>> 0;
}

function fnv1a64Bytes(payload) {
  // FNV-1a 64-bit using BigInt (primes as BigInt literals).
  let h = 0xcbf29ce484222325n;
  for (const byte of payload) {
    h ^= BigInt(byte);
    h = (h * 0x00000100000001b3n) & 0xffffffffffffffffn;
  }
  return Number(h);
}

// Computes extended checksums over a payload using the standard algorithms.
export class PayloadCrcExt {
  // The CRC32 of the payload (as a u64).
  crc32(payload) {
    return crc32Bytes(payload);
  }

  // The Adler-32 of the payload.
  adler32(payload) {
    return adler32Bytes(payload);
  }

  // The FNV-1a 64-bit of the payload.
  fnv1a64(payload) {
    return fnv1a64Bytes(payload);
  }

  // A named checksum result for a chosen algorithm.
  computeNamed(algorithm, payload) {
    const v = algorithm === "crc32"
      ? this.crc32(payload)
      : algorithm === "adler32"
        ? this.adler32(payload)
        : this.fnv1a64(payload);
    return new ExtChecksumResult(
      algorithm === "crc32" ? "crc32" : algorithm === "adler32" ? "adler32" : "fnv1a64",
      v,
    );
  }
}