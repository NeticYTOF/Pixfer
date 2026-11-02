// Behavioral tests for the new web modules: config, metrics, telemetry.
// Run: node tests/webmodules.test.mjs (from web/)

import * as CFG from "../js/config.js";
import * as M from "../js/metrics.js";
import * as T from "../js/telemetry.js";

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

// ---------- config.js ----------
{
  const def = new CFG.Config();
  check("Config: defaults", def.profile === "balanced" && def.defaultThroughput === 2_000_000);
  check("Config: validate() clean", def.validate().length === 0);

  const lp = CFG.Config.forProfile(CFG.ConfigProfile.LOW_POWER);
  check("forProfile(low_power)", lp.profile === "low_power" && lp.maxRetries === 5 && lp.ackTimeoutMs === 4000);

  const mt = CFG.Config.forProfile(CFG.ConfigProfile.MAX_THROUGHPUT);
  check("forProfile(max_throughput)", mt.defaultThroughput === 4_000_000 && mt.lockExposure === true && mt.ackTimeoutMs === 1500);

  const bad = new CFG.Config();
  bad.defaultThroughput = 0;
  bad.maxRetries = 0;
  check("validate catches bad fields", bad.validate().length === 2, `got ${bad.validate().length}`);

  check("needsZip(60MB) false", !def.needsZip(60_000_000));
  check("needsZip(61MB) true", def.needsZip(61_000_000));
  check("estimatedSeconds(60MB) == 30", def.estimatedSeconds(60_000_000) === 30);

  const b = new CFG.ConfigBuilder()
    .withProfile(CFG.ConfigProfile.LOW_POWER)
    .throughput(1_500_000)
    .build();
  check("ConfigBuilder overrides profile default", b.defaultThroughput === 1_500_000 && b.maxRetries === 5 && b.profile === "low_power");

  const reg = new CFG.FeatureRegistry();
  check("FeatureRegistry count", reg.enabledCount() === 5, `got ${reg.enabledCount()}`);
  check("FeatureRegistry summary", reg.summary().includes("auto_zip"));

  // The toJson fix: must parse as valid JSON.
  const json = reg.toJson();
  let parsed = null, jsonOk = false;
  try { parsed = JSON.parse(json); jsonOk = true; } catch (e) { jsonOk = false; }
  check("FeatureRegistry.toJson parses", jsonOk, json);
  check("toJson contains every feature", parsed && Object.keys(parsed).length === reg.features.size,
    `keys=${parsed ? Object.keys(parsed).length : 0}`);

  check("FeatureRegistry enable/disable", (() => { reg.enable("exposure_lock"); const on = reg.isEnabled("exposure_lock"); reg.disable("exposure_lock"); return on && !reg.isEnabled("exposure_lock"); })());
}

// ---------- metrics.js ----------
{
  const m = new M.Metrics();
  const c = m.counter("transfers");
  c.inc(); c.inc(2);
  check("Counter inc", c.val() === 3, `got ${c.val()}`);
  c.dec();
  check("Counter dec clamps at 0", c.dec(99).val() === 0 && m.counter("transfers").val() === 0);

  const g = m.gauge("queue_depth").set(7);
  check("Gauge set/add", g.add(3).val() === 10);

  const h = new M.Histogram([1, 5, 10, 25, 50, 100, 250, 500, 1000, 5000]);
  for (const v of [2, 4, 10, 25, 8, 100, 30, 50, 250, 700, 2000, 9000]) h.record(v);
  check("Histogram quantiles sane", h.p50() >= 10 && h.p95() >= 500 && h.p99() === 5000,
    `p50=${h.p50()} p95=${h.p95()} p99=${h.p99()}`);
  // 12 recorded values sum to 12179 -> mean = 1014.9166...
  check("Histogram mean", Math.abs(h.mean() - 1014.9166666666) < 0.001, `got ${h.mean()}`);

  const tm = new M.ThroughputMeter();
  const t0 = Date.now();
  tm.add(1_000_000, t0);
  // Force the window to close at +5s to verify mbps math.
  const mbps = tm.mbps();
  check("ThroughputMeter initial", mbps === 0, `got ${mbps}`);
  tm.add(5_000_000, t0 + 5000); // closes the window
  // 6MB total over the 5s window -> 1.2 MB/s
  check("ThroughputMeter reports ~1.2 MB/s after window close", Math.abs(tm.mbps() - 1.2) < 0.05, `got ${tm.mbps()}`);

  const rep = JSON.parse(m.report());
  check("Metrics.report parses", rep.counters.transfers === 0 && rep.gauges.queue_depth === 10 && rep.throughput_mbps === 0);
}

// ---------- telemetry.js ----------
{
  const bus = new T.TelemetryBus();
  const rec = new T.TelemetryRecorder();
  rec.attach(bus);

  bus.emit("transfer.started", { bytes: 61_000_000 });
  bus.emit("chunk.sent", { idx: 0 });
  bus.emit("chunk.acked", { idx: 0 });
  bus.emit("transfer.completed", { bytes: 61_000_000 });

  check("TelemetryBus subscriberCount", bus.subscriberCount() === 1);
  check("Recorder counts events", rec.count("transfer.started") === 1 && rec.count("chunk.sent") === 1 && rec.count("transfer.completed") === 1);
  check("Recorder last event", rec.last && rec.last.name === "transfer.completed");
  check("Recorder last data carried", rec.last.data.bytes === 61_000_000, `got ${rec.last.data.bytes}`);

  const rep = JSON.parse(rec.report());
  check("Recorder.report parses", rep.counts["transfer.started"] === 1 && rep.last === "transfer.completed");
  check("TELEMETRY_NAMES frozen", Object.isFrozen(T.TELEMETRY_NAMES));

  // The emitter's wildcard handler must hand the payload to TelemetryEvent
  // correctly (regression for the (type, payload) vs (type, ...rest) bug).
  const seen = [];
  const bus2 = new T.TelemetryBus();
  bus2.subscribe((ev) => seen.push(ev));
  bus2.emit("exposure.locked", { cam: 0 });
  check("TelemetryBus subscribe delivers event", seen.length === 1 && seen[0].name === "exposure.locked" && seen[0].data.cam === 0);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (failures.length) {
  for (const name of failures) console.log(`  - ${name}`);
  process.exit(1);
}