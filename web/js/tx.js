// TX: pick a file, auto-zip when the estimated transfer time exceeds the
// 30s threshold, chunk into 223-byte payloads, and transmit in LOOP MODE:
// the sender round-robins Ping → Chunk 0 → … → Chunk N-1 → Ping (repeat),
// flashing each frame for a fixed dwell. No per-chunk stop-and-wait: any
// frame the receiver misses (shake, blur, off-screen) comes around again
// on the next pass. Each QR carries the chunk index + total in its header,
// so the receiver always knows exactly which frames it is missing. The
// receiver flashes a Done QR once it has every chunk; seeing Done stops
// the loop.

import * as core from "./core.js";
import { openCamera, makeEmitter, fitCanvas } from "./light.js";
import { qrEncode } from "./qr.js";
import { qrDecodeVideo } from "./qrdecode.js";

const PING_DWELL_MS = 250; // must be a multiple of the 50 ms flash phase
const CHUNK_DWELL_MS = 300;
const MAX_PASSES = 40; // safety cap if the receiver never flashes Done

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
  let dwellTimer = null;
  let camHandle = null;
  let looping = false;
  let pass = 0;
  let cursor = -1; // -1 = ping phase, 0..N-1 = chunk frames

  function setStatus(t) {
    statusEl.textContent = t;
  }

  function stopLoop() {
    looping = false;
    if (dwellTimer) clearTimeout(dwellTimer);
    dwellTimer = null;
  }

  function chunkPayload(idx) {
    if (!data) return new Uint8Array(0);
    const start = idx * core.RS_K;
    const end = Math.min(data.length, start + core.RS_K);
    return data.subarray(start, end);
  }

  function frameForCursor() {
    if (cursor < 0) return core.newFrame("Ping", session.sessionId, 0, totalChunks, null);
    const f = core.newFrame("Chunk", session.sessionId, cursor, totalChunks, null);
    f.payload = chunkPayload(cursor);
    return f;
  }

  // Flash the current frame for its dwell, then advance the cursor.
  function stepLoop() {
    if (!looping) return;
    const f = frameForCursor();
    emitter.setQr(qrEncode(f));
    if (cursor < 0) {
      setStatus("loop pass " + (pass + 1) + " — waiting for receiver…");
    } else {
      setStatus("pass " + (pass + 1) + " — flashing frame " + (cursor + 1) + "/" + totalChunks);
      progressFill.style.width = (((cursor + 1) / totalChunks) * 100) + "%";
    }
    dwellTimer = setTimeout(() => {
      if (!looping) return;
      cursor++;
      if (cursor >= totalChunks) {
        cursor = -1;
        pass++;
        if (pass >= MAX_PASSES) {
          stopLoop();
          setStatus("gave up after " + MAX_PASSES + " passes — no Done from receiver");
          return;
        }
      }
      stepLoop();
    }, cursor < 0 ? PING_DWELL_MS : CHUNK_DWELL_MS);
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
    session = core.newFrame("Ping", sessionId, 0, totalChunks, null);
    metaEl.textContent = totalChunks + " frames";
    progressFill.style.width = "0%";

    stopLoop();
    pass = 0;
    cursor = -1;
    looping = true;

    if (!camHandle) {
      try {
        camHandle = await openCamera(camVideo, (video) => {
          // Read the receiver's flashed QR codes (Pong / Done).
          const dec = qrDecodeVideo(video);
          if (dec.ok) route(dec.frame);
        });
        stepLoop();
      } catch (e) {
        setStatus("camera: " + e.message);
      }
    } else {
      stepLoop();
    }
  });

  function route(f) {
    if (f.kind === "Pong") {
      setStatus("receiver is listening — looping frames…");
    } else if (f.kind === "Done") {
      if (session && f.sessionId === session.sessionId) {
        stopLoop();
        progressFill.style.width = "100%";
        metaEl.textContent = "done";
        setStatus("done — receiver confirmed every frame");
        emitter.setBlocks(new Uint8Array(core.GRID_W * core.GRID_H).fill(7));
      }
    }
  }

  backBtn.addEventListener("click", () => {
    stop();
    if (onBack) onBack();
  });

  function stop() {
    stopLoop();
    emitter.stop();
    if (camHandle) {
      camHandle.stop();
      camHandle = null;
    }
  }

  return { stop };
}
