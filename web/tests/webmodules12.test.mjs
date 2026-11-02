// Behavioral tests for the new web modules: transfer_retry_policy,
// link_calibration_store, frame_scheduler.
// Run: node tests/webmodules12.test.mjs (from web/)

import { RetryPolicy, RetryPolicyKind } from "../js/transfer_retry_policy.js";
import { CalibrationStore, CalibrationValues } from "../js/link_calibration_store.js";
import { FrameScheduler } from "../js/frame_scheduler.js";

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

// ---------- transfer_retry_policy.js ----------
{
  const p = RetryPolicy.fixed(250);
  check("retry: fixed delay constant", p.delay(1) === 250 && p.delay(5) === 250);

  const lin = RetryPolicy.linear(100, 400);
  check("retry: linear grows", lin.delay(1) === 100);
  check("retry: linear caps", lin.delay(10) === 400);
  check("retry: total_wait sums schedule", lin.totalWait(4) === 1000, `got ${lin.totalWait(4)}`);
  check("retry: zero retries zero wait", p.totalWait(0) === 0);

  const exp = RetryPolicy.exponential(100, 2.0, 400);
  check("retry: exponential grows", exp.delay(1) === 100 && exp.delay(2) === 200);
  check("retry: exponential caps", exp.delay(3) === 400 && exp.delay(10) === 400);

  const jit = RetryPolicy.fixed(100);
  jit.jitterMs = 10;
  check("retry: jitter deterministic", jit.delay(1) === 101 && jit.delay(2) === 102);

  check("retry: kind constants frozen", Object.isFrozen(RetryPolicyKind));
  check("retry: kind values", RetryPolicyKind.FIXED === "fixed"
    && RetryPolicyKind.LINEAR === "linear"
    && RetryPolicyKind.EXPONENTIAL === "exponential");
}

// ---------- link_calibration_store.js ----------
{
  const s = new CalibrationStore();
  check("calib: starts empty", s.isEmpty() && s.size() === 0);

  const v = new CalibrationValues();
  s.put(0xABCDEF, v);
  check("calib: put/get roundtrip", s.get(0xABCDEF) === v && s.contains(0xABCDEF));
  check("calib: default values", v.targetKelvin === 5500 && v.exposure === 128 && JSON.stringify(v.gains) === "[1,1,1]");

  check("calib: absent returns null", s.get(42) === null && !s.contains(42));

  const s2 = new CalibrationStore();
  s2.put(1, new CalibrationValues());
  s2.put(2, new CalibrationValues());
  check("calib: size tracks inserts", s2.size() === 2 && !s2.isEmpty());

  const s3 = new CalibrationStore();
  s3.put(7, new CalibrationValues());
  const v2 = new CalibrationValues({ targetKelvin: 6500 });
  s3.put(7, v2);
  check("calib: overwrite keeps size", s3.size() === 1 && s3.get(7).targetKelvin === 6500);
}

// ---------- frame_scheduler.js ----------
{
  const s = new FrameScheduler(30);
  check("scheduler: empty next is null", s.next() === null && s.popNext() === null);
  s.enqueue(0);
  s.enqueue(1);
  s.enqueue(2);
  check("scheduler: pending count", s.pendingCount() === 3);
  check("scheduler: next peeks head", s.next() === 0 && s.pendingCount() === 3);
  check("scheduler: pop_next in FIFO order", s.popNext() === 0 && s.popNext() === 1 && s.popNext() === 2 && s.popNext() === null);
  check("scheduler: queue drained", s.pendingCount() === 0);

  const s60 = new FrameScheduler(60);
  check("scheduler: dwell scales with fps",
    Math.abs(s60.minDwellMs() - 2 * (1000 / 60)) < 1e-9, `got ${s60.minDwellMs()}`);
  check("scheduler: target fps preserved", s60.targetFps() === 60);

  const s30 = new FrameScheduler(30);
  for (let i = 0; i < 100; i++) s30.enqueue(i);
  let orderOk = true;
  for (let i = 0; i < 100; i++) {
    if (s30.popNext() !== i) { orderOk = false; break; }
  }
  check("scheduler: 100-frame FIFO order", orderOk && s30.pendingCount() === 0);

  const sd = new FrameScheduler(30);
  sd.enqueue(5);
  sd.enqueue(5);
  check("scheduler: duplicates keep FIFO order", sd.popNext() === 5 && sd.popNext() === 5 && sd.popNext() === null);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}