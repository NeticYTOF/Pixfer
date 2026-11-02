// Behavioral tests for the new web modules: exposure_controller,
// frame_codec, transfer_rollback.
// Run: node tests/webmodules6.test.mjs (from web/)

import { ExposureController, lockedExposure } from "../js/exposure_controller.js";
import { encodePayload, decodePayload, BLOCKS_PER_FRAME } from "../js/frame_codec.js";
import { TransferRollback } from "../js/transfer_rollback.js";

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

// ---------- exposure_controller.js ----------
{
  const c = new ExposureController(0.5);
  const r = lockedExposure(128);
  check("exposure: locked passthrough", c.nextExposure(r) === 128);

  const c2 = new ExposureController(0.5);
  const r2 = { value: 64, locked: false, avgLuma: 0.2 };
  const next2 = c2.nextExposure(r2);
  check("exposure: luma below target raises", next2 > 64, `got ${next2}`);

  const c3 = new ExposureController(0.5);
  const r3 = { value: 200, locked: false, avgLuma: 0.8 };
  const next3 = c3.nextExposure(r3);
  check("exposure: luma above target lowers", next3 < 200, `got ${next3}`);

  const c4 = new ExposureController(0.3);
  check("exposure: targetLuma preserved", c4.targetLuma === 0.3);

  const c5 = new ExposureController(0.5);
  c5.nextExposure({ value: 100, locked: false, avgLuma: 0.2 });
  c5.reset();
  const next5 = c5.nextExposure({ value: 100, locked: false, avgLuma: 0.2 });
  check("exposure: reset clears integral", next5 > 100, `got ${next5}`);

  const c6 = new ExposureController(0.9);
  const n6 = c6.nextExposure({ value: 250, locked: false, avgLuma: 0.1 });
  check("exposure: output in range", n6 >= 0 && n6 <= 255, `got ${n6}`);
}

// ---------- frame_codec.js ----------
{
  const payload = [1, 2, 3, 4, 5];
  const blocks = encodePayload(payload);
  const back = decodePayload(blocks);
  check("codec: payload roundtrip small", JSON.stringify(back) === JSON.stringify(payload),
    `got ${JSON.stringify(back)}`);

  const full = new Array(223).fill(0xAB);
  const blocksFull = encodePayload(full);
  const backFull = decodePayload(blocksFull);
  check("codec: payload roundtrip full", JSON.stringify(backFull) === JSON.stringify(full));

  check("codec: blocks per frame", BLOCKS_PER_FRAME === 270 * 480);

  let threw = false;
  try {
    encodePayload(new Array(224).fill(1));
  } catch (e) {
    threw = true;
  }
  check("codec: too-large payload throws", threw);

  const emptyBlocks = encodePayload([]);
  check("codec: empty payload", emptyBlocks.length === 0);
  const emptyBack = decodePayload(emptyBlocks);
  check("codec: empty roundtrip", emptyBack.length === 0);

  const triple = [0x11, 0x22, 0x33, 0x44, 0x55];
  const blocksT = encodePayload(triple);
  const backT = decodePayload(blocksT);
  check("codec: partial triple roundtrip", JSON.stringify(backT) === JSON.stringify(triple),
    `got ${JSON.stringify(backT)}`);
}

// ---------- transfer_rollback.js ----------
{
  const rb = new TransferRollback();
  check("rollback: not active initially", !rb.isActive());
  check("rollback: rollback without begin", rb.rollback() === null);

  const fakeReasm = {
    receivedIndices: () => [0, 1, 2],
    totalChunks: () => 5,
    sessionId: () => 7,
    isComplete: () => false,
  };
  const rb2 = new TransferRollback();
  rb2.begin(fakeReasm);
  check("rollback: active after begin", rb2.isActive());
  const restored = rb2.restored();
  check("rollback: restored state matches snapshot",
    JSON.stringify(restored.received) === JSON.stringify([0, 1, 2]) && restored.sessionId === 7);
  const rbResult = rb2.rollback();
  check("rollback: returns restored", rbResult !== null);
  check("rollback: not active after rollback", !rb2.isActive());

  const rb3 = new TransferRollback();
  const fakeReasm2 = {
    receivedIndices: () => [0, 1],
    totalChunks: () => 4,
    sessionId: () => 7,
    isComplete: () => false,
  };
  rb3.begin(fakeReasm2);
  rb3.commit();
  check("rollback: commit clears active", !rb3.isActive());
  check("rollback: rollback after commit", rb3.rollback() === null);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}