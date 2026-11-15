// RX: open the camera, decode the sender's looping QR frames, reassemble
// chunks by their frame number (each QR header carries chunkIdx +
// totalChunks, so the receiver always knows exactly which frames it is
// missing and just waits for the loop to bring them around again),
// auto-unzip zipped payloads, download the file, and flash a Done QR
// until the sender stops.
//
// Hardening: chunk metadata is validated before storage (bounds, payload
// size, chunk-count cap), a foreign transfer can only take over via a
// Ping — never via a mid-stream Chunk — and a finished transfer still
// accepts a fresh Ping so the same pair can send a second file without
// going back to the landing screen.

import * as core from "./core.js";
import { openCamera, makeEmitter, fitCanvas, keepScreenAwake } from "./light.js";
import { qrEncode } from "./qr.js";
import { qrDecodeVideo } from "./qrdecode.js";

const PONG_DWELL_MS = 400;
const DONE_FLASH_MS = 30000; // stop flashing Done after this; the sender has seen it or is gone

export function mountRx(root, fflate, onBack) {
  const video = root.querySelector("#rx-video");
  const camCanvas = root.querySelector("#rx-canvas");
  const emitCanvas = root.querySelector("#rx-emit");
  const statusEl = root.querySelector("#rx-status");
  const progressFill = root.querySelector("#rx-progress");
  const lockBtn = root.querySelector("#rx-lock");
  const backBtn = root.querySelector("#rx-back");

  for (const cv of [camCanvas, emitCanvas]) {
    fitCanvas(cv);
    new ResizeObserver(() => fitCanvas(cv)).observe(cv.parentElement);
  }

  const emitter = makeEmitter(emitCanvas);
  // Idle screen: dark (all blocks 0) — sleek on OLED, and never decodes
  // as a QR (a single solid blob has fill ratio ~1 and is excluded by the
  // finder filter).
  const idleBlocks = () => new Uint8Array(core.GRID_W * core.GRID_H).fill(0);
  const emitter = makeEmitter(emitCanvas);
  emitter.setBlocks(idleBlocks());
  let pongTimer = null;
  let doneTimer = null;
  let releaseWake = null;

  let session = null;
  let sessionId = -1;
  let chunks = [];
  let totalChunks = 0;
  let totalBytes = 0;
  let finished = false;
  let camHandle = null;

  // Decode throttle: on slow devices a full decode (finder flood-fill +
  // RS) can take longer than a frame period; decoding every camera frame
  // then backs up the callback loop and effectively drops MORE frames.
  // Sample the frame rate down instead — the loop protocol brings back
  // anything this skips.
  let decodeStride = 1;
  let frameTick = 0;
  let failStreak = 0;
  let showedRes = false;
  openCamera(video, (v) => {
      drawCam(v, camCanvas);
      frameTick++;
      if (frameTick % decodeStride !== 0) return;
      const t0 = performance.now();
      const dec = qrDecodeVideo(v);
      const ms = performance.now() - t0;
      decodeStride = ms > 66 ? 3 : ms > 33 ? 2 : 1;
      if (dec.ok) {
        failStreak = 0;
        route(dec.frame);
      } else {
        failStreak++;
        if (!session) {
          if (!showedRes) {
            showedRes = true;
            statusEl.textContent =
              "camera " + v.videoWidth + "×" + v.videoHeight + " — fill the square with the sender's QR";
          } else if (failStreak === 45) {
            // ~1.5 s without a decode: give the user a specific, actionable hint
            statusEl.textContent =
              dec.error === "FindersNotFound"
                ? "no QR in view — move closer and fill the square"
                : "I can see a code but can't read it — hold steadier, get closer, or tap lock exposure";
          }
        }
      }
    })
    .then(async (h) => {
      camHandle = h;
      releaseWake = await keepScreenAwake();
    })
    .catch((e) => {
      statusEl.textContent =
        "camera: " + e.message +
        (typeof window !== "undefined" && !window.isSecureContext
          ? " — camera needs https:// or localhost"
          : " — allow camera access and try again");
    });

  function missingList() {
    const out = [];
    for (let i = 0; i < totalChunks; i++) if (!chunks[i]) out.push(i);
    return out;
  }

  function drawCam(v, canvas) {
    const ctx = canvas.getContext("2d");
    if (!ctx || !v.videoWidth || canvas.width === 0) return;
    const cw = canvas.width;
    const ch = canvas.height;
    const vw = v.videoWidth;
    const vh = v.videoHeight;
    // cover-crop: fill the viewfinder and crop the overflow, like a
    // camera app — the QR box guides framing instead of letterboxing
    const scale = Math.max(cw / vw, ch / vh);
    const dw = vw * scale;
    const dh = vh * scale;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, cw, ch);
    ctx.drawImage(v, (cw - dw) / 2, (ch - dh) / 2, dw, dh);
  }

  function flashBriefly(frame) {
    if (finished) return;
    emitter.setQr(qrEncode(frame));
    if (pongTimer) clearTimeout(pongTimer);
    pongTimer = setTimeout(() => {
      // back to the plain idle screen so the loop keeps decoding cleanly
      emitter.setBlocks(idleBlocks());
    }, PONG_DWELL_MS);
  }

  function ensureSession(sid, total) {
    if (pongTimer) clearTimeout(pongTimer);
    if (doneTimer) clearTimeout(doneTimer);
    sessionId = sid;
    totalChunks = total;
    chunks = new Array(total).fill(null);
    totalBytes = 0;
    finished = false;
    session = core.newFrame("Pong", sid, 0, total, null);
    statusEl.textContent = "sender detected — " + total + " frames, collecting…";
  }

  function route(f) {
    if (f.kind === "Ping") {
      // A Ping is the only way a foreign/new transfer may take over —
      // even after a finished transfer (send a second file without
      // going back to the landing screen).
      if (f.totalChunks < 1 || f.totalChunks > core.maxChunkCount()) return; // corrupt/allocation guard
      const sameSession = session && f.sessionId === sessionId && f.totalChunks === totalChunks;
      if (!sameSession || finished) ensureSession(f.sessionId, f.totalChunks);
      flashBriefly(core.newFrame("Pong", f.sessionId, 0, f.totalChunks, null));
      return;
    }
    if (finished) return;
    if (f.kind !== "Chunk") return;
    if (!core.validChunkMeta(f.chunkIdx, f.totalChunks, f.payload.length)) return; // corrupt or hostile frame
    if (session) {
      // Mid-stream chunks from a different session or with a different
      // chunk count are ignored — only a Ping switches transfers.
      if (f.sessionId !== sessionId || f.totalChunks !== totalChunks) return;
    } else {
      // joined mid-pass: the first frame we ever catch is a Chunk
      ensureSession(f.sessionId, f.totalChunks);
    }

    if (!chunks[f.chunkIdx]) {
      chunks[f.chunkIdx] = f.payload;
      totalBytes += f.payload.length;
    }
    const got = chunks.filter(Boolean).length;
    if (totalChunks > 0) {
      progressFill.style.width = (got / totalChunks) * 100 + "%";
    }
    if (got < totalChunks) {
      // frame numbers make the losses visible: list up to 12 missing
      const missing = missingList();
      const shown = missing.slice(0, 12).map((i) => i + 1).join(", ");
      statusEl.textContent =
        got + "/" + totalChunks + " frames — waiting for " +
        (missing.length > 12 ? shown + ", …" : shown) +
        " to come around again";
    } else {
      finish();
    }
  }

  function finish() {
    if (finished) return;
    finished = true;
    statusEl.textContent = "assembling…";
    const out = new Uint8Array(totalBytes);
    let off = 0;
    for (let i = 0; i < totalChunks; i++) {
      const p = chunks[i];
      if (p) {
        out.set(p, off);
        off += p.length;
      }
    }
    if (isZip(out)) {
      const files = fflate.unzipSync(out);
      const name = Object.keys(files)[0];
      downloadBlob(new Blob([files[name]], { type: "application/octet-stream" }), name);
      statusEl.textContent = "saved " + name;
    } else {
      downloadBlob(new Blob([out], { type: "application/octet-stream" }), "pixfer-file.bin");
      statusEl.textContent = "saved";
    }
    // Tell the sender to stop: keep flashing Done for a while so a
    // shake/blur cannot make it miss the signal.
    if (pongTimer) clearTimeout(pongTimer);
    if (doneTimer) clearTimeout(doneTimer);
    emitter.setQr(qrEncode(core.newFrame("Done", sessionId, 0, totalChunks, null)));
    if (doneTimer) clearTimeout(doneTimer);
    doneTimer = setTimeout(() => {
      emitter.setBlocks(idleBlocks());
      if (session) statusEl.textContent = "saved — you can go back, or wait for another transfer";
    }, DONE_FLASH_MS);
  }

  function isZip(b) {
    return b.length > 4 && b[0] === 0x50 && b[1] === 0x4b && b[2] === 0x03 && b[3] === 0x04;
  }

  function downloadBlob(blob, name) {
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  lockBtn.addEventListener("click", async () => {
    if (!camHandle) {
      lockBtn.textContent = "no camera";
      return;
    }
    const track = camHandle.stream.getVideoTracks()[0];
    if (!track) return;
    lockBtn.textContent = "…";
    try {
      const caps = await track.getCapabilities();
      if (caps.exposureTime && caps.exposureMode) {
        await track.applyConstraints({
          advanced: [{ exposureMode: "manual", exposureTime: caps.exposureTime.min }],
        });
        lockBtn.textContent = "exposure locked";
      } else {
        lockBtn.textContent = "lock unavailable";
      }
    } catch (e) {
      lockBtn.textContent = "lock unavailable";
    }
  });

  backBtn.addEventListener("click", () => {
    stop();
    if (onBack) onBack();
  });

  function stop() {
    emitter.stop();
    if (pongTimer) clearTimeout(pongTimer);
    if (doneTimer) clearTimeout(doneTimer);
    if (releaseWake) {
      releaseWake();
      releaseWake = null;
    }
    if (camHandle) {
      camHandle.stop();
      camHandle = null;
    }
  }

  return { stop };
}
