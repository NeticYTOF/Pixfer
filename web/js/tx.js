// TX: pick a file, auto-zip when the estimated transfer time exceeds the
// 30s threshold, chunk into 223-byte payloads, and transmit with
// stop-and-wait ARQ. This side flashes ACTUAL QR codes (black/white,
// standard QR format with Reed-Solomon FEC) and reads Pong/Ack/Nak QR
// codes from the peer's screen with its camera.

import * as core from "./core.js";
import { openCamera, makeEmitter, fitCanvas } from "./light.js";
import { qrEncode } from "./qr.js";
import { qrDecodeVideo } from "./qrdecode.js";

const ACK_TIMEOUT_MS = 2500;

export function mountTx(root, fflate, onBack) {
  const canvas = root.querySelector("#tx-canvas");
  const camVideo = root.querySelector("#tx-camera");
  const statusEl = root.querySelector("#tx-status");
  const metaEl = root.querySelector("#tx-meta");
  const progressFill = root.querySelector("#tx-progress");
  const fileInput = root.querySelector("#tx-file");
  const nameEl = root.querySelector("#tx-filename");
  const backBtn = root.querySelector("#tx-back");

  const parent = canvas.parentElement;
  fitCanvas(canvas);
  new ResizeObserver(() => fitCanvas(canvas)).observe(parent);

  const emitter = makeEmitter(canvas);
  // Idle screen: full-white grid (all blocks 7).
  emitter.setBlocks(new Uint8Array(core.GRID_W * core.GRID_H).fill(7));

  let session = null;
  let data = null;
  let totalChunks = 0;
  let ackTimer = null;
  let camHandle = null;

  function setStatus(t) {
    statusEl.textContent = t;
  }

  fileInput.addEventListener("change", async () => {
    const file = fileInput.files && fileInput.files[0];
    if (!file) return;
    nameEl.textContent = file.name;
    setStatus("reading " + file.name + "…");
    const raw = new Uint8Array(await file.arrayBuffer());
    const plan = core.transferPlan(file.size);
    if (plan.shouldZip) {
      data = fflate.zipSync({ [file.name]: raw });
      setStatus(
        "zipping — est. " +
          plan.estSeconds.toFixed(0) +
          "s > " +
          core.ZIP_THRESHOLD_S +
          "s, auto-zip on"
      );
    } else {
      data = raw;
      setStatus(
        (file.size / 1048576).toFixed(2) +
          " MB, est. " +
          plan.estSeconds.toFixed(1) +
          "s, sending as-is"
      );
    }
    totalChunks = Math.ceil(data.length / core.RS_K);
    const sessionId = (Math.random() * 0x10000) | 0;
    session = core.TransferSession.tx(sessionId, totalChunks);
    metaEl.textContent = totalChunks + " chunks";
    progressFill.style.width = "0%";

    if (!camHandle) {
      try {
        camHandle = await openCamera(camVideo, (video) => {
          // Read the peer's flashed QR codes (Pong/Ack/Nak).
          const dec = qrDecodeVideo(video);
          if (dec.ok) route(dec.frame);
        });
        pump();
      } catch (e) {
        setStatus("camera: " + e.message);
      }
    } else {
      pump();
    }
  });

  function route(f) {
    if (!session) return;
    if (f.kind === "Pong") {
      session.onPong();
      pump();
    } else if (f.kind === "Ack") {
      clearTimeout(ackTimer);
      session.onAck(f.chunkIdx);
      progressFill.style.width = (((f.chunkIdx + 1) / totalChunks) * 100) + "%";
      metaEl.textContent = "chunk " + (f.chunkIdx + 1) + "/" + totalChunks;
      pump();
    } else if (f.kind === "Nak") {
      clearTimeout(ackTimer);
      session.onNak(f.chunkIdx);
      pump();
    }
  }

  function chunkPayload(idx) {
    if (!data) return new Uint8Array(0);
    const start = idx * core.RS_K;
    const end = Math.min(data.length, start + core.RS_K);
    return data.subarray(start, end);
  }

  function pump() {
    if (!session) return;
    const f = session.nextFrame();
    if (!f) {
      setStatus("done — all chunks acked");
      return;
    }
    if (f.kind === "Chunk") {
      f.payload = chunkPayload(f.chunkIdx);
      emitter.setQr(qrEncode(f)); // flash an actual QR code (black/white)
      clearTimeout(ackTimer);
      ackTimer = setTimeout(onAckTimeout, ACK_TIMEOUT_MS);
    } else {
      emitter.setQr(qrEncode(f)); // flash Ping/End as QR too
    }
  }

  function onAckTimeout() {
    if (!session || session.state !== "AwaitingAck") return;
    const r = session.onTimeout();
    if (r) {
      r.payload = chunkPayload(r.chunkIdx);
      emitter.setQr(qrEncode(r));
      ackTimer = setTimeout(onAckTimeout, ACK_TIMEOUT_MS);
    } else {
      setStatus("no ack — transfer failed after retries");
    }
  }

  backBtn.addEventListener("click", () => {
    stop();
    if (onBack) onBack();
  });

  function stop() {
    emitter.stop();
    if (ackTimer) clearTimeout(ackTimer);
    if (camHandle) {
      camHandle.stop();
      camHandle = null;
    }
  }

  return { stop };
}