// Behavioral tests for the new web modules: color_adaptive,
// transfer_watchdog, link_probe.
// Run: node tests/webmodules8.test.mjs (from web/)

import { AdaptivePalette, recalibrate } from "../js/color_adaptive.js";
import { TransferWatchdog, WatchdogState } from "../js/transfer_watchdog.js";
import { LinkProbe } from "../js/link_probe.js";

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

// ---------- color_adaptive.js ----------
{
  const measured = [
    { r: 0, g: 0, b: 0 },
    { r: 1, g: 1, b: 1 },
    { r: 1, g: 0, b: 0 },
    { r: 0, g: 1, b: 0 },
    { r: 0, g: 0, b: 1 },
    { r: 1, g: 0, b: 1 },
    { r: 1, g: 1, b: 0 },
    { r: 0.5, g: 0.5, b: 0.5 },
  ];
  const p = AdaptivePalette.fromMeasurements(measured);
  check("adaptive: default palette detected", p.isDefault() === true);

  const drifted = [
    { r: 0.05, g: 0.05, b: 0.05 },
    { r: 0.9, g: 0.9, b: 0.9 },
    { r: 1, g: 0.1, b: 0 },
    { r: 0, g: 1, b: 0.1 },
    { r: 0, g: 0.1, b: 1 },
    { r: 1, g: 0.1, b: 1 },
    { r: 1, g: 1, b: 0.1 },
    { r: 0.5, g: 0.5, b: 0.5 },
  ];
  const p2 = AdaptivePalette.fromMeasurements(drifted);
  check("adaptive: drifted palette not default", p2.isDefault() === false);
  const red = p2.colorForValue(2);
  check("adaptive: red shifted toward orange", red.g === 26, `got ${JSON.stringify(red)}`);

  const p3 = AdaptivePalette.fromMeasurements([
    { r: 1, g: 0, b: 0 },
    { r: 0, g: 1, b: 0 },
    { r: 0, g: 0, b: 1 },
    { r: 1, g: 0, b: 1 },
    { r: 1, g: 1, b: 0 },
    { r: 0.5, g: 0.5, b: 0.5 },
    { r: 0.2, g: 0.2, b: 0.2 },
    { r: 0.8, g: 0.8, b: 0.8 },
  ]);
  check("adaptive: value 0 = red", JSON.stringify(p3.colorForValue(0)) === JSON.stringify({ r: 255, g: 0, b: 0 }));
  check("adaptive: value 7 = 204 gray", JSON.stringify(p3.colorForValue(7)) === JSON.stringify({ r: 204, g: 204, b: 204 }));

  const p4 = recalibrate([{ r: 0.5, g: 0.5, b: 0.5 }]);
  check("adaptive: recalibrate alias", p4.isDefault() === false);

  const p5 = AdaptivePalette.fromMeasurements([{ r: 1, g: 1, b: 1 }]);
  check("adaptive: single measurement pads rest with black",
    JSON.stringify(p5.colorForValue(0)) === JSON.stringify({ r: 255, g: 255, b: 255 }) &&
    JSON.stringify(p5.colorForValue(1)) === JSON.stringify({ r: 0, g: 0, b: 0 }) &&
    JSON.stringify(p5.colorForValue(7)) === JSON.stringify({ r: 0, g: 0, b: 0 }));
}

// ---------- transfer_watchdog.js ----------
{
  const w = new TransferWatchdog(1000);
  check("watchdog: starts idle", w.getState() === WatchdogState.IDLE);
  check("watchdog: timeout preserved", w.timeout() === 1000);

  const w2 = new TransferWatchdog(500);
  w2.start();
  check("watchdog: active after start", w2.getState() === WatchdogState.ACTIVE);
  w2.nowMs = 400;
  check("watchdog: not timed out at 400ms", w2.check() === false);
  w2.nowMs = 600;
  check("watchdog: timed out at 600ms", w2.check() === true);
  check("watchdog: state is timed_out", w2.getState() === WatchdogState.TIMED_OUT);

  const w3 = new TransferWatchdog(500);
  w3.start();
  w3.nowMs = 300;
  w3.recordProgress();
  w3.nowMs = 700;
  check("watchdog: progress resets timer", w3.check() === false);
  w3.nowMs = 850;
  check("watchdog: times out after progress", w3.check() === true);

  const w4 = new TransferWatchdog(1000);
  w4.start();
  w4.stop();
  check("watchdog: stop returns to idle", w4.getState() === WatchdogState.IDLE);

  const w5 = new TransferWatchdog(100);
  w5.nowMs = 10000;
  check("watchdog: check when idle is false", w5.check() === false);
  check("watchdog: still idle", w5.getState() === WatchdogState.IDLE);
}

// ---------- link_probe.js ----------
{
  const p = new LinkProbe(500);
  // Simulate: clock advances past the timeout, no pong.
  let now = 0;
  const r1 = p.run({
    sendPing: () => {},
    recvPong: () => null,
    nowMs: () => {
      now = Math.max(now, 600); // jump past the 500ms timeout
      return now;
    },
    payload: [0xAB],
  });
  check("probe: times out with no response", r1.responded === false && r1.rttMs === 0);

  const p2 = new LinkProbe(1000);
  let clockMs = 0;
  let gotPong = false;
  const r2 = p2.run({
    sendPing: () => {
      clockMs = 120; // pong arrives after 120 ms
    },
    recvPong: () => {
      if (clockMs >= 120 && !gotPong) {
        gotPong = true;
        return [0xAB];
      }
      return null;
    },
    nowMs: () => clockMs,
    payload: [0xAB],
  });
  check("probe: measures RTT", r2.responded === true && r2.rttMs === 120, `got rtt ${r2.rttMs}`);
  check("probe: echoes payload", JSON.stringify(r2.payload) === JSON.stringify([0xAB]));

  const p3 = new LinkProbe(1000);
  const r3 = p3.run({
    sendPing: () => {},
    recvPong: () => [1, 2, 3],
    nowMs: () => 0,
    payload: [1, 2, 3],
  });
  check("probe: echoes arbitrary payload", JSON.stringify(r3.payload) === JSON.stringify([1, 2, 3]));

  check("probe: timeout preserved", p3.timeout() === 1000);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}