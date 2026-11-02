// Behavioral tests for ui_state, throughput_meter, reassembly_ext web modules.
// Run: node tests/webmodules3.test.mjs (from web/)

import * as UI from "../js/ui_state.js";
import * as M from "../js/throughput_meter.js";
import * as R from "../js/reassembly_ext.js";

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

// ---------- ui_state.js ----------
{
  const m = new UI.UiStateMachine();
  check("ui_state starts idle", m.getState() === UI.UiState.IDLE && m.getProgress() === 0);

  m.startTransfer();
  check("ui_state transferring", m.getState() === UI.UiState.TRANSFERRING);
  m.setProgress(0.5);
  check("ui_state progress 0.5", Math.abs(m.getProgress() - 0.5) < 1e-9);
  m.complete();
  check("ui_state done + progress 1", m.getState() === UI.UiState.DONE && m.getProgress() === 1);

  const m2 = new UI.UiStateMachine();
  m2.startTransfer();
  m2.fail("BadCrc");
  check("ui_state error", m2.getState() === UI.UiState.ERROR && m2.getErrorCode() === "BadCrc");

  const m3 = new UI.UiStateMachine();
  m3.startTransfer();
  m3.setProgress(1.5);
  check("ui_state progress clamps to 1", m3.getProgress() === 1);
  m3.setProgress(-0.5);
  check("ui_state progress clamps to 0", m3.getProgress() === 0);
}

// ---------- throughput_meter.js ----------
{
  const m = new M.ThroughputMeter2(5000);
  m.add(10_485_760, 0);
  check("meter initial 0 mbps", m.mbps() === 0);
  m.add(0, 5000); // close the 5s window
  check("meter 10MiB/5s = 2 MB/s", Math.abs(m.mbps() - 2.0) < 0.01, `got ${m.mbps()}`);
  m.reset(5000);
  check("meter reset -> 0", m.mbps() === 0);
}

// ---------- reassembly_ext.js ----------
{
  const r = new R.ReassemblyExt(5);
  r.markReceived(0);
  r.markReceived(1);
  r.markReceived(3);
  check("reassembly gap at 2", r.nextMissing() === 2 && !r.isComplete());
  r.markReceived(2);
  r.markReceived(4);
  check("reassembly complete", r.isComplete() && r.nextMissing() === null);
  check("reassembly count 5", r.receivedCount() === 5);

  const r2 = new R.ReassemblyExt(4);
  r2.markReceived(2);
  r2.markReceived(0);
  check("reassembly out-of-order gap at 1", r2.nextMissing() === 1);

  const r3 = new R.ReassemblyExt(3);
  r3.markReceived(10);
  check("reassembly out-of-range ignored", r3.receivedCount() === 0);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (failures.length) {
  for (const name of failures) console.log(`  - ${name}`);
  process.exit(1);
}