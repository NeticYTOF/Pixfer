// Behavioral tests for the new web modules: color_quantizer_ext, rs_ext,
// transfer_resume_ext.
// Run: node tests/webmodules23.test.mjs (from web/)

import { ColorQuantizerExt, QuantResult } from "../js/color_quantizer_ext.js";
import { RSExt, RsBatchResult } from "../js/rs_ext.js";
import { TransferResumeExt, ResumePointExt } from "../js/transfer_resume_ext.js";

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

// ---------- color_quantizer_ext.js ----------
{
  const q = new ColorQuantizerExt(0);
  const r = q.quantize([128, 128, 128]);
  check("quant_ext: exact palette color zero distance", r instanceof QuantResult && r.value === 7 && r.dist2 === 0
    && JSON.stringify(r.color) === JSON.stringify([128, 128, 128]));

  const r2 = q.quantize([200, 30, 250]);
  check("quant_ext: off-palette snaps to nearest", r2.value === 5 && Math.abs(r2.dist2 - 3950) < 1e-6, `got ${r2.dist2}`);

  const q2 = new ColorQuantizerExt(10);
  const [r3, ok] = q2.quantizeWithTolerance([128, 128, 128]);
  check("quant_ext: tolerance gates acceptance", ok === true && r3.value === 7);
  const [r4, ok2] = q2.quantizeWithTolerance([10, 10, 10]);
  check("quant_ext: far color rejected by tolerance", ok2 === false);

  const r5 = q.quantize([127, 0, 0]);
  check("quant_ext: tie prefers lower index", r5.value === 0);
}

// ---------- rs_ext.js ----------
{
  const r = new RSExt();
  const cw = r.encode(new Array(223).fill(7));
  check("rs_ext: encode produces full codeword", cw.length === 255 && r.parityBytes() === 32);

  const good = new Array(223).fill(7);
  const cw2 = r.encode(good);
  r.verify(cw2); // should not throw
  check("rs_ext: verify accepts valid codeword", true);

  const corrupted = r.encode(good);
  corrupted[0] ^= 0xff;
  let threw = false;
  try {
    r.verify(corrupted);
  } catch (e) {
    threw = true;
  }
  check("rs_ext: verify rejects corrupted parity", threw);

  let threwLen = false;
  try {
    r.verify([1, 2, 3]);
  } catch (e) {
    threwLen = true;
  }
  check("rs_ext: verify rejects bad length", threwLen);

  let threwEnc = false;
  try {
    r.encode([1, 2, 3]);
  } catch (e) {
    threwEnc = true;
  }
  check("rs_ext: encode rejects bad length", threwEnc);

  const goodMsg = new Array(223).fill(1);
  const badMsg = new Array(5).fill(1);
  const { codewords, result } = r.encodeBatch([goodMsg, badMsg, goodMsg]);
  check("rs_ext: batch mixed", codewords.length === 2 && !result.ok && result.errors.length === 1
    && result.errors[0].includes("bad length"));

  const { codewords: cwAll, result: resAll } = r.encodeBatch([goodMsg, goodMsg]);
  check("rs_ext: batch all good", resAll.ok === true && codewords.length === 2);

  const br = new RsBatchResult();
  check("rs_ext: batch result shape", br.ok === true && br.errors.length === 0);
}

// ---------- transfer_resume_ext.js ----------
{
  const t = new TransferResumeExt(5);
  const p = t.resumePoint();
  check("resume_ext: starts at chunk zero", p instanceof ResumePointExt && p.nextChunk === 0
    && p.totalChunks === 5 && p.delivered === 0 && t.status() === "in_progress");

  const t2 = new TransferResumeExt(5);
  t2.markDelivered(0);
  check("resume_ext: advances with in-order delivery", t2.resumePoint().nextChunk === 1);
  t2.markDelivered(1);
  check("resume_ext: delivered count", t2.resumePoint().delivered === 2 && t2.remaining() === 3);

  const t3 = new TransferResumeExt(5);
  t3.markDelivered(2);
  check("resume_ext: out-of-order ignored", t3.resumePoint().nextChunk === 0 && t3.resumePoint().delivered === 0);

  const t4 = new TransferResumeExt(3);
  for (let i = 0; i < 3; i++) t4.markDelivered(i);
  check("resume_ext: completes when all delivered", t4.isComplete() && t4.remaining() === 0 && t4.status() === "complete");

  const t5 = new TransferResumeExt(0);
  check("resume_ext: zero total is empty", t5.status() === "empty" && t5.isComplete() && t5.remaining() === 0);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}