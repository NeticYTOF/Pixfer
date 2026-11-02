// Behavioral tests for the new web modules: color_drift_monitor,
// transfer_heartbeat, link_throughput_probe, frame_dedup, rx_exposure_lock.
// Run: node tests/webmodules13.test.mjs (from web/)

import { ColorDriftMonitor, DriftSample, expectedPalette } from "../js/color_drift_monitor.js";
import { TransferHeartbeat, HeartbeatSample } from "../js/transfer_heartbeat.js";
import { LinkThroughputProbe, ThroughputProbeResult } from "../js/link_throughput_probe.js";
import { FrameDedup } from "../js/frame_dedup.js";
import { RxCameraExposureLock, suggestExposure } from "../js/rx_exposure_lock.js";

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

// ---------- color_drift_monitor.js ----------
{
  const pal = expectedPalette();
  const m = new ColorDriftMonitor();
  check("drift: no samples zero drift", m.meanDriftOf(0) === 0 && m.meanDriftOf(7) === 0 && m.worstColor() === null);

  m.record(new DriftSample({ colorIndex: 2, r: 255, g: 0, b: 0, tMs: 1000 }), pal);
  check("drift: exact match zero drift", m.meanDriftOf(2) === 0 && m.sampleCount(2) === 1);

  const m2 = new ColorDriftMonitor();
  m2.record(new DriftSample({ colorIndex: 3, r: 0, g: 200, b: 0, tMs: 1000 }), pal);
  // expected green (0,255,0): |200-255| = 55 / 3 = 18.33
  check("drift: drifted sample raises mean", Math.abs(m2.meanDriftOf(3) - 55 / 3) < 1e-9, `got ${m2.meanDriftOf(3)}`);
  check("drift: exceeds threshold", m2.exceeds(3, 10) && !m2.exceeds(3, 30));

  const m3 = new ColorDriftMonitor();
  m3.record(new DriftSample({ colorIndex: 5, r: 100, g: 100, b: 100, tMs: 0 }), pal);
  const afterFirst = m3.meanDriftOf(5);
  check("drift: first sample drift", Math.abs(afterFirst - (155 + 100 + 155) / 3) < 1e-9, `got ${afterFirst}`);
  m3.record(new DriftSample({ colorIndex: 5, r: 255, g: 0, b: 255, tMs: 1 }), pal);
  check("drift: rolling mean halves", Math.abs(m3.meanDriftOf(5) - afterFirst / 2) < 1e-9 && m3.sampleCount(5) === 2);

  const m4 = new ColorDriftMonitor();
  m4.record(new DriftSample({ colorIndex: 4, r: 200, g: 200, b: 55, tMs: 0 }), pal);
  const worst = m4.worstColor();
  check("drift: worst color found", worst !== null && worst.colorIndex === 4 && Math.abs(worst.drift - 200) < 1e-9);

  const m5 = new ColorDriftMonitor();
  for (let i = 0; i < 300; i++) {
    m5.record(new DriftSample({ colorIndex: i % 8, r: 10, g: 10, b: 10, tMs: i }), pal);
  }
  check("drift: ring caps at 256", m5.length() === 256);

  const m6 = new ColorDriftMonitor();
  m6.record(new DriftSample({ colorIndex: 0, r: 50, g: 50, b: 50, tMs: 0 }), pal);
  m6.reset();
  check("drift: reset clears state", m6.length() === 0 && m6.sampleCount(0) === 0 && m6.meanDriftOf(0) === 0);

  const m7 = new ColorDriftMonitor();
  m7.record(new DriftSample({ colorIndex: 8, r: 10, g: 10, b: 10, tMs: 0 }), pal);
  check("drift: index wraps at 8", m7.sampleCount(0) === 1 && Math.abs(m7.meanDriftOf(0) - 10) < 1e-9);
}

// ---------- transfer_heartbeat.js ----------
{
  const h = new TransferHeartbeat(1000, 3);
  check("heartbeat: no heartbeats not stale", !h.isStale() && h.beatCount() === 0 && h.lastSample() === null);

  const h2 = new TransferHeartbeat(1000, 3);
  h2.record(new HeartbeatSample({ tMs: 0, seq: 0 }));
  h2.record(new HeartbeatSample({ tMs: 1000, seq: 1 }));
  h2.record(new HeartbeatSample({ tMs: 2000, seq: 2 }));
  check("heartbeat: on-time resets missed", h2.missedCount() === 0 && h2.beatCount() === 3 && !h2.isStale());

  const h3 = new TransferHeartbeat(1000, 2);
  h3.record(new HeartbeatSample({ tMs: 0, seq: 0 }));
  h3.record(new HeartbeatSample({ tMs: 5000, seq: 1 }));
  check("heartbeat: slow sample adds one miss", h3.missedCount() === 1);
  h3.record(new HeartbeatSample({ tMs: 10000, seq: 2 }));
  check("heartbeat: stale after threshold", h3.isStale() && h3.missedCount() === 2);

  const h4 = new TransferHeartbeat(500, 5);
  check("heartbeat: interval preserved", h4.interval() === 500);

  const h5 = new TransferHeartbeat(1000, 3);
  h5.record(new HeartbeatSample({ tMs: 42, seq: 7 }));
  const l = h5.lastSample();
  check("heartbeat: last sample returned", l.tMs === 42 && l.seq === 7);

  const h6 = new TransferHeartbeat(1000, 1);
  h6.record(new HeartbeatSample({ tMs: 0, seq: 0 }));
  h6.record(new HeartbeatSample({ tMs: 1e15, seq: 1 }));
  check("heartbeat: large gap flags stale", h6.isStale());
}

// ---------- link_throughput_probe.js ----------
{
  const p = new LinkThroughputProbe(223, 4);
  check("probe: bytes and rounds preserved", p.probeBytes === 223 && p.rounds === 4);

  const r = p.run((_, __) => 33, null);
  check("probe: bps math", Math.abs(r.bps - (223 * 4) / 0.132) < 1e-6 && r.bytes === 892 && r.elapsedMs === 132, `got ${r.bps}`);

  const p2 = new LinkThroughputProbe(100, 2);
  const r2 = p2.run((_, __) => 0, null);
  check("probe: zero elapsed infinite bps", r2.bps === Infinity && r2.bytes === 200);

  const state = { calls: 0 };
  const times = [10, 20, 30];
  const p3 = new LinkThroughputProbe(50, 3);
  const r3 = p3.run((_, s) => {
    const t = times[s.calls % 3];
    s.calls++;
    return t;
  }, state);
  check("probe: variable round times averaged", r3.elapsedMs === 60 && Math.abs(r3.bps - 2500) < 1e-6, `got ${r3.bps}`);

  const res = new ThroughputProbeResult({ bytes: 10, elapsedMs: 5, bps: 2000 });
  check("probe: result shape", res.bytes === 10 && res.elapsedMs === 5 && res.bps === 2000);
}

// ---------- frame_dedup.js ----------
{
  const d = new FrameDedup();
  check("dedup: starts empty", d.isEmpty() && d.size() === 0 && !d.isDelivered(0));

  d.markDelivered(0);
  d.markDelivered(1);
  check("dedup: mark and query", d.isDelivered(0) && d.isDelivered(1) && !d.isDelivered(2) && d.size() === 2);

  d.markDelivered(5);
  d.markDelivered(5);
  check("dedup: duplicates idempotent", d.size() === 3);

  d.clear();
  check("dedup: clear resets", d.isEmpty() && !d.isDelivered(0));
}

// ---------- rx_exposure_lock.js ----------
{
  const l = new RxCameraExposureLock();
  check("exposure: starts unlocked", !l.isLocked());
  l.lock(64);
  check("exposure: lock sets target and flag", l.isLocked() && l.targetValue() === 64);
  l.unlock();
  check("exposure: unlock keeps target", !l.isLocked() && l.targetValue() === 64);

  check("exposure: dark scene higher exposure", suggestExposure(20) > suggestExposure(200));
  check("exposure: luma floor prevents div-by-zero", suggestExposure(0) === 255);
  check("exposure: 255 luma -> 156", suggestExposure(255) === 156, `got ${suggestExposure(255)}`);
  let rangeOk = true;
  for (const luma of [0, 1, 10, 128, 255]) {
    if (suggestExposure(luma) < 0 || suggestExposure(luma) > 255) rangeOk = false;
  }
  check("exposure: stays in 0..=255", rangeOk);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}