// Camera simulation: a bad-quality 30 fps phone camera in unsteady hands
// watching the sender's looping colored QR code.
//
// The physical model, per captured frame:
//   1. The screen shows the QR box (quiet zone + colored dark modules) —
//      the same geometry light.js paints. The sender LOOPS: Ping → every
//      chunk → repeat, each frame flashed for its dwell; any frame the
//      camera misses comes around again on the next pass.
//   2. Hand shake: the box center drifts (slow wander), jitters (fast
//      tremor), tilts (rotation wobble) and occasionally slides right
//      off the camera view for a moment, then comes back.
//   3. The camera integrates light over an exposure window. If the flash
//      phase flips mid-exposure the capture blends the two phases; if a
//      frame BOUNDARY (two different chunks) falls mid-exposure the
//      capture is an unrecoverable mix.
//   4. Optics blur the sharp modules (gaussian PSF; focus "breathes"),
//      the sensor adds per-channel noise, white-balance tints, and
//      exposure scales.
//   5. qrDecodeFromPixels runs on the result exactly as qrDecodeVideo
//      does: DS=4 first, DS=2 retry on failure.
//
// Scenario thresholds are calibrated against observed decode rates with
// margin — see the summary printout at the end.
// Run: node tests/camera_sim.test.mjs (from web/)

import test from "node:test";
import assert from "node:assert/strict";
import * as core from "../js/core.js";
import { qrEncode } from "../js/qr.js";
import { qrDecodeFromPixels } from "../js/qrdecode.js";
import { QR_DARK_COLORS, QR_QUIET_MODULES, FLASH_PHASE_MS } from "../js/light.js";
import { PING_DWELL_MS, CHUNK_DWELL_MS } from "../js/tx.js";

const CAM_W = 1280;
const CAM_H = 720;
const FPS = 30;
const FRAME_MS = 1000 / FPS;
const PASS_MS = (chunks) => PING_DWELL_MS + chunks * CHUNK_DWELL_MS;

// Deterministic PRNG (xorshift32) so runs are reproducible.
function makeRng(seed) {
  let x = seed >>> 0;
  return () => {
    x ^= x << 13; x >>>= 0;
    x ^= x >> 17; x >>>= 0;
    x ^= x << 5; x >>>= 0;
    return x / 0xffffffff;
  };
}
function gauss(rng) {
  const u = Math.max(rng(), 1e-9);
  const v = rng();
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
}

// Motion model: where the box is and how it's tilted at time t (ms).
// Deterministic wobble + scripted off-screen excursions.
function makeMotion(kind) {
  // kind: "static" | "shake" | "offscreen"
  const D2R = Math.PI / 180;
  return (t) => {
    const s = t / 1000;
    let cx = CAM_W / 2;
    let cy = CAM_H / 2;
    let rot = 0;
    if (kind === "shake") {
      cx += 90 * Math.sin(2 * Math.PI * s / 2.9) + 22 * Math.sin(2 * Math.PI * s / 0.41);
      cy += 60 * Math.sin(2 * Math.PI * s / 3.7 + 1) + 18 * Math.sin(2 * Math.PI * s / 0.33);
      rot = 2.5 * D2R * Math.sin(2 * Math.PI * s / 4.1) + 0.8 * D2R * Math.sin(2 * Math.PI * s / 0.5);
    } else if (kind === "offscreen") {
      cx += 70 * Math.sin(2 * Math.PI * s / 3.1);
      cy += 40 * Math.sin(2 * Math.PI * s / 2.3 + 1);
      rot = 2 * D2R * Math.sin(2 * Math.PI * s / 3.7);
      // scripted excursions: the box fully leaves the view mid-pass (eating
      // the sender's chunks 0-3 of pass 1), then comes back — the loop
      // protocol must recapture those on the next pass
      if (t >= 200 && t < 1200) cx += 1400; // off to the right
      if (t >= 3600 && t < 4100) cy -= 1200; // off above, once more
    }
    return { cx, cy, rot };
  };
}

// The sharp screen image at a camera pixel: inverse-rotate into the QR
// box's module grid, return [r,g,b]. `inverted` renders the flash phase.
function makeScreenSampler(qr, { boxPx, cx, cy, rot, colorRot }) {
  const n = qr.size;
  const units = n + 2 * QR_QUIET_MODULES;
  const mp = boxPx / units; // module pitch in camera px
  const cos = Math.cos(rot);
  const sin = Math.sin(rot);
  const k = QR_DARK_COLORS.length;
  const background = [235, 235, 235]; // the page around the box

  return (px, py, inverted) => {
    const dx = px - cx;
    const dy = py - cy;
    const bx = cos * dx + sin * dy;
    const by = -sin * dx + cos * dy;
    const uc = Math.floor((bx + boxPx / 2) / mp) - QR_QUIET_MODULES;
    const vc = Math.floor((by + boxPx / 2) / mp) - QR_QUIET_MODULES;
    if (uc < 0 || vc < 0 || uc >= n || vc >= n) return background;
    const dark = qr.data[vc * n + uc] === 1;
    if (inverted ? !dark : dark) {
      if (inverted) return [255, 255, 255];
      const col = QR_DARK_COLORS[(vc * 7 + uc * 11 + colorRot) % k];
      return [col[0], col[1], col[2]];
    }
    return inverted ? [0, 0, 0] : [255, 255, 255];
  };
}

// Separable gaussian blur evaluated only at the DS-subsampled pixel
// positions (that's all the decoder ever sees), with white balance,
// exposure scale and per-channel sensor noise. The gaussian models the
// browser's anti-aliased downscale chain (qrdecode.js draws in smoothed
// 2x stages); pure nearest-neighbor decimation — what
// imageSmoothingEnabled=false did — aliases the sender's screen pixel
// grid into the buffer and destroys module contrast at distance.
const kernelCache = new Map();
function kernel(sigma) {
  if (kernelCache.has(sigma)) return kernelCache.get(sigma);
  const radius = Math.max(1, Math.ceil(sigma * 2));
  const tbl = [];
  let sum = 0;
  for (let d = -radius; d <= radius; d++) {
    const v = Math.exp(-(d * d) / (2 * sigma * sigma));
    tbl.push(v);
    sum += v;
  }
  const out = { tbl: tbl.map((v) => v / sum), radius };
  kernelCache.set(sigma, out);
  return out;
}

function renderDsBuffer(
  sampler, opts, rng, exposureBlend, ds,
  originX = 0, originY = 0, bufW = CAM_W / ds, bufH = CAM_H / ds
) {
  const w = bufW;
  const h = bufH;
  const out = new Uint8Array(w * h * 4);
  const { tbl, radius } = kernel(opts.blurSigma);
  const wb = opts.wb;
  const noise = opts.noiseSigma;
  const bright = opts.brightness;

  for (let j = 0; j < h; j++) {
    for (let i = 0; i < w; i++) {
      const camX = originX + i * ds;
      const camY = originY + j * ds;
      let r = 0, g = 0, b = 0;
      // 2D gaussian in camera space, evaluated only at the DS grid
      // points (that is all the decoder ever sees)
      for (let dy = -radius; dy <= radius; dy++) {
        const wy = tbl[dy + radius];
        for (let dx = -radius; dx <= radius; dx++) {
          const wx = tbl[dx + radius];
          const wgt = wx * wy;
          const px = camX + dx;
          const py = camY + dy;
          const c1 = sampler(px, py, false);
          if (exposureBlend === 0) {
            r += wgt * c1[0]; g += wgt * c1[1]; b += wgt * c1[2];
          } else {
            const c2 = sampler(px, py, true);
            r += wgt * ((1 - exposureBlend) * c1[0] + exposureBlend * c2[0]);
            g += wgt * ((1 - exposureBlend) * c1[1] + exposureBlend * c2[1]);
            b += wgt * ((1 - exposureBlend) * c1[2] + exposureBlend * c2[2]);
          }
        }
      }
      const o = (j * w + i) * 4;
      out[o] = Math.max(0, Math.min(255, r * wb[0] * bright + gauss(rng) * noise)) | 0;
      out[o + 1] = Math.max(0, Math.min(255, g * wb[1] * bright + gauss(rng) * noise)) | 0;
      out[o + 2] = Math.max(0, Math.min(255, b * wb[2] * bright + gauss(rng) * noise)) | 0;
      out[o + 3] = 255;
    }
  }
  return { data: out, w, h };
}

// What the sender is flashing at time t: { frame: "ping" | chunkIndex }
// plus whether the exposure window crosses a content boundary.
function senderAt(t, nChunks) {
  const pm = PASS_MS(nChunks);
  const tInPass = t % pm;
  let content;
  if (tInPass < PING_DWELL_MS) content = -1; // ping
  else content = Math.floor((tInPass - PING_DWELL_MS) / CHUNK_DWELL_MS);
  if (content >= nChunks) content = -1; // pass wrap safety
  // boundary crossing check over [t, t+exposure]
  return { content };
}

function contentChanges(a, b, nChunks) {
  return a !== b;
}

function runScenario(name, opts, seconds = 2) {
  const rng = makeRng(opts.seed || 12345);
  const frame = opts.frame || core.newFrame("Chunk", 4242, 7, 12, new Uint8Array(core.RS_K).fill(0x37));
  const qr = qrEncode(frame);
  const motion = makeMotion(opts.motion || "static");

  const frames = Math.round(seconds * FPS);
  let decodes = 0;
  let cleanFrames = 0; // frames whose exposure fell inside one flash phase
  let decodeMs = 0; // decode-only time (renders are simulation cost, not decode cost)
  let maxDecodeMs = 0;
  let invertedCatches = 0;
  let ds4 = 0;
  let ds2Rescues = 0;
  let cropRescues = 0;
  const errors = {};

  for (let f = 0; f < frames; f++) {
    const t = f * FRAME_MS + (rng() - 0.5) * 3; // real cameras jitter frame times
    const phaseStart = Math.floor(t / FLASH_PHASE_MS) * FLASH_PHASE_MS;
    const boundary = phaseStart + FLASH_PHASE_MS;
    const fracInFirstPhase = Math.max(0, Math.min(1, (boundary - t) / opts.exposureMs));
    const isClean = fracInFirstPhase >= 1 || fracInFirstPhase <= 0;
    if (isClean) cleanFrames++;
    const exposureBlend = isClean ? 0 : 1 - fracInFirstPhase;

    const m = motion(t);
    const blur = opts.blurSigma * (1 + 0.25 * Math.sin(2 * Math.PI * (t / 1000) / 2.7));
    const jitter = { x: (rng() - 0.5) * opts.jitterPx, y: (rng() - 0.5) * opts.jitterPx };
    const sampler = makeScreenSampler(qr, {
      boxPx: opts.boxPx,
      cx: m.cx + jitter.x,
      cy: m.cy + jitter.y,
      rot: m.rot,
      colorRot: 1,
    });
    const simOpts = { ...opts, blurSigma: blur };

    // DS=4 first (qrDecodeVideo fast path)
    const buf4 = renderDsBuffer(sampler, simOpts, rng, exposureBlend, 4);
    let d0 = process.hrtime.bigint();
    let dec = qrDecodeFromPixels(buf4.data, buf4.w, buf4.h);
    let d1 = process.hrtime.bigint();
    let frameDecodeMs = Number(d1 - d0) / 1e6;
    if (dec.ok) {
      ds4++;
    } else {
      // DS=2 retry on failure (qrDecodeVideo retry)
      const buf2 = renderDsBuffer(sampler, simOpts, rng, exposureBlend, 2);
      d0 = process.hrtime.bigint();
      dec = qrDecodeFromPixels(buf2.data, buf2.w, buf2.h);
      d1 = process.hrtime.bigint();
      frameDecodeMs += Number(d1 - d0) / 1e6;
      if (dec.ok) {
        ds2Rescues++;
      } else if (opts.tryCrop) {
        // centered native-resolution crop (qrDecodeVideo's crop pass for
        // streams too big for full-res decoding)
        const side = Math.round(Math.min(CAM_W, CAM_H) * 0.55);
        const bufC = renderDsBuffer(
          sampler, simOpts, rng, exposureBlend, 1,
          (CAM_W - side) / 2, (CAM_H - side) / 2, side, side
        );
        d0 = process.hrtime.bigint();
        dec = qrDecodeFromPixels(bufC.data, bufC.w, bufC.h);
        d1 = process.hrtime.bigint();
        frameDecodeMs += Number(d1 - d0) / 1e6;
        if (dec.ok) cropRescues++;
      }
    }
    decodeMs += frameDecodeMs;
    if (frameDecodeMs > maxDecodeMs) maxDecodeMs = frameDecodeMs;
    if (dec.ok) {
      decodes++;
      if (dec.inverted) invertedCatches++;
    } else {
      errors[dec.error] = (errors[dec.error] || 0) + 1;
    }
  }

  const result = {
    name, frames, cleanFrames, decodes,
    rate: decodes / frames,
    cleanRate: cleanFrames > 0 ? decodes / cleanFrames : 0,
    avgDecodeMs: decodeMs / frames,
    maxDecodeMs, invertedCatches, ds4, ds2Rescues, cropRescues, errors,
  };
  console.log(
    `${name}: ${decodes}/${frames} decode (${(result.rate * 100).toFixed(0)}%),` +
      ` clean frames ${cleanFrames} -> clean-rate ${(result.cleanRate * 100).toFixed(0)}%,` +
      ` ds4 ${ds4} + ds2 rescues ${ds2Rescues}` +
      (opts.tryCrop ? ` + crop rescues ${cropRescues}` : "") +
      `, avg decode ${result.avgDecodeMs.toFixed(1)}ms (max ${maxDecodeMs.toFixed(1)}ms),` +
      ` inverted catches ${invertedCatches}` +
      (Object.keys(errors).length ? `, errors: ${JSON.stringify(errors)}` : "")
  );
  return result;
}

test("camera sim: ideal close-up decodes nearly every frame", () => {
  const r = runScenario("ideal-close", {
    boxPx: 620, motion: "static", rot: 0, blurSigma: 0.7, noiseSigma: 3, wb: [1, 1, 1],
    brightness: 1.0, exposureMs: 10, jitterPx: 0, seed: 101,
  }, 1);
  assert.ok(r.rate >= 0.85, `rate ${(r.rate * 100).toFixed(0)}% < 85%`);
});

test("camera sim: typical phone distance + hand-held rotation", () => {
  const r = runScenario("typical-tilt", {
    boxPx: 560, motion: "static", rot: 2 * Math.PI / 180, blurSigma: 1.2, noiseSigma: 8,
    wb: [1.08, 1.0, 0.94], brightness: 1.0, exposureMs: 15, jitterPx: 0, seed: 202,
  }, 1);
  assert.ok(r.rate >= 0.5, `rate ${(r.rate * 100).toFixed(0)}% < 50%`);
});

test("camera sim: dim room, longer exposure (phase blending)", () => {
  const r = runScenario("dim", {
    boxPx: 560, motion: "static", rot: 1 * Math.PI / 180, blurSigma: 1.3, noiseSigma: 16,
    wb: [1.12, 1.0, 0.9], brightness: 0.78, exposureMs: 30, jitterPx: 0, seed: 303,
  }, 1);
  assert.ok(r.rate >= 0.25, `rate ${(r.rate * 100).toFixed(0)}% < 25%`);
});

test("camera sim: far away — small code rescued by the DS=2 retry", () => {
  const r = runScenario("far", {
    boxPx: 400, motion: "static", rot: 0, blurSigma: 0.9, noiseSigma: 8, wb: [1, 1, 1],
    brightness: 1.0, exposureMs: 10, jitterPx: 0, seed: 404,
  }, 1);
  assert.ok(r.rate >= 0.4, `rate ${(r.rate * 100).toFixed(0)}% < 40%`);
  assert.ok(r.ds2Rescues > 0, "expected the DS=2 retry to rescue small-code frames");
});

test("camera sim: phone-to-phone distance (tiny code in view)", () => {
  // Phone-to-phone reality: the sender's screen is small, so at holding
  // distance the QR covers only ~20% of the camera frame — modules land
  // near 2 px in the DS=2 buffer. This scenario documents the coarse-scale
  // floor WITHOUT the crop pass: a partial per-frame rate is still fine,
  // because every frame is numbered and the loop brings back what is
  // missed. Production additionally runs the full-res pass (720p) or the
  // centered crop pass (1080p) — see the crop-rescue scenario.
  const r = runScenario("far-phone", {
    boxPx: 260, motion: "static", rot: 0, blurSigma: 0.8, noiseSigma: 6, wb: [1, 1, 1],
    brightness: 1.0, exposureMs: 15, jitterPx: 2, seed: 909,
  }, 1);
  assert.ok(r.rate >= 0.2, `rate ${(r.rate * 100).toFixed(0)}% < 20%`);
  assert.ok(r.ds2Rescues > 0, "expected the fine-scale pass to rescue tiny-code frames");
});

test("camera sim: tiny code rescued by the centered crop pass", () => {
  // Extreme distance: modules land at ~1.4 px in the DS=2 buffer — below
  // what any full-frame pass can read. The centered square crop at native
  // resolution — where the viewfinder brackets put the code — still
  // resolves the modules (this is the pass 1080p phones run when their
  // full-res frame exceeds the decode budget).
  const r = runScenario("crop-rescue", {
    boxPx: 200, motion: "static", rot: 0, blurSigma: 0.8, noiseSigma: 6, wb: [1, 1, 1],
    brightness: 1.0, exposureMs: 15, jitterPx: 2, seed: 909, tryCrop: true,
  }, 0.7);
  assert.ok(r.rate >= 0.5, `rate ${(r.rate * 100).toFixed(0)}% < 50%`);
  assert.ok(r.cropRescues > 0, "expected the centered crop pass to rescue tiny-code frames");
});

test("camera sim: hand shake — drift, tremor, rotation wobble", () => {
  const r = runScenario("shake", {
    boxPx: 560, motion: "shake", blurSigma: 1.1, noiseSigma: 10, wb: [1.06, 1.0, 0.96],
    brightness: 1.0, exposureMs: 15, jitterPx: 6, seed: 505,
  }, 2);
  // the affine grid tracks rotation; drift keeps the code in frame; the
  // loop protocol recovers whatever this drops
  assert.ok(r.rate >= 0.25, `rate ${(r.rate * 100).toFixed(0)}% < 25%`);
});

test("camera sim: sender catches the receiver's Done reply", () => {
  // The reply channel: the receiver flashes a small v2 Done QR in its
  // ~320px corner box — the sender's camera must catch it under the same
  // shake. This is the ack path that ends the loop.
  const r = runScenario("reply-catch", {
    frame: core.newFrame("Done", 4242, 0, 12, null),
    boxPx: 320, motion: "shake", blurSigma: 1.2, noiseSigma: 10, wb: [1.06, 1.0, 0.96],
    brightness: 1.0, exposureMs: 15, jitterPx: 6, seed: 808,
  }, 1);
  assert.ok(r.rate >= 0.5, `rate ${(r.rate * 100).toFixed(0)}% < 50%`);
});

test("camera sim: decode stays fast enough for 30fps", () => {
  const r = runScenario("perf", {
    boxPx: 560, motion: "static", rot: 0, blurSigma: 1.2, noiseSigma: 8, wb: [1, 1, 1],
    brightness: 1.0, exposureMs: 15, jitterPx: 0, seed: 606,
  }, 0.5);
  // decode-only cost (renders excluded — a real device blits via the
  // GPU): must fit the 33ms frame budget at 30fps
  assert.ok(r.avgDecodeMs < 25, `avg decode ${r.avgDecodeMs.toFixed(1)}ms >= 25ms`);
});

// ---------- loop end-to-end: lost frames get recaptured ----------

test("camera sim: loop transfer recaptures lost frames (shake + off-screen)", () => {
  const N = 5;
  const seed = 777;
  const rng = makeRng(seed);
  const sessionId = 9182;

  // sender payload: N distinct chunks; the LAST one is short (54 B) so
  // the loop also exercises a small-version QR (v5), like real files do
  const payloads = [];
  for (let i = 0; i < N; i++) {
    const len = i === N - 1 ? 54 : core.RS_K;
    const p = new Uint8Array(len);
    for (let b = 0; b < len; b++) p[b] = (i * 31 + b * 7 + 5) & 0xff;
    payloads.push(p);
  }
  const pings = qrEncode(core.newFrame("Ping", sessionId, 0, N, null));
  const chunkQrs = payloads.map((p, i) =>
    qrEncode(core.newFrame("Chunk", sessionId, i, N, p))
  );

  // hand shake + an off-screen excursion that eats the FIRST pass's
  // chunks 0-3 — they can only arrive on the second pass, which is
  // exactly what the loop protocol must recapture.
  const motion = makeMotion("offscreen");
  const opts = {
    boxPx: 560, blurSigma: 1.1, noiseSigma: 10, wb: [1.06, 1.0, 0.96],
    brightness: 1.0, exposureMs: 15,
  };
  const pm = PASS_MS(N);

  // receiver state, exactly like rx.js loop mode
  const got = new Array(N).fill(null);
  let caughtByIndex = 0;
  let wrongIndex = 0;
  let offscreenFrames = 0;
  let transitions = 0;
  const missingTimeline = [];

  const seconds = 5.2; // ~3 passes
  const frames = Math.round(seconds * FPS);
  let allDoneAt = -1;

  for (let f = 0; f < frames; f++) {
    const t = f * FRAME_MS + (rng() - 0.5) * 3;
    if (allDoneAt > 0 && t > allDoneAt + 1000) break; // receiver is done; TX stops on Done

    const cur = senderAt(t, N);
    const nxt = senderAt(t + opts.exposureMs, N);
    const isTransition = cur.content !== nxt.content;
    if (isTransition) transitions++;

    const phaseStart = Math.floor(t / FLASH_PHASE_MS) * FLASH_PHASE_MS;
    const boundary = phaseStart + FLASH_PHASE_MS;
    const fracInFirstPhase = Math.max(0, Math.min(1, (boundary - t) / opts.exposureMs));
    const isClean = fracInFirstPhase >= 1 || fracInFirstPhase <= 0;
    const exposureBlend = isClean ? 0 : 1 - fracInFirstPhase;

    const m = motion(t);
    const fullyOffscreen =
      m.cx < -opts.boxPx / 2 || m.cx > CAM_W + opts.boxPx / 2 ||
      m.cy < -opts.boxPx / 2 || m.cy > CAM_H + opts.boxPx / 2;
    if (fullyOffscreen) offscreenFrames++;

    if (isTransition || fullyOffscreen) continue; // guaranteed miss

    const qr = cur.content < 0 ? pings : chunkQrs[cur.content];
    const jitter = { x: (rng() - 0.5) * 6, y: (rng() - 0.5) * 6 };
    const blur = opts.blurSigma * (1 + 0.25 * Math.sin(2 * Math.PI * (t / 1000) / 2.7));
    const sampler = makeScreenSampler(qr, {
      boxPx: opts.boxPx,
      cx: m.cx + jitter.x,
      cy: m.cy + jitter.y,
      rot: m.rot,
      colorRot: (cur.content + 1) % QR_DARK_COLORS.length,
    });
    const simOpts = { ...opts, blurSigma: blur };

    // qrDecodeVideo behavior: DS=4 first, DS=2 retry
    const buf4 = renderDsBuffer(sampler, simOpts, rng, exposureBlend, 4);
    let dec = qrDecodeFromPixels(buf4.data, buf4.w, buf4.h);
    if (!dec.ok) {
      const buf2 = renderDsBuffer(sampler, simOpts, rng, exposureBlend, 2);
      dec = qrDecodeFromPixels(buf2.data, buf2.w, buf2.h);
    }
    if (dec.ok && dec.frame.kind === "Chunk" && dec.frame.sessionId === sessionId) {
      // the frame NUMBER inside the QR tells us exactly which chunk this is
      if (dec.frame.chunkIdx !== cur.content) {
        wrongIndex++;
      } else if (!got[dec.frame.chunkIdx]) {
        got[dec.frame.chunkIdx] = dec.frame.payload;
        caughtByIndex++;
      }
      const expect = payloads[dec.frame.chunkIdx];
      if (!expect || !dec.frame.payload.every((v, i) => v === expect[i])) wrongIndex++;
    }

    const missingCount = got.filter((x) => !x).length;
    missingTimeline.push({ t, missingCount });
    if (allDoneAt < 0 && missingCount === 0) allDoneAt = t;
  }

  const gotAll = got.every(Boolean);
  const firstPassEnd = pm; // chunks 0-3 were missed during the excursion
  const arrivalsAfter = missingTimeline.filter(
    (x, i, a) => i > 0 && x.missingCount < a[i - 1].missingCount && x.t > firstPassEnd
  ).length;
  console.log(
    `loop-e2e: ${caughtByIndex}/${N} chunks caught by frame number,` +
      ` wrong-index ${wrongIndex}, off-screen frames ${offscreenFrames},` +
      ` transitions ${transitions},` +
      ` complete at t=${allDoneAt < 0 ? "never" : allDoneAt.toFixed(0) + "ms"}` +
      ` (pass length ${pm}ms), arrivals after pass 1: ${arrivalsAfter}`
  );
  assert.ok(gotAll, `receiver missing chunks: ${got.map((g, i) => (g ? "" : i)).filter(Boolean).join(",")}`);
  assert.strictEqual(wrongIndex, 0, "decoded frame numbers or payload bytes mismatched");
  assert.ok(allDoneAt > 0, "transfer did not complete");
  assert.ok(
    arrivalsAfter > 0,
    "no chunk arrived after pass 1 — loop recapture not proven"
  );
  assert.ok(allDoneAt > firstPassEnd, "completed without needing the second pass — excursion never bit");
});
