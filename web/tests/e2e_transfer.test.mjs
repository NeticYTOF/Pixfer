// End-to-end transfer test: proves a file actually moves from sender to
// receiver through the full pipeline:
//
//   file bytes -> transferPlan (auto-zip when est > 30s)
//   -> chunking (223-byte payloads)
//   -> TX session frames -> renderBlocks
//   -> simulated camera capture with random block corruption:
//      - light corruption (5 blocks) stays within the RS T=16 bound -> corrected
//      - heavy corruption (25 blocks) exceeds the bound -> decode fails ->
//        RX NAKs the lowest missing chunk -> TX retransmits (ARQ)
//   -> RX decodeBlocks + TransferSession retransmit loop
//   -> reassembly -> byte-exact comparison with the original bytes
//
// Run: node tests/e2e_transfer.test.mjs (from web/)

import * as P from "../js/core.js";
import { Buffer } from "node:buffer";

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

// Deterministic byte source, mirrors the LCG helper in roundtrip.test.mjs.
function buildData(n, seed) {
  const v = new Uint8Array(n);
  let x = seed >>> 0;
  for (let i = 0; i < n; i++) {
    x = (Math.imul(x, 1103515245) + 12345) | 0;
    v[i] = (x >>> 16) & 0xff;
  }
  return v;
}

// Corrupt `count` distinct blocks spread across the whole RS codeword
// region (parity + payload blocks). Corrupting a block flips its 3 bits,
// i.e. 1-2 codeword byte errors per block. The TR finder's overlap adds a
// fixed ~12-byte baseline; spreading the corruption across the full region
// (deterministic stride, coprime with the span) keeps the added errors
// from being absorbed into that baseline.
function corruptBlocks(blocks, count) {
  const start = P.PARITY_START;
  const end = P.PAYLOAD_START + P.PAYLOAD_BLOCKS;
  const span = end - start;
  const step = 97; // coprime with 681 -> full-coverage stride
  for (let k = 0; k < count; k++) {
    const i = start + ((k * step) % span);
    blocks[i] = blocks[i] ^ 7;
  }
  return blocks;
}

// ---------- 1. auto-zip threshold (mirrors tx.js behavior) ----------
{
  const planBig = P.transferPlan(70_000_000); // est 35s > 30s
  check("e2e_plan: 70MB auto-zips (est 35s > 30s)", planBig.shouldZip === true && planBig.estSeconds === 35);
  const planSmall = P.transferPlan(40_000_000); // est 20s < 30s
  check("e2e_plan: 40MB sends as-is (est 20s < 30s)", planSmall.shouldZip === false && planSmall.estSeconds === 20);
  check("e2e_plan: threshold is exact 30s boundary", P.needsZip(60_000_000) === false && P.needsZip(60_000_001) === true);
}

// ---------- 2. full transfer with loss + NAK retransmit ----------
{
  const N_CHUNKS = 100;
  const data = buildData(N_CHUNKS * P.RS_K, 42);
  const sessionId = 42;

  const tx = P.TransferSession.tx(sessionId, N_CHUNKS);
  const rx = P.TransferSession.rx(sessionId, N_CHUNKS);
  const chunksRx = new Array(N_CHUNKS).fill(null);
  const attempts = new Map(); // chunk -> number of transmissions

  function chunkPayload(idx) {
    const start = idx * P.RS_K;
    return data.subarray(start, Math.min(data.length, start + P.RS_K));
  }

  let totalBytes = 0;
  let naks = 0;
  let rsCorrected = 0;

  function lowestMissing() {
    for (let i = 0; i < N_CHUNKS; i++) {
      if (!chunksRx[i]) return i;
    }
    return 0;
  }

  // Handshake: Ping -> Pong.
  let f = tx.nextFrame();
  check("e2e_handshake: tx starts with Ping", f !== null && f.kind === "Ping");
  rx.onPing();
  const rf0 = rx.nextFrame();
  check("e2e_handshake: rx answers Pong", rf0 !== null && rf0.kind === "Pong");
  tx.onPong();
  check("e2e_handshake: tx enters Transferring", tx.state === "Transferring");

  let guard = 0;
  while (tx.state !== "Done" && guard < 10000) {
    guard++;
    const out = tx.nextFrame();
    if (!out) continue; // waiting for the peer's ack/naq frame
    if (out.kind !== "Chunk") continue; // End frame

    const n = (attempts.get(out.chunkIdx) || 0) + 1;
    attempts.set(out.chunkIdx, n);

    // Simulated camera: first transmission of every 3rd chunk is hit with
    // heavy corruption (exceeds the RS bound -> NAK + retransmit); every
    // other capture carries light corruption (within the RS bound ->
    // corrected). The TR finder always overlaps ~12 payload bytes by design;
    // light (2 blocks -> 14 total) stays <= T=16, heavy (4 blocks -> 17
    // total) exceeds it.
    out.payload = chunkPayload(out.chunkIdx);
    const blocks = P.renderBlocks(out);
    const heavy = n === 1 && out.chunkIdx % 3 === 0;
    if (heavy) {
      corruptBlocks(blocks, 10); // 12 baseline + 10..20 = 22..32 > T=16 -> decode fails
    } else {
      corruptBlocks(blocks, 2); // 12 baseline + 2..4 = 14..16 <= 16 -> RS corrects
    }

    const dec = P.decodeBlocks(blocks);
    if (dec.ok) {
      if (!heavy) rsCorrected++;
      chunksRx[out.chunkIdx] = dec.frame.payload;
      totalBytes += dec.frame.payload.length;
      rx.onChunkOk(out.chunkIdx);
    } else {
      // Mirrors the new rx.js path: decode failed -> NAK the lowest
      // unreceived chunk so the sender retransmits it.
      naks++;
      rx.onChunkErr(lowestMissing());
    }
    const rf = rx.nextFrame();
    if (rf) {
      if (rf.kind === "Ack") tx.onAck(rf.chunkIdx);
      else if (rf.kind === "Nak") tx.onNak(rf.chunkIdx);
    }
  }

  check("e2e_loop: transfer completed", tx.state === "Done", "state=" + tx.state);
  check("e2e_loss: some chunks were NAKed and retransmitted", naks >= N_CHUNKS / 3 - 1 && naks <= N_CHUNKS / 3 + 1, `naks=${naks}`);
  check("e2e_loss: light corruption corrected by RS", rsCorrected > 0, "rsCorrected=" + rsCorrected);
  check("e2e_rx: all chunks received", rx.allReceived(), "received=" + (rx.received ? rx.received.size : "?"));

  // Reassemble in chunk order and compare byte-for-byte.
  const out = new Uint8Array(totalBytes);
  let off = 0;
  for (let i = 0; i < N_CHUNKS; i++) {
    const p = chunksRx[i];
    out.set(p, off);
    off += p.length;
  }
  check("e2e_bytes: reassembled file matches original", Buffer.from(out).equals(Buffer.from(data)));
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}