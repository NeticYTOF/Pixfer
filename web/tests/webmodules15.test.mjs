// Behavioral tests for the new web modules: link_matrix_ext,
// transfer_checkpoint, pipeline_ext.
// Run: node tests/webmodules15.test.mjs (from web/)

import { LinkMatrixExt, MetricCellExt } from "../js/link_matrix_ext.js";
import { TransferCheckpoint, newCheckpoint, CheckpointStore } from "../js/transfer_checkpoint.js";
import {
  PipelineExt,
  PipelineErrorExt,
  ValidateStageExt,
  DedupStageExt,
  CompressStageExt,
  EncodeStageExt,
  SendStageExt,
} from "../js/pipeline_ext.js";

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

// ---------- link_matrix_ext.js ----------
{
  const m = new LinkMatrixExt();
  check("matrix: starts empty", m.linkCount() === 0 && m.get(1, "throughput") === null);

  const m2 = new LinkMatrixExt();
  m2.set(0xabc, "throughput", 6772.7, 1234);
  m2.set(0xabc, "latency_ms", 16.7, 1234);
  const tp = m2.get(0xabc, "throughput");
  check("matrix: set and get", tp !== null && Math.abs(tp.value - 6772.7) < 1e-6 && tp.tMs === 1234 && m2.linkCount() === 1);

  const m3 = new LinkMatrixExt();
  m3.set(1, "goodput", 3345, 0);
  m3.set(2, "goodput", 6690, 0);
  check("matrix: multiple links", m3.linkCount() === 2
    && m3.metricsFor(1) !== null && m3.metricsFor(2) !== null);

  const m4 = new LinkMatrixExt();
  m4.set(7, "throughput", 6772.7, 100);
  m4.set(8, "throughput", 3345, 200);
  const json = m4.toJson();
  check("matrix: json export shape", json.startsWith("[") && json.endsWith("]")
    && json.includes("throughput") && json.includes('"7":') && json.includes('"8":'));

  const m5 = new LinkMatrixExt();
  m5.set(5, "jitter_ms", 1, 0);
  m5.set(5, "jitter_ms", 2, 1);
  const c = m5.get(5, "jitter_ms");
  check("matrix: overwrite same metric", c.value === 2 && c.tMs === 1);

  const cell = new MetricCellExt({ value: 1.5, tMs: 9 });
  check("matrix: cell shape", cell.value === 1.5 && cell.tMs === 9);
}

// ---------- transfer_checkpoint.js ----------
{
  const cp = newCheckpoint(42, 10);
  check("checkpoint: fresh starts empty", cp.sessionId === 42 && cp.totalChunks === 10
    && !cp.isComplete() && cp.pendingCount() === 10 && cp.version === 1);

  const cp2 = newCheckpoint(7, 3);
  cp2.delivered.add(0);
  cp2.delivered.add(1);
  check("checkpoint: incomplete until all delivered", !cp2.isComplete() && cp2.pendingCount() === 1);
  cp2.delivered.add(2);
  check("checkpoint: complete when all delivered", cp2.isComplete() && cp2.pendingCount() === 0);

  const store = new CheckpointStore();
  const cp3 = newCheckpoint(9, 5);
  cp3.delivered.add(0);
  store.save(cp3);
  check("checkpoint: store save/load", store.size() === 1
    && store.load(9).delivered.size === 1 && store.load(9).totalChunks === 5);

  const store2 = new CheckpointStore();
  store2.save(newCheckpoint(3, 4));
  store2.dropSession(3);
  check("checkpoint: drop session", store2.isEmpty() && store2.load(3) === null);

  const store3 = new CheckpointStore();
  store3.save(newCheckpoint(1, 8));
  store3.save(newCheckpoint(2, 16));
  check("checkpoint: multiple sessions", store3.size() === 2
    && store3.load(1).totalChunks === 8 && store3.load(2).totalChunks === 16);
}

// ---------- pipeline_ext.js ----------
{
  const p = new PipelineExt();
  const data = [1, 2, 3];
  const out = p.run(data);
  check("pipeline: empty pipeline passes through", JSON.stringify(out) === JSON.stringify(data) && p.stageCount() === 0);

  const p2 = new PipelineExt();
  p2.addStage(new ValidateStageExt(10));
  let threw = null;
  try {
    p2.run(new Array(100).fill(0));
  } catch (e) {
    threw = e;
  }
  check("pipeline: validate rejects oversized", threw instanceof PipelineErrorExt && threw.stage === "validate");

  const p3 = new PipelineExt();
  p3.addStage(new ValidateStageExt(223));
  p3.addStage(new DedupStageExt());
  p3.addStage(new CompressStageExt(223));
  p3.addStage(new EncodeStageExt());
  check("pipeline: full pipeline runs all stages", p3.stageCount() === 4);
  const out3 = p3.run(new Array(50).fill(9));
  check("pipeline: output non-empty", out3.length === 50);
  check("pipeline: per-stage timings recorded", p3.stageTimingsMs().length === 4);

  const p4 = new PipelineExt();
  p4.addStage(new SendStageExt((_bytes) => ({})));
  const out4 = p4.run([1, 2, 3]);
  check("pipeline: send stage invokes sender", JSON.stringify(out4) === JSON.stringify([1, 2, 3]));

  const p5 = new PipelineExt();
  p5.addStage(new SendStageExt(function (_bytes) { return { error: "link down" }; }));
  let threw2 = null;
  try {
    p5.run([1]);
  } catch (e) {
    threw2 = e;
  }
  check("pipeline: send failure propagates", threw2 instanceof PipelineErrorExt && threw2.stage === "send" && threw2.message === "link down");
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}