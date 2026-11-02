// Behavioral tests for the new web modules: color_temperature,
// transfer_resume, link_budget_ext.
// Run: node tests/webmodules9.test.mjs (from web/)

import { TemperatureCorrector } from "../js/color_temperature.js";
import { TransferResume } from "../js/transfer_resume.js";
import { LinkBudgetExt } from "../js/link_budget_ext.js";

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

// ---------- color_temperature.js ----------
{
  const c = new TemperatureCorrector(5500);
  c.compensate({ kelvin: 5500, valid: true });
  check("temp: neutral no change", Math.abs(c.gains().r - 1.0) < 1e-9 && Math.abs(c.gains().b - 1.0) < 1e-9);

  const c2 = new TemperatureCorrector(5500);
  c2.compensate({ kelvin: 7000, valid: true });
  const g = c2.gains();
  check("temp: warm boosts red", g.r > 1.0, `got r=${g.r}`);
  check("temp: warm cuts blue", g.b < 1.0, `got b=${g.b}`);
  check("temp: green stays 1.0", Math.abs(g.g - 1.0) < 1e-9);

  const c3 = new TemperatureCorrector(5500);
  c3.compensate({ kelvin: 4000, valid: true });
  const g3 = c3.gains();
  check("temp: cool cuts red", g3.r < 1.0, `got r=${g3.r}`);
  check("temp: cool boosts blue", g3.b > 1.0, `got b=${g3.b}`);

  const c4 = new TemperatureCorrector(5500);
  c4.compensate({ kelvin: 7000, valid: false });
  check("temp: invalid reading ignored", Math.abs(c4.gains().r - 1.0) < 1e-9);

  const c5 = new TemperatureCorrector(5500);
  c5.compensate({ kelvin: 15500, valid: true });
  const g5 = c5.gains();
  check("temp: gains clamped", g5.r <= 1.5 && g5.r >= 0.5 && g5.b <= 1.5 && g5.b >= 0.5);

  const c6 = new TemperatureCorrector(5500);
  c6.compensate({ kelvin: 7000, valid: true });
  const pal = c6.correctedPalette();
  check("temp: red channel boosted", pal[2].r >= 255, `got ${JSON.stringify(pal[2])}`);
  check("temp: blue channel cut", pal[4].b < 255, `got ${JSON.stringify(pal[4])}`);

  check("temp: target preserved", c6.target() === 5500);
}

// ---------- transfer_resume.js ----------
{
  const r = new TransferResume(5);
  check("resume: starts at chunk 0", r.resumePoint().nextChunk === 0);
  r.markDelivered(0);
  r.markDelivered(1);
  check("resume: advances to chunk 2", r.resumePoint().nextChunk === 2);
  r.markDelivered(2);
  r.markDelivered(3);
  r.markDelivered(4);
  check("resume: complete at end", r.isComplete() === true);
  check("resume: next chunk = total", r.resumePoint().nextChunk === 5);

  const r2 = new TransferResume(4);
  r2.markDelivered(0);
  r2.markDelivered(1);
  check("resume: delivered count", r2.deliveredCount() === 2);
  check("resume: resume point chunk 2", r2.resumePoint().nextChunk === 2);

  const r3 = new TransferResume(3);
  r3.markDelivered(0);
  const s = r3.status();
  check("resume: status string", s === "resume from chunk 1/3", `got ${s}`);

  const r4 = new TransferResume(0);
  check("resume: total 0 no resume point", r4.resumePoint() === null);
  check("resume: total 0 status", r4.status() === "no resume point");

  const r5 = new TransferResume(4);
  r5.markDelivered(2);
  r5.markDelivered(0);
  check("resume: out-of-order stays at missing 1", r5.resumePoint().nextChunk === 1);
  r5.markDelivered(1);
  check("resume: after 1, advances to 3", r5.resumePoint().nextChunk === 3);
}

// ---------- link_budget_ext.js ----------
{
  const b = new LinkBudgetExt(1000000, [
    { flowId: 1, weight: 1, minBps: 0 },
    { flowId: 2, weight: 1, minBps: 0 },
  ]);
  const s = b.allocate();
  check("budget: equal split", Math.abs(s.perFlow[0].bytesPerSec - 500000) < 1e-6 && Math.abs(s.perFlow[1].bytesPerSec - 500000) < 1e-6);
  check("budget: no waste", Math.abs(s.waste) < 1e-6);
  check("budget: total capacity", Math.abs(s.totalCapacity - 1000000) < 1e-6);

  const b2 = new LinkBudgetExt(4000000, [
    { flowId: 1, weight: 3, minBps: 0 },
    { flowId: 2, weight: 1, minBps: 0 },
  ]);
  const s2 = b2.allocate();
  check("budget: weighted 3:1", Math.abs(s2.perFlow[0].bytesPerSec - 3000000) < 1e-6 && Math.abs(s2.perFlow[1].bytesPerSec - 1000000) < 1e-6);

  const b3 = new LinkBudgetExt(1000000, [
    { flowId: 1, weight: 1, minBps: 200000 },
    { flowId: 2, weight: 1, minBps: 200000 },
  ]);
  const s3 = b3.allocate();
  check("budget: minimums reserved first", Math.abs(s3.perFlow[0].bytesPerSec - 500000) < 1e-6 && Math.abs(s3.perFlow[1].bytesPerSec - 500000) < 1e-6);

  const b4 = new LinkBudgetExt(300000, [
    { flowId: 1, weight: 1, minBps: 200000 },
    { flowId: 2, weight: 1, minBps: 200000 },
  ]);
  const s4 = b4.allocate();
  check("budget: below minimums unsatisfied", !s4.summary.plans.every((p) => p.satisfied));

  const b5 = new LinkBudgetExt(2000000, [{ flowId: 7, weight: 5, minBps: 100000 }]);
  check("budget: flow count", b5.flowCount() === 1);
  check("budget: capacity", Math.abs(b5.capacity - 2000000) < 1e-6);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}