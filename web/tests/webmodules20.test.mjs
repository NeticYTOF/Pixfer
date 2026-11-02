// Behavioral tests for the new web modules: link_matrix_v2,
// transfer_session_v2.
// Run: node tests/webmodules20.test.mjs (from web/)

import { LinkMatrixV2, MetricSample } from "../js/link_matrix_v2.js";
import { TransferSessionV2, RoleV2, SessionStateV2, SessionEventV2 } from "../js/transfer_session_v2.js";

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

// ---------- link_matrix_v2.js ----------
{
  const m = new LinkMatrixV2(8);
  check("matrix_v2: starts empty", m.linkCount() === 0 && m.windowSize() === 8 && m.latest(1, "throughput") === null);

  const m2 = new LinkMatrixV2(8);
  m2.record(0xabc, new MetricSample({ metric: "throughput", value: 5000 }));
  m2.record(0xabc, new MetricSample({ metric: "throughput", value: 6000 }));
  check("matrix_v2: record and latest", m2.latest(0xabc, "throughput") === 6000 && m2.linkCount() === 1);

  const m3 = new LinkMatrixV2(3);
  for (const v of [10, 20, 30, 40, 50]) {
    m3.record(1, new MetricSample({ metric: "x", value: v }));
  }
  check("matrix_v2: ring caps at window", Math.abs(m3.rollingMean(1, "x") - 40) < 1e-9 && m3.latest(1, "x") === 50);

  const m4 = new LinkMatrixV2(4);
  for (const v of [10, 20, 30]) {
    m4.record(2, new MetricSample({ metric: "jitter", value: v }));
  }
  check("matrix_v2: trend detects rise", m4.trend(2, "jitter") === 20);

  const m5 = new LinkMatrixV2(4);
  m5.record(3, new MetricSample({ metric: "latency", value: 100 }));
  check("matrix_v2: trend null with <2 samples", m5.trend(3, "latency") === null);

  const m6 = new LinkMatrixV2(4);
  m6.record(5, new MetricSample({ metric: "goodput", value: 3345 }));
  m6.dropLink(5);
  check("matrix_v2: drop link clears", m6.linkCount() === 0 && m6.latest(5, "goodput") === null);
}

// ---------- transfer_session_v2.js ----------
{
  const s = new TransferSessionV2(RoleV2.SENDER, 42);
  check("session_v2: starts idle", s.roleValue() === RoleV2.SENDER
    && s.sessionIdValue() === 42 && s.stateValue() === SessionStateV2.IDLE && s.eventLog().length === 0);

  const s2 = new TransferSessionV2(RoleV2.RECEIVER, 7);
  s2.beginHandshake(0);
  check("session_v2: handshake transitions", s2.stateValue() === SessionStateV2.HANDSHAKING && s2.eventLog().length === 1);
  s2.beginTransfer(100, 5);
  check("session_v2: transfer begin", s2.stateValue() === SessionStateV2.TRANSFERRING
    && s2.totalChunks() === 5 && s2.eventLog().length === 2);

  const s3 = new TransferSessionV2(RoleV2.SENDER, 1);
  s3.beginTransfer(0, 3);
  s3.markChunkDelivered(10);
  s3.markChunkDelivered(20);
  check("session_v2: partial delivery stays transferring", s3.deliveredCount() === 2 && s3.stateValue() === SessionStateV2.TRANSFERRING);
  s3.markChunkDelivered(30);
  check("session_v2: all delivered completes", s3.stateValue() === SessionStateV2.DONE
    && Math.abs(s3.progress() - 1.0) < 1e-9);

  const s4 = new TransferSessionV2(RoleV2.SENDER, 1);
  s4.beginTransfer(0, 2);
  for (let i = 0; i < 5; i++) s4.markChunkDelivered(0);
  check("session_v2: delivered clamps at total", s4.deliveredCount() === 2);

  const s5 = new TransferSessionV2(RoleV2.RECEIVER, 9);
  s5.beginHandshake(0);
  s5.fail(100, "peer unresponsive");
  check("session_v2: fail sets failed state", s5.stateValue() === SessionStateV2.FAILED
    && s5.eventLog()[s5.eventLog().length - 1].kind === "transfer_failed");

  const s6 = new TransferSessionV2(RoleV2.SENDER, 3);
  s6.beginTransfer(0, 4);
  s6.markChunkDelivered(1);
  s6.markChunkDelivered(2);
  check("session_v2: progress fraction", Math.abs(s6.progress() - 0.5) < 1e-9);

  const ev = new SessionEventV2({ tMs: 5, kind: "x", detail: "y" });
  check("session_v2: event shape", ev.tMs === 5 && ev.kind === "x" && ev.detail === "y");
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}