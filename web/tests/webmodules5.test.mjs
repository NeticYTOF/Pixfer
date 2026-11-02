// Behavioral tests for the new web modules: transfer_health,
// chunk_prioritizer, sync_recovery.
// Run: node tests/webmodules5.test.mjs (from web/)

import { TransferHealth, HealthTier } from "../js/transfer_health.js";
import { ChunkPrioritizer } from "../js/chunk_prioritizer.js";
import { SyncRecovery, RecoveryState } from "../js/sync_recovery.js";

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

// ---------- transfer_health.js ----------
{
  const h = new TransferHealth(0.3);
  check("health: unknown before samples", h.tier(1.0) === HealthTier.UNKNOWN);

  const h2 = new TransferHealth(1.0);
  h2.observe({ throughputMbps: 2.0, retries: 0, fecFailed: 0 });
  check("health: healthy at 2x floor", h2.tier(1.0) === HealthTier.HEALTHY);

  const h3 = new TransferHealth(1.0);
  h3.observe({ throughputMbps: 0.7, retries: 0, fecFailed: 0 });
  check("health: degraded at 70% floor", h3.tier(1.0) === HealthTier.DEGRADED);

  const h4 = new TransferHealth(1.0);
  h4.observe({ throughputMbps: 0.4, retries: 0, fecFailed: 0 });
  check("health: unhealthy at 40% floor", h4.tier(1.0) === HealthTier.UNHEALTHY);

  const h5 = new TransferHealth(1.0);
  h5.observe({ throughputMbps: 2.0, retries: 0, fecFailed: 20 });
  check("health: heavy fec failures -> unhealthy", h5.tier(1.0) === HealthTier.UNHEALTHY);

  const h6 = new TransferHealth(0.1);
  h6.observe({ throughputMbps: 1.0, retries: 0, fecFailed: 0 });
  const after1 = h6.emaTp;
  h6.observe({ throughputMbps: 5.0, retries: 0, fecFailed: 0 });
  check("health: ema moves toward new value", h6.emaTp > after1);
}

// ---------- chunk_prioritizer.js ----------
{
  const q = new ChunkPrioritizer();
  check("prioritizer: empty pop is null", q.pop() === null);
  check("prioritizer: empty maxPriority 0", q.maxPriority() === 0);

  const q2 = new ChunkPrioritizer();
  q2.push({ chunkIdx: 0, priority: 1, queuedMs: 0 });
  q2.push({ chunkIdx: 1, priority: 5, queuedMs: 100 });
  q2.push({ chunkIdx: 2, priority: 3, queuedMs: 200 });
  check("prioritizer: highest priority pops first", q2.pop().chunkIdx === 1);
  check("prioritizer: maxPriority after pop", q2.maxPriority() === 3);

  const q3 = new ChunkPrioritizer();
  q3.push({ chunkIdx: 0, priority: 2, queuedMs: 1000 });
  q3.push({ chunkIdx: 1, priority: 2, queuedMs: 2000 });
  q3.push({ chunkIdx: 2, priority: 2, queuedMs: 3000 });
  check("prioritizer: FIFO within same priority",
    q3.pop().chunkIdx === 0 && q3.pop().chunkIdx === 1 && q3.pop().chunkIdx === 2);

  const q4 = new ChunkPrioritizer();
  q4.push({ chunkIdx: 10, priority: 1, queuedMs: 100 });
  q4.push({ chunkIdx: 11, priority: 9, queuedMs: 200 });
  q4.push({ chunkIdx: 12, priority: 9, queuedMs: 300 });
  q4.push({ chunkIdx: 13, priority: 1, queuedMs: 400 });
  const order = [q4.pop().chunkIdx, q4.pop().chunkIdx, q4.pop().chunkIdx, q4.pop().chunkIdx];
  check("prioritizer: mixed priority order",
    JSON.stringify(order) === JSON.stringify([11, 12, 10, 13]),
    `got ${JSON.stringify(order)}`);
}

// ---------- sync_recovery.js ----------
{
  const pattern = [0, 1, 2, 3, 4, 5, 6, 7];
  const r = new SyncRecovery(pattern, 5);
  check("sync: starts acquiring", r.state === RecoveryState.ACQUIRING);
  check("sync: maxScans", r.maxScans === 5, `got ${r.maxScans}`);

  const r2 = new SyncRecovery(pattern, 5);
  check("sync: match locks", r2.scan([0, 1, 2, 3, 4, 5, 6, 7]) === true);
  check("sync: state locked after match", r2.state === RecoveryState.LOCKED);
  check("sync: scan count reset", r2.scanCount() === 0);

  const r3 = new SyncRecovery(pattern, 3);
  const wrong = [1, 2, 3, 4, 5, 6, 7, 0];
  check("sync: no match increments scans", r3.scan(wrong) === false && r3.scanCount() === 1);

  const r4 = new SyncRecovery(pattern, 2);
  r4.scan(wrong);
  r4.scan(wrong);
  check("sync: exceeds maxScans -> failed", r4.state === RecoveryState.FAILED);

  const r5 = new SyncRecovery(pattern, 2);
  r5.scan([0, 1, 2, 3, 4, 5, 6, 7]);
  check("sync: locked ignores budget", r5.scan([0, 1, 2, 3, 4, 5, 6, 7]) === true);
  check("sync: still locked", r5.state === RecoveryState.LOCKED);

  const r6 = new SyncRecovery(pattern, 1);
  r6.scan(wrong); // fails
  check("sync: failed until reset", r6.scan([0, 1, 2, 3, 4, 5, 6, 7]) === false);
  r6.reset();
  check("sync: reset + match re-locks", r6.scan([0, 1, 2, 3, 4, 5, 6, 7]) === true && r6.state === RecoveryState.LOCKED);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}