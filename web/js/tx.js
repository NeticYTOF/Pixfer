// TX: pick a file, auto-zip when the estimated transfer time exceeds the
// 30s threshold, chunk into 223-byte payloads, and transmit in LOOP MODE:
// the sender round-robins Ping → Chunk 0 → … → Chunk N-1 → Ping (repeat),
// flashing each frame for a fixed dwell. No per-chunk stop-and-wait: any
// frame the receiver misses (shake, blur, off-screen) comes around again
// on the next pass. Each QR carries the chunk index + total in its header,
// so the receiver always knows exactly which frames it is missing. The
// receiver flashes a Done QR once it has every chunk; seeing Done stops
// the loop.
//
// The sender's camera is OPTIONAL: without it the loop still streams (it
// just cannot see the receiver's Done, so it stops at MAX_PASSES).

import * as core from "./core.js";
import { openCamera, makeEmitter, fitCanvas, keepScreenAwake, FLASH_PHASE_MS } from "./light.js";
import { qrEncode } from "./qr.js";
import { qrDecodeVideo } from "./qrdecode.js";

// Default frame dwells in whole flash phases. The flash phase itself is
// settable at runtime (#tx-flash): slow cameras integrate long exposures,
// and the receiver's status names the exact ms to type. Exported so the
// camera simulation stays in lockstep with the sender's defaults.
export const PING_DWELL_MS = FLASH_PHASE_MS;
export const CHUNK_DWELL_MS = FLASH_PHASE_MS * 2;
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
  emitter.setBlocks(new Uint8Array(core.GRID_W * core.GRID_H).fill(0));

  let session = null;
  let data = null;
  let totalChunks = 0;
  let dwellTimer = null;
  let camHandle = null;
  let looping = false;
  let pass = 0;
  let cursor = -1; // -1 = ping phase, 0..N-1 = chunk frames
  let pongSeen = false;
  let startedAt = 0;
  let releaseWake = null;
  // live frame timing — derived from the flash phase so every frame shows
  // whole phases at any setting
  let pingDwell = PING_DWELL_MS;
  let chunkDwell = CHUNK_DWELL_MS;

  function setStatus(t) {
    statusEl.textContent = t;
  }

  function elapsedS() {
    return startedAt ? ((performance.now() - startedAt) / 1000).toFixed(1) : "0.0";
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
      setStatus(
        (pongSeen ? "receiver is listening — looping frames… " : "waiting for a receiver… ") +
          "pass " + (pass + 1) + " (" + elapsedS() + "s)"
      );
    } else {
      setStatus(
        "pass " + (pass + 1) + " — flashing frame " + (cursor + 1) + "/" + totalChunks +
          " (" + elapsedS() + "s" + (pongSeen ? "" : ", no receiver yet") + ")"
      );
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
          setStatus("stopped after " + MAX_PASSES + " passes — no Done from the receiver. Try again?");
          return;
        }
      }
      stepLoop();
    }, cursor < 0 ? pingDwell : chunkDwell);
  }

  fileInput.addEventListener("change", async () => {
    const file = fileInput.files && fileInput.files[0];
    fileInput.value = ""; // re-picking the same file must fire "change" again
    if (!file) return;
    nameEl.textContent = file.name;
    if (file.size === 0) {
      setStatus("that file is empty — pick another one");
      return;
    }
    setStatus("reading " + file.name + "…");
    let raw;
    try {
      raw = new Uint8Array(await file.arrayBuffer());
    } catch (e) {
      setStatus("could not read the file: " + e.message);
      return;
    }
    const plan = core.transferPlan(file.size);
    if (plan.shouldZip) {
      try {
        data = fflate.zipSync({ [file.name]: raw });
        setStatus(
          "zipping — est. " + plan.estSeconds.toFixed(0) + "s > " + core.ZIP_THRESHOLD_S + "s, auto-zip on"
        );
      } catch (e) {
        setStatus("could not zip the file: " + e.message);
        return;
      }
    } else {
      data = raw;
    }
    totalChunks = Math.ceil(data.length / core.RS_K);
    if (totalChunks > core.maxChunkCount()) {
      setStatus("too big for this link — over " + (core.MAX_TRANSFER_BYTES / 1048576) + " MB even zipped");
      return;
    }
    if (!plan.shouldZip) {
      setStatus((file.size / 1048576).toFixed(2) + " MB, est. " + plan.estSeconds.toFixed(1) + "s, sending as-is");
    }
    metaEl.textContent =
      totalChunks + " frames · ~" +
      ((pingDwell + totalChunks * chunkDwell) / 1000).toFixed(1) +
      "s per pass";
    progressFill.style.width = "0%";

    stopLoop();
    pass = 0;
    cursor = -1;
    pongSeen = false;
    startedAt = performance.now();
    const sessionId = (Math.random() * 0x10000) | 0;
    session = core.newFrame("Ping", sessionId, 0, totalChunks, null);
    looping = true;
    if (releaseWake) releaseWake();
    releaseWake = await keepScreenAwake();

    if (!camHandle) {
      try {
        camHandle = await openCamera(camVideo, (video) => {
          // Read the receiver's flashed QR codes (Pong / Done).
          const dec = qrDecodeVideo(video);
          if (dec.ok) route(dec.frame);
        });
        stepLoop();
      } catch (e) {
        setStatus(
          "camera unavailable (" + e.message + ") — still sending, but the Done signal can't be seen." +
            (typeof window !== "undefined" && !window.isSecureContext ? " Camera needs https:// or localhost." : "")
        );
        stepLoop(); // camera is optional in loop mode
      }
    } else {
      stepLoop();
    }
  });

  const flashInput = root.querySelector("#tx-flash");
  flashInput.addEventListener("change", () => {
    // The receiver names this number when its camera is slow — typing it
    // here lengthens the flash so long exposures stop blending phases.
    const raw = Number(flashInput.value);
    const v = Number.isFinite(raw) ? Math.max(100, Math.min(4000, Math.round(raw))) : FLASH_PHASE_MS;
    flashInput.value = v;
    emitter.setFlashPhase(v);
    pingDwell = v; // 1 whole phase
    chunkDwell = v * 2; // 2 whole phases
    if (totalChunks > 0) {
      metaEl.textContent =
        totalChunks + " frames · ~" +
        ((pingDwell + totalChunks * chunkDwell) / 1000).toFixed(1) +
        "s per pass";
    }
    setStatus("flash phase " + v + " ms — frames will show " + (v / 1000).toFixed(1) + "s per phase");
  });

  function route(f) {
    if (!session) return;
    if (f.sessionId !== session.sessionId) return; // foreign transfer nearby
    if (f.kind === "Pong" && f.totalChunks === totalChunks) {
      pongSeen = true;
    } else if (f.kind === "Done" && f.totalChunks === totalChunks) {
      stopLoop();
      progressFill.style.width = "100%";
      metaEl.textContent = "done";
      setStatus("done — receiver confirmed every frame (" + elapsedS() + "s)");
      emitter.setBlocks(new Uint8Array(core.GRID_W * core.GRID_H).fill(0));
    }
  }

  backBtn.addEventListener("click", () => {
    stop();
    if (onBack) onBack();
  });

  function stop() {
    stopLoop();
    emitter.stop();
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
