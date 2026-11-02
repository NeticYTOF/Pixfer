// Behavioral tests for the new web modules: color_space,
// transfer_progress_ext, link_quality_ext, transfer_orchestrator_ext.
// Run: node tests/webmodules19.test.mjs (from web/)

import { ColorSpace } from "../js/color_space.js";
import { TransferProgressExt, ProgressSnapshotExt } from "../js/transfer_progress_ext.js";
import { qualityScoreExt, QualityInputsExt } from "../js/link_quality_ext.js";
import { TransferOrchestratorExt, ChunkExt } from "../js/transfer_orchestrator_ext.js";

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

// ---------- color_space.js ----------
{
  const cs = new ColorSpace();
  // All 8 palette colors must be pairwise distinct.
  const pal = [0, 1, 2, 3, 4, 5, 6, 7].map((v) => cs.colorForValue(v));
  let distinct = true;
  for (let i = 0; i < pal.length; i++) {
    for (let j = i + 1; j < pal.length; j++) {
      if (!cs.isDistinct(pal[i], pal[j])) distinct = false;
    }
  }
  check("color: 8 distinct palette colors", distinct);

  for (let v = 0; v < 8; v++) {
    const c = cs.colorForValue(v);
    check(`color: value roundtrip ${v}`, cs.valueForColor(c) === v);
  }

  check("color: (128,0,0) nearest is red(2)", cs.nearestValue([128, 0, 0]) === 2);
  check("color: (128,128,128) is gray(7)", cs.nearestValue([128, 128, 128]) === 7);
  check("color: distance zero for same color", cs.distance2([255, 0, 255], [255, 0, 255]) === 0);
  check("color: red vs white distance", cs.distance2([255, 0, 0], [255, 255, 255]) === 255 * 255 * 2);
}

// ---------- transfer_progress_ext.js ----------
{
  const p0 = new TransferProgressExt(10);
  check("progress: starts at zero", p0.deliveredCount() === 0 && p0.totalValue() === 10 && !p0.isComplete() && p0.fraction() === 0);

  const p1 = new TransferProgressExt(3);
  p1.markDelivered();
  p1.markDelivered();
  p1.markDelivered();
  check("progress: complete when all delivered", p1.deliveredCount() === 3 && p1.isComplete() && Math.abs(p1.fraction() - 1.0) < 1e-9);

  const p2 = new TransferProgressExt(2);
  p2.markDelivered();
  p2.markDelivered();
  p2.markDelivered();
  check("progress: clamps at total", p2.deliveredCount() === 2);

  const p3 = new TransferProgressExt(5);
  p3.markDelivered();
  p3.setElapsedMs(500);
  const snap = p3.snapshot();
  check("progress: snapshot shape", snap instanceof ProgressSnapshotExt && snap.delivered === 1 && snap.total === 5 && snap.elapsedMs === 500);

  const p4 = new TransferProgressExt(10);
  for (let i = 0; i < 5; i++) p4.markDelivered();
  p4.setElapsedMs(1000);
  check("progress: ETA estimation", p4.etaMs() === 1000, `got ${p4.etaMs()}`);

  const p5 = new TransferProgressExt(10);
  p5.setElapsedMs(500);
  check("progress: ETA zero when no progress", p5.etaMs() === 0);
}

// ---------- link_quality_ext.js ----------
{
  const perfect = new QualityInputsExt({ throughputBps: 10000, capacityBps: 10000, jitterMs: 0, fecCorrections: 0, frames: 100 });
  check("quality: perfect link scores 100", Math.abs(qualityScoreExt(perfect) - 100.0) < 1e-9);

  const degraded = new QualityInputsExt({ throughputBps: 5000, capacityBps: 10000, jitterMs: 10, fecCorrections: 2, frames: 100 });
  const expected = 0.5 * 40 + 0.9 * 30 + 0.98 * 30;
  check("quality: degraded link scores lower", Math.abs(qualityScoreExt(degraded) - expected) < 1e-6, `got ${qualityScoreExt(degraded)}`);

  const extreme = new QualityInputsExt({ throughputBps: 20000, capacityBps: 10000, jitterMs: 200, fecCorrections: 150, frames: 100 });
  const s = qualityScoreExt(extreme);
  check("quality: score clamped to [0,100]", s >= 0 && s <= 100);

  const noFrames = new QualityInputsExt({ throughputBps: 10000, capacityBps: 10000, jitterMs: 0, fecCorrections: 5, frames: 0 });
  check("quality: zero frames no fec penalty", Math.abs(qualityScoreExt(noFrames) - 100.0) < 1e-9);
}

// ---------- transfer_orchestrator_ext.js ----------
{
  const o = new TransferOrchestratorExt(223, 500);
  check("orchestrator: chunk count math", o.chunkCount(0) === 0 && o.chunkCount(1) === 1
    && o.chunkCount(223) === 1 && o.chunkCount(224) === 2 && o.chunkCount(500) === 3);

  const o2 = new TransferOrchestratorExt(4, 500);
  const chunks = o2.slice(new Array(10).fill(1));
  check("orchestrator: slice produces sequential chunks", chunks.length === 3
    && chunks[0].idx === 0 && chunks[0].data.length === 4
    && chunks[1].data.length === 4 && chunks[2].data.length === 2);

  // Encode + send: use the project's frame_codec for encoding.
  const { encodePayload } = await import("../js/frame_codec.js");
  const o3 = new TransferOrchestratorExt(223, 500);
  const data = new Array(30).fill(1);
  let sends = 0;
  const result = o3.run(data, (chunk) => encodePayload(chunk), (frame) => {
    sends += 1;
    return {};
  });
  check("orchestrator: full transfer succeeds", result.ok === true && result.bytes === 30 && sends === 1);

  const o4 = new TransferOrchestratorExt(223, 500);
  const result2 = o4.run(new Array(30).fill(1), (chunk) => encodePayload(chunk), () => ({ error: "link down" }));
  check("orchestrator: reports send failure", result2.ok === false && result2.reason.includes("link down"));
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}