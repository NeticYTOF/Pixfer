// RX: open the camera, decode the sender's looping QR frames, reassemble
// chunks by their frame number (each QR header carries chunkIdx +
// totalChunks, so the receiver always knows exactly which frames it is
// missing and just waits for the loop to bring them around again),
// auto-unzip zipped payloads, download the file, and flash a Done QR
// until the sender stops.

import * as core from "./core.js";
import { openCamera, makeEmitter, fitCanvas } from "./light.js";
import { qrEncode } from "./qr.js";
import { qrDecodeVideo } from "./qrdecode.js";

const PONG_DWELL_MS = 400;

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
  let pongTimer = null;

  let session = null;
  let sessionId = -1;
  let chunks = [];
  let totalChunks = 0;
  let totalBytes = 0;
  let finished = false;
  let camHandle = null;

  openCamera(video, (v) => {
      drawCam(v, camCanvas);
      // Decode the sender's looping frames (actual QR, colored flash).
      const dec = qrDecodeVideo(v);
      if (dec.ok) route(dec.frame);
    })
    .then((h) => {
      camHandle = h;
    })
    .catch((e) => {
      statusEl.textContent = "camera: " + e.message;
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
    const scale = Math.min(cw / vw, ch / vh);
    const dw = vw * scale;
    const dh = vh * scale;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, cw, ch);
    ctx.drawImage(v, (cw - dw) / 2, (ch - dh) / 2, dw, dh);
  }

  function flashBriefly(frame) {
    emitter.setQr(qrEncode(frame));
    if (pongTimer) clearTimeout(pongTimer);
    pongTimer = setTimeout(() => {
      // back to the plain idle screen so the loop keeps decoding cleanly
      emitter.setBlocks(new Uint8Array(core.GRID_W * core.GRID_H).fill(7));
    }, PONG_DWELL_MS);
  }

  function ensureSession(sid, total) {
    if (session && sessionId === sid) return;
    sessionId = sid;
    totalChunks = total;
    chunks = new Array(total).fill(null);
    totalBytes = 0;
    finished = false;
    session = core.newFrame("Pong", sid, 0, total, null);
    statusEl.textContent = "sender detected — " + total + " frames, collecting…";
  }

  function route(f) {
    if (finished) return;
    if (f.kind === "Ping") {
      ensureSession(f.sessionId, f.totalChunks);
      flashBriefly(core.newFrame("Pong", f.sessionId, 0, f.totalChunks, null));
    } else if (f.kind === "Chunk") {
      ensureSession(f.sessionId, f.totalChunks);
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
    // Tell the sender to stop: keep flashing Done until it reacts.
    if (pongTimer) clearTimeout(pongTimer);
    emitter.setQr(qrEncode(core.newFrame("Done", sessionId, 0, totalChunks, null)));
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
    if (camHandle) {
      camHandle.stop();
      camHandle = null;
    }
  }

  return { stop };
}
