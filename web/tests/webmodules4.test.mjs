// Behavioral tests for the new web modules: link_scorer,
// bandwidth_allocator, frame_pacer.
// Run: node tests/webmodules4.test.mjs (from web/)

import * as S from "../js/link_scorer.js";
import * as B from "../js/bandwidth_allocator.js";
import { FramePacer } from "../js/frame_pacer.js";

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

// ---------- link_scorer.js ----------
{
  // Perfect link: no failures, no retries, no jitter, 2x floor throughput.
  const perfect = S.linkScore({
    fecCorrected: 0,
    fecFailed: 0,
    retries: 0,
    samples: 10,
    jitterMs: 0,
    throughputMbps: 2.0,
    floorMbps: 1.0,
  });
  check("scorer: perfect link = 100", perfect === 100, `got ${perfect}`);

  // Heavy fec failures (90% failed) should drop the score a lot.
  const badFec = S.linkScore({
    fecCorrected: 10,
    fecFailed: 90,
    retries: 0,
    samples: 100,
    jitterMs: 0,
    throughputMbps: 2.0,
    floorMbps: 1.0,
  });
  check("scorer: heavy fec failures lower score", badFec < 70, `got ${badFec}`);

  // 5 retries/sample -> retry subscore 0.
  const heavyRetries = S.linkScore({
    fecCorrected: 0,
    fecFailed: 0,
    retries: 50,
    samples: 10,
    jitterMs: 0,
    throughputMbps: 2.0,
    floorMbps: 1.0,
  });
  check("scorer: heavy retries lower score", heavyRetries < 80, `got ${heavyRetries}`);

  // 200 ms jitter -> jitter subscore 0.
  const highJitter = S.linkScore({
    fecCorrected: 0,
    fecFailed: 0,
    retries: 0,
    samples: 10,
    jitterMs: 200,
    throughputMbps: 2.0,
    floorMbps: 1.0,
  });
  check("scorer: high jitter lower score", highJitter < 90, `got ${highJitter}`);

  // 0.1 MB/s vs 1.0 floor -> tp subscore 0.
  const lowTp = S.linkScore({
    fecCorrected: 0,
    fecFailed: 0,
    retries: 0,
    samples: 10,
    jitterMs: 0,
    throughputMbps: 0.1,
    floorMbps: 1.0,
  });
  check("scorer: low throughput lower score", lowTp < 85, `got ${lowTp}`);

  // No samples -> 0.
  const none = S.linkScore({ samples: 0 });
  check("scorer: no samples -> 0", none === 0, `got ${none}`);
}

// ---------- bandwidth_allocator.js ----------
{
  const reqs = [
    { jobId: 1, weight: 1 },
    { jobId: 2, weight: 3 },
  ];
  const alloc = B.allocate(10_000_000, reqs);
  check("allocator: proportional 1:3", Math.abs(alloc[0].allocatedBps - 2_500_000) < 1e-6 && Math.abs(alloc[1].allocatedBps - 7_500_000) < 1e-6,
    `got ${alloc[0].allocatedBps}, ${alloc[1].allocatedBps}`);

  const eq = B.allocate(8_000_000, [
    { jobId: 1, weight: 1 },
    { jobId: 2, weight: 1 },
  ]);
  check("allocator: equal split", Math.abs(eq[0].allocatedBps - 4_000_000) < 1e-6, `got ${eq[0].allocatedBps}`);

  const zero = B.allocate(8_000_000, [
    { jobId: 1, weight: 0 },
    { jobId: 2, weight: 0 },
  ]);
  check("allocator: zero weights -> zero", zero[0].allocatedBps === 0 && zero[1].allocatedBps === 0);

  const total = B.totalAllocated(1_000_000, [
    { jobId: 1, weight: 1 },
    { jobId: 2, weight: 3 },
  ]);
  check("allocator: total allocated = capacity", Math.abs(total - 1_000_000) < 1e-6, `got ${total}`);

  const budgetAlloc = B.allocateFromBudget({ capacity: 4_000_000 }, [
    { jobId: 1, weight: 1 },
    { jobId: 2, weight: 3 },
  ]);
  check("allocator: from budget capacity", Math.abs(budgetAlloc[0].allocatedBps - 1_000_000) < 1e-6, `got ${budgetAlloc[0].allocatedBps}`);
}

// ---------- frame_pacer.js ----------
{
  const p30 = new FramePacer(30);
  check("pacer: 30fps dwell", Math.abs(p30.minDwellMs() - 2 * (1000 / 30)) < 1e-6, `got ${p30.minDwellMs()}`);

  const p60 = new FramePacer(60);
  check("pacer: 60fps dwell", Math.abs(p60.minDwellMs() - 2 * (1000 / 60)) < 1e-6, `got ${p60.minDwellMs()}`);

  const p = new FramePacer(30);
  const t10 = p.transferTimeS(10);
  check("pacer: 10 chunks @30fps = 1.0s", Math.abs(t10 - 1.0) < 1e-6, `got ${t10}`);

  const m = p.minDwellMs(10);
  check("pacer: margin adds to dwell", Math.abs(m - (2 * (1000 / 30) + 10)) < 1e-6, `got ${m}`);

  const p2 = new FramePacer(30);
  p2.observeInterval(33.3);
  check("pacer: single interval jitter 0", p2.jitterMs === 0, `got ${p2.jitterMs}`);
  p2.observeInterval(33.3);
  check("pacer: constant intervals jitter 0", p2.jitterMs === 0, `got ${p2.jitterMs}`);
  p2.observeInterval(60.0);
  check("pacer: varying intervals raise jitter", p2.jitterMs > 0, `got ${p2.jitterMs}`);

  const p3 = new FramePacer(60);
  for (let i = 0; i < 100; i++) p3.observeInterval(16.7);
  check("pacer: bounded window keeps jitter ~0", p3.jitterMs < 1e-9, `got ${p3.jitterMs}`);

  const p4 = new FramePacer(60);
  check("pacer: targetFps preserved", p4.targetFps === 60, `got ${p4.targetFps}`);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}