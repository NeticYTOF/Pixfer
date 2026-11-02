// Behavioral tests for the new web modules: packet_reorder_buffer,
// color_lut, camera_focus_controller, screen_brightness_governor,
// link_integrity_report, rs_tuner.
// Run: node tests/webmodules14.test.mjs (from web/)

import { BufferedPacket, PacketReorderBuffer } from "../js/packet_reorder_buffer.js";
import { ColorLut, quantize } from "../js/color_lut.js";
import { CameraFocusController, FocusSample } from "../js/camera_focus_controller.js";
import { ScreenBrightnessGovernor, BrightnessReading } from "../js/screen_brightness_governor.js";
import { LinkIntegrityReport, IntegritySample } from "../js/link_integrity_report.js";
import { RSTuner } from "../js/rs_tuner.js";

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

function pkt(seq, bytes) {
  return new BufferedPacket({ seq, payload: bytes });
}

// ---------- packet_reorder_buffer.js ----------
{
  const b = new PacketReorderBuffer();
  check("reorder: starts at seq 0", b.getNextSeq() === 0 && b.pending() === 0 && b.popNext() === null);

  const b2 = new PacketReorderBuffer();
  b2.insert(pkt(0, [1, 2, 3]));
  const p = b2.popNext();
  check("reorder: in-order packet pops immediately", p !== null && p.seq === 0 && JSON.stringify(p.payload) === JSON.stringify([1, 2, 3]) && b2.getNextSeq() === 1 && b2.pending() === 0);

  const b3 = new PacketReorderBuffer();
  b3.insert(pkt(2, [9]));
  check("reorder: out-of-order stays buffered", b3.popNext() === null && b3.pending() === 1 && b3.minSeq() === 2 && b3.contains(2));
  b3.insert(pkt(0, [0]));
  b3.insert(pkt(1, [1]));
  const p0 = b3.popNext();
  check("reorder: filling gap pops seq 0", p0 !== null && p0.seq === 0);
  const p1 = b3.popNext();
  check("reorder: then seq 1", p1 !== null && p1.seq === 1);
  const p2 = b3.popNext();
  check("reorder: then buffered seq 2", p2 !== null && p2.seq === 2);
  check("reorder: empty after drain", b3.popNext() === null);

  const b4 = new PacketReorderBuffer();
  b4.insert(pkt(1, [1]));
  b4.insert(pkt(1, [1, 1]));
  check("reorder: duplicate seq overwrites", b4.pending() === 1);
  b4.insert(pkt(0, [0]));
  b4.insert(pkt(2, [2]));
  const q0 = b4.popNext();
  check("reorder: pop seq 0", q0 !== null && q0.seq === 0);
  const q1 = b4.popNext();
  check("reorder: pop seq 1 (overwritten payload)", q1 !== null && q1.seq === 1 && JSON.stringify(q1.payload) === JSON.stringify([1, 1]));
  const q2 = b4.popNext();
  check("reorder: pop seq 2", q2 !== null && q2.seq === 2);

  const b5 = new PacketReorderBuffer();
  for (let i = 49; i >= 0; i--) b5.insert(pkt(i, [i]));
  let orderOk = true;
  for (let i = 0; i < 50; i++) {
    const p = b5.popNext();
    if (p === null || p.seq !== i || p.payload[0] !== i) {
      orderOk = false;
      break;
    }
  }
  check("reorder: 50-packet FIFO order", orderOk && b5.popNext() === null);
}

// ---------- color_lut.js ----------
{
  const lut = new ColorLut();
  check("lut: covers 8 colors", lut.size() === 8);

  for (let v = 0; v < 8; v++) {
    const c = lut.colorFor(v);
    check(`lut: exact palette color ${v} maps to ${v}`, lut.valueFor(c) === v);
  }

  check("lut: (128,0,0) -> red(2)", lut.valueFor([128, 0, 0]) === 2);
  check("lut: (128,128,128) -> gray(7)", lut.valueFor([128, 128, 128]) === 7);

  const q = quantize([200, 30, 250]);
  check("lut: quantize snaps to palette", Array.isArray(q) && q.length === 3);
  check("lut: colorFor matches palette", JSON.stringify(lut.colorFor(0)) === JSON.stringify([0, 0, 0])
    && JSON.stringify(lut.colorFor(1)) === JSON.stringify([255, 255, 255])
    && JSON.stringify(lut.colorFor(7)) === JSON.stringify([128, 128, 128]));

  check("lut: tie prefers lower index", lut.valueFor([127, 0, 0]) === 0);
}

// ---------- camera_focus_controller.js ----------
{
  const c = new CameraFocusController(0.5);
  check("focus: starts with no samples", c.refocusCount() === 0 && c.lastSample() === null && !c.isBlurry() && c.threshold() === 0.5);

  const c2 = new CameraFocusController(0.5);
  c2.record(new FocusSample({ defocus: 0.1, tMs: 0 }));
  check("focus: sharp sample no refocus", c2.refocusCount() === 0 && !c2.isBlurry() && c2.lastSample().defocus === 0.1);

  const c3 = new CameraFocusController(0.5);
  c3.record(new FocusSample({ defocus: 0.8, tMs: 0 }));
  check("focus: blurry sample counts refocus", c3.refocusCount() === 1 && c3.isBlurry());
  c3.record(new FocusSample({ defocus: 0.9, tMs: 1 }));
  check("focus: refocus accumulates", c3.refocusCount() === 2);
}

// ---------- screen_brightness_governor.js ----------
{
  const g = new ScreenBrightnessGovernor(200);
  check("brightness: starts with no readings", g.adjustmentCount() === 0 && g.lastReading() === null && !g.atTarget() && g.targetValue() === 200);

  const g2 = new ScreenBrightnessGovernor(200);
  g2.record(new BrightnessReading({ level: 200, tMs: 0 }));
  check("brightness: at target no adjustment", g2.adjustmentCount() === 0 && g2.atTarget() && g2.lastReading().level === 200);

  const g3 = new ScreenBrightnessGovernor(200);
  g3.record(new BrightnessReading({ level: 50, tMs: 0 }));
  check("brightness: off target counts adjustment", g3.adjustmentCount() === 1 && !g3.atTarget());
  g3.record(new BrightnessReading({ level: 150, tMs: 1 }));
  check("brightness: adjustments accumulate", g3.adjustmentCount() === 2);
}

// ---------- link_integrity_report.js ----------
{
  const r = new LinkIntegrityReport();
  check("integrity: empty report zero rate", r.total() === 0 && r.passRate() === 0 && r.allPassed());

  const r2 = new LinkIntegrityReport();
  r2.record(new IntegritySample({ ok: true, tMs: 0 }));
  r2.record(new IntegritySample({ ok: true, tMs: 1 }));
  r2.record(new IntegritySample({ ok: false, tMs: 2 }));
  check("integrity: record and counts", r2.passedCount() === 2 && r2.failedCount() === 1 && r2.total() === 3 && !r2.allPassed());

  const r3 = new LinkIntegrityReport();
  for (let i = 0; i < 9; i++) r3.record(new IntegritySample({ ok: true, tMs: i }));
  r3.record(new IntegritySample({ ok: false, tMs: 9 }));
  check("integrity: pass rate math", Math.abs(r3.passRate() - 0.9) < 1e-9 && r3.failedCount() === 1);

  const r4 = new LinkIntegrityReport();
  r4.record(new IntegritySample({ ok: true, tMs: 0 }));
  r4.record(new IntegritySample({ ok: true, tMs: 1 }));
  check("integrity: all passed when no failures", r4.allPassed());
}

// ---------- rs_tuner.js ----------
{
  const t = new RSTuner();
  check("rs_tuner: starts at default parity 16", t.parityCount() === 16 && t.observedBer() === 0);
  t.observe(0.0001);
  check("rs_tuner: clean link uses min parity", t.parityCount() === 16);
  t.observe(0.005);
  check("rs_tuner: moderate BER raises parity", t.parityCount() === 24);
  t.observe(0.02);
  check("rs_tuner: higher BER raises more", t.parityCount() === 32);
  t.observe(0.2);
  check("rs_tuner: heavy BER uses max parity", t.parityCount() === 255 && t.observedBer() === 0.2);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}