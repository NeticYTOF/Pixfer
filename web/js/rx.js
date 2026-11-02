// RX: open the camera, sample and decode the peer's screen, reassemble
// chunks, auto-unzip zipped payloads, and download the file. This side
// emits Pong/Ack/Nak frames on its own screen via a small canvas.

import * as core from "./core.js";
import { openCamera, sampleBlocks, camOpts, makeEmitter, fitCanvas } from "./light.js";
import { detectAlignment } from "./align.js";

const ALIGN_EVERY_N_FRAMES = 15;
const ALIGN_MIN_QUALITY = 0.5;

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
  // Idle screen: full-white grid (all blocks 7).
  emitter.setBlocks(new Uint8Array(core.GRID_W * core.GRID_H).fill(7));

  let session = null;
  let chunks = [];
  let totalChunks = 0;
  let totalBytes = 0;
  let finished = false;
  let camHandle = null;
  let frameCount = 0;
  let aligned = null; // {pitchX, pitchY, offX, offY, quality}

  openCamera(video, (v) => {
    drawCam(v, camCanvas);
    // Auto-calibration: detect the 3 finder squares to recover the true on-
    // screen pitch/offset instead of trusting the slider guess. Throttle the
    // detection (every 15 frames) and lock the first result that clears the
    // quality bar.
    frameCount++;
    if (!aligned && (frameCount % ALIGN_EVERY_N_FRAMES === 0)) {
      const al = detectAlignment(v);
      if (al.found && al.quality >= ALIGN_MIN_QUALITY) aligned = al;
    }
    const opts = samplerOpts();
    const dec = core.decodeBlocks(sampleBlocks(v, opts));
    if (dec.ok) {
      route(dec.frame, dec.ok);
    } else if (session && totalChunks > 0) {
      // A captured frame failed to decode (loss/corruption past the RS
      // correction bound). NAK the lowest chunk we have not received yet,
      // so the sender retransmits it — that is how lost packets are recovered.
      session.onChunkErr(lowestMissing());
      const nf = session.nextFrame();
      if (nf) emitter.setBlocks(core.renderBlocks(nf));
    }
  })
    .then((h) => {
      camHandle = h;
    })
    .catch((e) => {
      statusEl.textContent = "camera: " + e.message;
    });

  // Slider opts, overridden by the detected alignment when it has locked.
  function samplerOpts() {
    const base = camOpts(root, "rx");
    if (aligned) {
      return {
        ...base,
        blockPx: aligned.pitchX,
        blockPy: aligned.pitchY,
        offX: aligned.offX,
        offY: aligned.offY,
      };
    }
    return base;
  }

  // Lowest chunk index not yet received (for NAKing lost frames).
  function lowestMissing() {
    for (let i = 0; i < totalChunks; i++) {
      if (!chunks[i]) return i;
    }
    return 0;
  }

  function drawCam(v, canvas) {
    const ctx = canvas.getContext("2d");
    if (!ctx || !v.videoWidth || canvas.width === 0) return;
    const cw = canvas.width;
    const ch = canvas.height;
    const vw = v.videoWidth;
    const vh = v.videoHeight;
    const scale = Math.min(cw / vw, ch / vh);
    const dw = vw * scale;
    const dh = vh * scale;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, cw, ch);
    ctx.drawImage(v, (cw - dw) / 2, (ch - dh) / 2, dw, dh);
  }

  function ensureSession(sessionId, total) {
    if (!session || session.sessionId !== sessionId) {
      totalChunks = total;
      chunks = new Array(total).fill(null);
      totalBytes = 0;
      session = core.TransferSession.rx(sessionId, totalChunks);
    }
  }

  function route(f, ok) {
    if (finished) return;
    if (f.kind === "Ping") {
      ensureSession(f.sessionId, f.totalChunks);
      const nf = session.nextFrame();
      if (nf) emitter.setBlocks(core.renderBlocks(nf));
    } else if (f.kind === "Chunk") {
      ensureSession(f.sessionId, f.totalChunks);
      if (ok) {
        if (!chunks[f.chunkIdx]) totalBytes += f.payload.length;
        chunks[f.chunkIdx] = f.payload;
        session.onChunkOk(f.chunkIdx);
      } else {
        session.onChunkErr(f.chunkIdx);
      }
      const nf = session.nextFrame();
      if (nf) emitter.setBlocks(core.renderBlocks(nf));
      const got = chunks.filter(Boolean).length;
      if (totalChunks > 0) {
        progressFill.style.width = (got / totalChunks) * 100 + "%";
      }
      if (session.allReceived() && chunks.every(Boolean)) finish();
    } else if (f.kind === "End") {
      if (session && session.allReceived()) finish();
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
    if (camHandle) {
      camHandle.stop();
      camHandle = null;
    }
  }

  return { stop };
}