// Protocol roundtrip tests for the JS core — mirrors pixfer-core/tests/roundtrip.rs.
// Run: node tests/roundtrip.test.mjs (from web/)

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

// Deterministic pseudo-payload generator, mirrors the Rust LCG helper.
function chunkPayload(seed) {
  const v = new Uint8Array(P.RS_K);
  let x = seed >>> 0;
  for (let i = 0; i < P.RS_K; i++) {
    x = (Math.imul(x, 1103515245) + 12345) | 0;
    v[i] = (x >>> 16) & 0xff;
  }
  return v;
}


// 1. CRC32 known vector (zlib): crc32("123456789") == 0xCBF43926
{
  const crc = P.crc32(Buffer.from("123456789"));
  check("crc32('123456789') == 0xCBF43926", crc === 0xcbf43926, `got 0x${crc.toString(16)}`);
}

// 2. Chunk frame roundtrip (mirrors frame_roundtrip_chunk)
{
  const payload = chunkPayload(1);
  const frame = P.newFrame("Chunk", 7, 3, 9, payload);
  const blocks = P.renderBlocks(frame);
  const dec = P.decodeBlocks(blocks);
  const ok =
    dec.ok &&
    dec.frame.kind === "Chunk" &&
    dec.frame.sessionId === 7 &&
    dec.frame.chunkIdx === 3 &&
    dec.frame.totalChunks === 9 &&
    Buffer.from(dec.frame.payload).equals(Buffer.from(payload));
  check("chunk frame roundtrip", ok, `ok=${dec.ok} err=${dec.error}`);
}

// 3. Control frame roundtrip (mirrors frame_roundtrip_control)
{
  const frame = P.newFrame("Ping", 42, 0, 9, null);
  const dec = P.decodeBlocks(P.renderBlocks(frame));
  check(
    "control frame roundtrip",
    dec.ok && dec.frame.kind === "Ping" && dec.frame.payload.length === 0,
    `ok=${dec.ok} err=${dec.error}`
  );
}

// 4. RS(255,223,16) corrects exactly t=16 symbol errors; 17 exceed t.
{
  const rs = new P.ReedSolomon();
  const msg = new Uint8Array(P.RS_K);
  for (let i = 0; i < P.RS_K; i++) msg[i] = (i * 7 + 3) & 0xff;
  const coded = rs.encode(msg);

  const c16 = new Uint8Array(coded);
  for (let n = 0; n < 16; n++) c16[100 + n * 9] ^= 0xff;
  let d = rs.decode(c16);
  let same = true;
  if (d.ok) {
    for (let i = 0; i < P.RS_K; i++) if (d.payload[i] !== msg[i]) same = false;
  }
  check("rs corrects 16 symbol errors", d.ok && same, d.error || "");

  const c17 = new Uint8Array(coded);
  for (let n = 0; n < 17; n++) c17[100 + n * 9] ^= 0xff;
  d = rs.decode(c17);
  check("rs fails at 17 symbol errors", !d.ok, d.error || "");
}

// 5. Protocol level: the TR finder overlaps the payload region and corrupts
// 12 symbols (bytes 24-29, 126-129 for a sequential payload). The total
// error budget is 16, so at most 4 additional flipped bytes keep us within t.
{
  const seq = new Uint8Array(P.RS_K);
  for (let i = 0; i < P.RS_K; i++) seq[i] = i & 0xff;

  const blocks = P.renderBlocks(P.newFrame("Chunk", 9, 4, 6, seq));
  for (const i of [33, 36, 39, 42]) {
    blocks[P.PAYLOAD_START + ((8 * i) / 3) | 0] ^= 0b111; // 1 symbol each
  }
  let dec = P.decodeBlocks(blocks);
  check(
    "protocol: 16 total errors (12 finder + 4 flipped) correct",
    dec.ok && Buffer.from(dec.frame.payload).equals(Buffer.from(seq)),
    `ok=${dec.ok} err=${dec.error}`
  );

  const blocks2 = P.renderBlocks(P.newFrame("Chunk", 9, 4, 6, seq));
  for (const i of [33, 36, 39, 42, 45]) {
    blocks2[P.PAYLOAD_START + ((8 * i) / 3) | 0] ^= 0b111;
  }
  dec = P.decodeBlocks(blocks2);
  check(
    "protocol: 17 total errors (12 finder + 5 flipped) -> BadCrc",
    !dec.ok && dec.error === "BadCrc",
    `ok=${dec.ok} err=${dec.error}`
  );
}

// 6. Header corruption -> BadCrc (mirrors crc_catches_header_corruption)
{
  const frame = P.newFrame("Chunk", 1, 2, 5, chunkPayload(2));
  const blocks = P.renderBlocks(frame);
  blocks[99] ^= 0b101; // header region (HEADER_START=49 .. 111)
  const dec = P.decodeBlocks(blocks);
  check("header corruption -> BadCrc", !dec.ok && dec.error === "BadCrc", `ok=${dec.ok} err=${dec.error}`);
}

// 7. Auto-zip threshold (mirrors needs_zip_threshold): ~2MB/s, zip when est > 30s
check("needsZip(60MB) is false", !P.needsZip(60_000_000));
check("needsZip(61MB) is true", P.needsZip(61_000_000));
check("needsZip(1MB) is false", !P.needsZip(1_000_000));
check("transferPlan(60MB).shouldZip is false", P.transferPlan(60_000_000).shouldZip === false);
check("transferPlan(61MB).shouldZip is true", P.transferPlan(61_000_000).shouldZip === true);
check("estimateSeconds(60MB) == 30", P.estimateSeconds(60_000_000) === 30);

// 8. ARQ session flow (mirrors arq_session_flow)
{
  const tx = P.TransferSession.tx(7, 4);
  check("tx starts with Ping", tx.nextFrame().kind === "Ping");
  tx.onPong();
  let chunksOk = true;
  for (let i = 0; i < 4; i++) {
    const f = tx.nextFrame();
    if (f.kind !== "Chunk" || f.chunkIdx !== i) chunksOk = false;
    tx.onAck(i);
  }
  check("tx emits every chunk in order", chunksOk);
  check("tx finishes with End", tx.nextFrame().kind === "End");
  check("tx idle after End", tx.nextFrame() === null);

  const rx = P.TransferSession.rx(7, 4);
  rx.onPing();
  check("rx answers Ping with Pong", rx.nextFrame().kind === "Pong");
  rx.onChunkOk(1);
  check("rx acks verified chunk", rx.nextFrame().kind === "Ack");
  rx.onChunkErr(2);
  check("rx naks failed chunk", rx.nextFrame().kind === "Nak");

  rx.markReceived(0);
  check("not all chunks received yet", !rx.allReceived());
  for (let i = 1; i < 4; i++) rx.markReceived(i);
  check("all chunks received", rx.allReceived());
}

// 9. NAK retransmit + bounded timeout retries
{
  const tx = P.TransferSession.tx(7, 4);
  tx.onPong();
  tx.nextFrame(); // chunk 0 -> AwaitingAck
  tx.onNak(0);
  const f = tx.nextFrame();
  check("nak triggers retransmit", f.kind === "Chunk" && f.chunkIdx === 0);

  check("timeout retry 1 retransmits", tx.onTimeout() !== null);
  check("timeout retry 2 retransmits", tx.onTimeout() !== null);
  check("timeout retry 3 retransmits", tx.onTimeout() !== null);
  check("timeout retry 4 -> Failed", tx.onTimeout() === null && tx.state === "Failed");
}

console.log(`\n${pass} passed, ${fail} failed`);
if (failures.length) {
  for (const name of failures) console.log(`  - ${name}`);
  process.exit(1);
}