// Behavioral tests for the new web modules: frame_queue_ext,
// link_jitter_meter.
// Run: node tests/webmodules16.test.mjs (from web/)

import { FrameQueueExt, QueuedFrame } from "../js/frame_queue_ext.js";
import { LinkJitterMeter } from "../js/link_jitter_meter.js";

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

function frame(seq, prio) {
  return new QueuedFrame({ seq, data: [seq], priority: prio });
}

// ---------- frame_queue_ext.js ----------
{
  const q = new FrameQueueExt(4);
  check("queue: starts empty at capacity", q.isEmpty() && q.capacityValue() === 4 && q.available() === 4);

  const q2 = new FrameQueueExt(2);
  check("queue: push until full", q2.push(frame(0, 1)) === true
    && q2.push(frame(1, 1)) === true && q2.push(frame(2, 1)) === false
    && q2.size() === 2 && q2.available() === 0);

  const q3 = new FrameQueueExt(8);
  q3.push(frame(0, 1));
  q3.push(frame(1, 3));
  q3.push(frame(2, 2));
  const f = q3.pop();
  check("queue: highest priority pops first", f !== null && f.seq === 1 && f.priority === 3);
  const f2 = q3.pop();
  check("queue: then next priority", f2 !== null && f2.seq === 2 && f2.priority === 2);
  const f3 = q3.pop();
  check("queue: then lowest", f3 !== null && f3.seq === 0 && f3.priority === 1);
  check("queue: empty after draining", q3.pop() === null);

  const q4 = new FrameQueueExt(8);
  q4.push(frame(5, 2));
  q4.push(frame(6, 2));
  q4.push(frame(7, 2));
  check("queue: FIFO within same priority", q4.pop().seq === 5 && q4.pop().seq === 6 && q4.pop().seq === 7 && q4.pop() === null);

  const q5 = new FrameQueueExt(4);
  q5.push(frame(0, 1));
  q5.push(frame(1, 5));
  const p = q5.peek();
  check("queue: peek does not remove", p !== null && p.seq === 1 && q5.size() === 2);
  check("queue: pop after peek", q5.pop().seq === 1 && q5.pop().seq === 0);

  const q6 = new FrameQueueExt(4);
  q6.push(frame(0, 1));
  q6.push(frame(1, 1));
  q6.clear();
  check("queue: clear drops all", q6.isEmpty() && q6.available() === 4);
}

// ---------- link_jitter_meter.js ----------
{
  const m = new LinkJitterMeter(60);
  check("jitter: fps preserved", m.fpsValue() === 60);

  const m2 = new LinkJitterMeter(30);
  for (let i = 0; i < 10; i++) m2.observe(33.3);
  check("jitter: constant intervals zero jitter", m2.jitterMs() < 1e-6, `got ${m2.jitterMs()}`);

  const m3 = new LinkJitterMeter(30);
  m3.observe(33.3);
  m3.observe(33.3);
  m3.observe(60.0);
  check("jitter: varying intervals raise jitter", m3.jitterMs() > 0);

  const m4 = new LinkJitterMeter(60);
  for (let i = 0; i < 5; i++) m4.observe(16.7);
  check("jitter: stable within bound", m4.isStable(1.0));
  m4.observe(50.0);
  check("jitter: unstable beyond bound", !m4.isStable(0.5));

  const m5 = new LinkJitterMeter(60);
  check("jitter: frame period derived", Math.abs(m5.framePeriodMs() - 16.6666) < 0.01, `got ${m5.framePeriodMs()}`);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}