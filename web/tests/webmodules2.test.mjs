// Behavioral tests for the new web modules: dedup, progress, reliability,
// throughput_estimator, link_stats, color_palette.
// Run: node tests/webmodules2.test.mjs (from web/)

import * as D from "../js/dedup.js";
import * as P from "../js/progress.js";
import * as R from "../js/reliability.js";
import * as T from "../js/throughput_estimator.js";
import * as L from "../js/link_stats.js";
import * as C from "../js/color_palette.js";

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

// ---------- dedup.js ----------
{
  // FNV-1a 64-bit known vector for "123456789".
  const bytes = Buffer.from("123456789");
  const h = D.fnv1a64(bytes);
  check("fnv1a64 known vector", h === 0x6d5573923c6cdfcn, `got ${h}`);

  const fpPresent = D.FileFingerprint.ofBytes(Buffer.from("already-here"));
  const fpAbsent = D.FileFingerprint.ofBytes(Buffer.from("new-file"));

  const rx = new D.DedupSet();
  rx.add(fpPresent);
  check("dedup: absent -> transfer", D.decide(fpAbsent, rx) === D.DedupDecision.TRANSFER);
  check("dedup: present -> skip", D.decide(fpPresent, rx) === D.DedupDecision.SKIP);

  const rx2 = new D.DedupSet();
  const stage = new D.DedupStage(rx2);
  rx2.add(fpPresent);
  check("dedup stage counts", (() => {
    const d1 = stage.process(fpAbsent);
    const d2 = stage.process(fpPresent);
    return d1 === D.DedupDecision.TRANSFER && d2 === D.DedupDecision.SKIP;
  })());
  check("dedup stage skipped=1", stage.skippedCount() === 1);
  check("dedup stage transferred=1", stage.transferredCount() === 1);
}

// ---------- progress.js ----------
{
  const t = new P.ProgressTracker(10, 100_000);
  t.markReceived(3, 30_000);
  const s = t.snapshot();
  check("progress fraction", Math.abs(s.fraction() - 0.3) < 1e-9);
  check("progress percent", Math.abs(s.percent() - 30.0) < 1e-9);
  check("progress remaining", s.remaining() === 7);

  t.setRate(2_000_000);
  const eta = t.etaSecs();
  check("progress eta", eta !== null && Math.abs(eta - 0.035) < 1e-6, `got ${eta}`);

  const t0 = new P.ProgressTracker(0, 0);
  check("progress zero total fraction 0", new P.ProgressSnapshot(0, 0, 0, 0).fraction() === 0);
}

// ---------- reliability.js ----------
{
  const perfect = new R.ReliabilitySnapshot(10, 10, 0, 0);
  check("reliability perfect goodput=1", Math.abs(perfect.goodputFraction() - 1.0) < 1e-9);
  check("reliability perfect quality=1", Math.abs(perfect.qualityScore() - 1.0) < 1e-9);

  const degraded = new R.ReliabilitySnapshot(10, 8, 4, 2);
  const score = degraded.qualityScore();
  check("reliability retries lower quality", score < 1.0 && score > 0.0, `got ${score}`);

  const zero = new R.ReliabilitySnapshot(0, 0, 0, 0);
  check("reliability zero total", zero.goodputFraction() === 0 && zero.qualityScore() === 0);

  const tr = new R.ReliabilityTracker();
  tr.markDelivered(10);
  tr.markRetry();
  tr.markRetry();
  tr.markNak();
  check("reliability tracker counts", tr.snapshot().retries === 2 && tr.snapshot().naks === 1);
  check("reliability tracker quality < 1", tr.qualityScore() < 1.0);
}

// ---------- throughput_estimator.js ----------
{
  const e = new T.ThroughputEstimator(0.5, 3);
  for (let i = 0; i < 20; i++) e.observe(2_000_000);
  check("estimator converges", Math.abs(e.value() - 2_000_000) < 1.0);
  check("estimator confident", e.isConfident());
  check("estimator sample count", e.sampleCount() === 20);

  const e2 = new T.ThroughputEstimator(0.3, 5);
  e2.observe(1_000_000);
  check("estimator first sample", Math.abs(e2.value() - 1_000_000) < 1e-6);
  check("estimator not confident (1<5)", !e2.isConfident());

  const e3 = new T.ThroughputEstimator(0.3, 5);
  for (let i = 0; i < 4; i++) e3.observe(2_000_000);
  check("estimator threshold 4<5 not confident", !e3.isConfident());
  e3.observe(2_000_000);
  check("estimator confident at 5", e3.isConfident());
}

// ---------- link_stats.js ----------
{
  const s = new L.LinkStats();
  s.addSentBytes(10_485_760); // 10 MiB
  s.setElapsed(5_000);
  check("link_stats sent_mbps=2.0", Math.abs(s.sentMbps() - 2.0) < 0.01, `got ${s.sentMbps()}`);

  const s0 = new L.LinkStats();
  s0.addSentBytes(10_485_760);
  check("link_stats zero elapsed -> 0 mbps", s0.sentMbps() === 0);

  const s2 = new L.LinkStats();
  s2.chunks_total = 10;
  s2.chunks_delivered = 7;
  check("link_stats delivery_ratio=0.7", Math.abs(s2.deliveryRatio() - 0.7) < 1e-9);

  const s3 = new L.LinkStats();
  s3.addSentBytes(100);
  s3.setElapsed(1000);
  const r = s3.report();
  check("link_stats report shape", r.startsWith("link_stats:") && r.includes("sent=100B") && r.includes("elapsed=1000ms"));
}

// ---------- color_palette.js ----------
{
  check("palette size 8", C.paletteSize() === 8);
  for (let v = 0; v < 8; v++) {
    const rgb = C.colorForValue(v);
    check(`palette roundtrip value ${v}`, C.valueForColor(rgb) === v, `got ${C.valueForColor(rgb)}`);
  }
  check("palette unknown color -> null", C.valueForColor([10, 200, 30]) === null);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (failures.length) {
  for (const name of failures) console.log(`  - ${name}`);
  process.exit(1);
}