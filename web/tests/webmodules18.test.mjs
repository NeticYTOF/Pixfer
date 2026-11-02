// Behavioral tests for the new web modules: transfer_stats_ext,
// link_budget_v2, transfer_watchdog_ext.
// Run: node tests/webmodules18.test.mjs (from web/)

import { TransferStatsExt } from "../js/transfer_stats_ext.js";
import { LinkBudgetV2, FlowRequestV2 } from "../js/link_budget_v2.js";
import { TransferWatchdogExt, WatchdogState } from "../js/transfer_watchdog_ext.js";

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

function sample() {
  const s = new TransferStatsExt();
  s.bytes = 892;
  s.elapsedMs = 132;
  s.retries = 3;
  s.fecCorrections = 1;
  s.failedFrames = 0;
  s.deliveredChunks = 4;
  s.totalChunks = 4;
  return s;
}

// ---------- transfer_stats_ext.js ----------
{
  const s = sample();
  check("stats: throughput math", Math.abs(s.throughputBps() - 892 / 0.132) < 1e-3, `got ${s.throughputBps()}`);

  const s0 = new TransferStatsExt();
  s0.bytes = 100;
  check("stats: zero elapsed zero throughput", s0.throughputBps() === 0);

  check("stats: delivered fraction full", Math.abs(s.deliveredFraction() - 1.0) < 1e-9);
  check("stats: retry rate math", Math.abs(s.retryRate() - 0.75) < 1e-9);
  check("stats: success when all delivered", s.isSuccess());

  const s2 = sample();
  s2.failedFrames = 2;
  check("stats: not success with failures", !s2.isSuccess());

  const j = s.toJson();
  check("stats: json export shape", j.includes('"bytes":892') && j.includes('"total_chunks":4'));
}

// ---------- link_budget_v2.js ----------
{
  const b = new LinkBudgetV2(100);
  const a = b.allocate([new FlowRequestV2({ flowId: 1, weight: 1 }), new FlowRequestV2({ flowId: 2, weight: 1 })]);
  check("budget: equal split", Math.abs(a[0].amount - 50) < 1e-9 && Math.abs(a[1].amount - 50) < 1e-9);

  const b2 = new LinkBudgetV2(100);
  const a2 = b2.allocate([new FlowRequestV2({ flowId: 1, weight: 3 }), new FlowRequestV2({ flowId: 2, weight: 1 })]);
  check("budget: weighted 3:1 split", Math.abs(a2[0].amount - 75) < 1e-9 && Math.abs(a2[1].amount - 25) < 1e-9);

  const b3 = new LinkBudgetV2(100);
  const a3 = b3.allocate([
    new FlowRequestV2({ flowId: 1, weight: 1, minimum: 20 }),
    new FlowRequestV2({ flowId: 2, weight: 1, minimum: 30 }),
  ]);
  check("budget: minimums reserved first", Math.abs(a3[0].amount - 45) < 1e-9 && Math.abs(a3[1].amount - 55) < 1e-9
    && a3[0].satisfiedMinimum && a3[1].satisfiedMinimum);

  const b4 = new LinkBudgetV2(50);
  const a4 = b4.allocate([
    new FlowRequestV2({ flowId: 1, weight: 1, minimum: 30 }),
    new FlowRequestV2({ flowId: 2, weight: 1, minimum: 40 }),
  ]);
  check("budget: below minimums scales down", Math.abs(a4[0].amount - (30 * 50 / 70)) < 1e-9
    && Math.abs(a4[1].amount - (40 * 50 / 70)) < 1e-9 && !a4[0].satisfiedMinimum);

  const b5 = new LinkBudgetV2(100);
  const a5 = b5.allocate([new FlowRequestV2({ flowId: 1, weight: 0, minimum: 10 }), new FlowRequestV2({ flowId: 2, weight: 5 })]);
  check("budget: zero weight gets only minimum", Math.abs(a5[0].amount - 10) < 1e-9 && Math.abs(a5[1].amount - 90) < 1e-9);
}

// ---------- transfer_watchdog_ext.js ----------
{
  const w = new TransferWatchdogExt(1000, 500);
  check("watchdog: effective timeout includes grace", w.effectiveTimeoutMs() === 1500);

  const w0 = new TransferWatchdogExt(1000, 0);
  check("watchdog: starts active not stalled", w0.stateValue() === WatchdogState.ACTIVE && !w0.isStalled() && w0.fireCount() === 0);

  const w1 = new TransferWatchdogExt(1000, 0);
  w1.noteStall();
  check("watchdog: stall flags stalled", w1.isStalled());
  w1.noteProgress();
  check("watchdog: progress resets stall", !w1.isStalled() && w1.stateValue() === WatchdogState.ACTIVE);

  const w2 = new TransferWatchdogExt(1000, 0);
  w2.noteStall();
  w2.noteStall();
  check("watchdog: stall increments fire count", w2.fireCount() === 2 && w2.stateValue() === WatchdogState.TIMED_OUT);

  const w3 = new TransferWatchdogExt(1000, 100);
  w3.noteStall();
  w3.reset();
  check("watchdog: reset clears state", w3.fireCount() === 0 && !w3.isStalled()
    && w3.stateValue() === WatchdogState.ACTIVE && w3.effectiveTimeoutMs() === 1100);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}