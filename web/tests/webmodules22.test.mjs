// Behavioral tests for the new web modules: link_monitor_ext,
// transfer_orchestrator_v2, protocol_ext.
// Run: node tests/webmodules22.test.mjs (from web/)

import { LinkMonitorExt, MonitorSample } from "../js/link_monitor_ext.js";
import { TransferOrchestratorV2, OrchestratorReport, PhaseResult, Phase } from "../js/transfer_orchestrator_v2.js";
import { ProtocolExt, ProtocolErrorExt } from "../js/protocol_ext.js";

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

// ---------- link_monitor_ext.js ----------
{
  const m = new LinkMonitorExt(8);
  check("mon_ext: empty monitor zero means", m.meanGoodput() === 0 && m.meanLatency() === 0 && m.size() === 0);

  const m2 = new LinkMonitorExt(4);
  m2.record(new MonitorSample({ goodputBps: 3345, latencyMs: 16.7 }));
  m2.record(new MonitorSample({ goodputBps: 6690, latencyMs: 16.7 }));
  check("mon_ext: mean tracks window", Math.abs(m2.meanGoodput() - (3345 + 6690) / 2) < 1e-6
    && Math.abs(m2.meanLatency() - 16.7) < 1e-6);

  const m3 = new LinkMonitorExt(3);
  for (let i = 0; i < 10; i++) m3.record(new MonitorSample({ goodputBps: 100, latencyMs: 10 }));
  check("mon_ext: window caps samples", m3.size() === 3);

  const m4 = new LinkMonitorExt(4);
  m4.record(new MonitorSample({ goodputBps: 6690, latencyMs: 0 }));
  check("mon_ext: health score high for good link", Math.abs(m4.healthScore() - 100) < 1e-6);

  const m5 = new LinkMonitorExt(4);
  m5.record(new MonitorSample({ goodputBps: 500, latencyMs: 50 }));
  const expected = (500 / 6690) * 100 * 0.6 + 50 * 0.4;
  check("mon_ext: health score low for degraded link", Math.abs(m5.healthScore() - expected) < 1e-6, `got ${m5.healthScore()}`);

  const m6 = new LinkMonitorExt(4);
  m6.record(new MonitorSample({ goodputBps: 9999, latencyMs: 0 }));
  check("mon_ext: health score clamped high", m6.healthScore() === 100);
  const m7 = new LinkMonitorExt(4);
  m7.record(new MonitorSample({ goodputBps: 0, latencyMs: 500 }));
  check("mon_ext: health score clamped low", m7.healthScore() === 0);
}

// ---------- transfer_orchestrator_v2.js ----------
{
  const o = new TransferOrchestratorV2(223, 500);
  check("orch_v2: chunk bytes preserved", o.chunkBytesValue() === 223);

  const data = new Array(30).fill(1);
  let sends = 0;
  const report = o.run(data, (frame) => {
    sends += 1;
    return {};
  });
  check("orch_v2: runs all phases", report.allOk() && report instanceof OrchestratorReport);
  const n = Math.ceil(30 / 223);
  check("orch_v2: phase count", report.phaseCount() === 4 + 2 * n, `got ${report.phaseCount()}, expected ${4 + 2 * n}`);
  check("orch_v2: total elapsed sums phases", report.totalElapsedMs() === report.phases.reduce((a, p) => a + p.elapsedMs, 0));

  const report2 = o.run(new Array(30).fill(1), () => ({ error: "link down" }));
  check("orch_v2: send failure stops and reports", !report2.allOk()
    && report2.phases[report2.phases.length - 1].phase === Phase.SEND
    && report2.phases[report2.phases.length - 1].detail.includes("link down"));

  const big = new Array(2000).fill(9);
  const report3 = o.run(big, () => ({}));
  const compress = report3.phases.find((p) => p.phase === Phase.COMPRESS);
  check("orch_v2: zips large payload", compress !== undefined && compress.detail.includes("zipped"));

  const pr = new PhaseResult({ phase: Phase.VERIFY, ok: true, detail: "x", elapsedMs: 5 });
  check("orch_v2: phase result shape", pr.phase === Phase.VERIFY && pr.ok === true && pr.elapsedMs === 5);
}

// ---------- protocol_ext.js ----------
{
  const p = new ProtocolExt(42);
  check("proto_ext: session id preserved", p.sessionIdValue() === 42);

  const blocks = p.encodeControl(0, []);
  check("proto_ext: control frame renders", blocks.length > 0);
  const frame = p.decode(blocks);
  check("proto_ext: control frame roundtrip", frame.kind === "Ping" && frame.sessionId === 42);

  const payload = new Array(30).fill(5);
  const cblocks = p.encodeChunk(2, 10, payload);
  const cframe = p.decode(cblocks);
  check("proto_ext: chunk frame roundtrip", cframe.kind === "Chunk" && cframe.chunkIdx === 2 && cframe.totalChunks === 10);
  const decoded = p.decodePayload(cblocks);
  check("proto_ext: payload roundtrip", JSON.stringify(decoded) === JSON.stringify(payload));

  const ackBlocks = p.encodeControl(3, []);
  let err = null;
  let threw = false;
  try {
    p.decodePayload(ackBlocks);
  } catch (e) {
    err = e;
    threw = true;
  }
  check("proto_ext: decode payload rejects non-chunk", threw && err instanceof ProtocolErrorExt && err.stage === "decode_payload");

  let err2 = null;
  try {
    p.encodeChunk(0, 1, new Array(300).fill(1));
  } catch (e) {
    err2 = e;
  }
  check("proto_ext: oversize chunk errors", err2 instanceof ProtocolErrorExt && err2.stage === "encode");
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}