// Shared light engine: camera capture + block sampling + screen emission.
// Both sides run the same two primitives: their screen emits light (canvas)
// and their camera reads the peer's screen. Keep in lockstep with the
// wire format in core.js.

import * as core from "./core.js";

// Open the environment (back) camera and call onFrame(video) on every new
// frame. Prefers requestVideoFrameCallback (Chrome 107+); falls back to
// requestAnimationFrame polling.
export async function openCamera(video, onFrame) {
  const stream = await navigator.mediaDevices.getUserMedia({
    video: { width: 1280, height: 720, facingMode: "environment" },
    audio: false,
  });
  video.srcObject = stream;
  video.muted = true;
  video.playsInline = true;
  await video.play();
  let stopped = false;
  const tick = () => {
    if (stopped) return;
    onFrame(video);
    if (typeof video.requestVideoFrameCallback === "function") {
      video.requestVideoFrameCallback(tick);
    } else {
      requestAnimationFrame(tick);
    }
  };
  tick();
  return {
    stream,
    stop() {
      stopped = true;
      for (const t of stream.getTracks()) t.stop();
      video.srcObject = null;
    },
  };
}

// Read calibration slider values from a view's header. Pass the prefix that
// matches the view ("tx" or "rx"); pass null for defaults.
export function camOpts(root, prefix) {
  if (!root) return { blockPx: 4, offX: 0, offY: 0, gain: 1 };
  const val = (suffix) => {
    const el = root.querySelector("#" + prefix + suffix);
    return el ? el.value : null;
  };
  return {
    blockPx: parseInt(val("-blockpx") || "4", 10),
    offX: parseInt(val("-offx") || "0", 10),
    offY: parseInt(val("-offy") || "0", 10),
    gain: parseFloat(val("-gain") || "1"),
  };
}

// Sample one video frame into the 270x480 grid of 3-bit block values.
// The 8-color palette is the full 3-bit cube, so nearest-palette-color is
// exactly per-channel thresholding at the midpoint after gain scaling.
// `blockPy` allows a separate vertical pitch: camera FOV distorts the
// grid, so X and Y block spacing are generally different on screen.
export function sampleBlocks(video, opts = {}) {
  const { blockPx = 4, blockPy = 4, offX = 0, offY = 0, gain = 1 } = opts;
  const vw = video.videoWidth || 640;
  const vh = video.videoHeight || 480;

  let cv = sampleBlocks._cv;
  if (!cv || cv.width !== vw || cv.height !== vh) {
    cv = document.createElement("canvas");
    cv.width = vw;
    cv.height = vh;
    sampleBlocks._cv = cv;
  }
  const ctx = cv.getContext("2d", { willReadFrequently: true });
  ctx.drawImage(video, 0, 0, vw, vh);
  const d = ctx.getImageData(0, 0, vw, vh).data;

  const blocks = new Uint8Array(core.GRID_W * core.GRID_H);
  for (let r = 0; r < core.GRID_H; r++) {
    for (let c = 0; c < core.GRID_W; c++) {
      const x = Math.max(0, Math.min(vw - 1, (offX + c * blockPx + Math.floor(blockPx / 2)) | 0));
      const y = Math.max(0, Math.min(vh - 1, (offY + r * blockPy + Math.floor(blockPy / 2)) | 0));
      const idx = (y * vw + x) * 4;
      const rv = Math.min(255, d[idx] * gain) | 0;
      const gv = Math.min(255, d[idx + 1] * gain) | 0;
      const bv = Math.min(255, d[idx + 2] * gain) | 0;
      blocks[r * core.GRID_W + c] =
        (rv >= 128 ? 1 : 0) | (gv >= 128 ? 2 : 0) | (bv >= 128 ? 4 : 0);
    }
  }
  return blocks;
}

// Blink period for flashing frames: 50 ms per phase → a 10 Hz full blink
// cycle. Slow enough for camera exposure to resolve each phase cleanly.
const FLASH_PHASE_MS = 50;

// Emit a block grid through a screen. The 270x480 grid is stretched to fill
// the whole canvas (rectangular blocks), so the display uses the full box
// instead of a letterboxed square. Repaint happens only when the blocks or
// the canvas size change — no per-frame pixel work in the hot loop.
//
// `setFrame(frame)` enables "proper flashing QR-code" output: the emitter
// alternates between the frame and its 3-bit-inverted rendering, giving the
// peer's camera a strong temporal sync signal. The receiver recovers the
// frame from either phase via core.inverseBlocks (see decode fallbacks in
// tx.js / rx.js).
export function makeEmitter(canvas) {
  const ctx = canvas.getContext("2d");
  const pal = core.PALETTE;
  let blocksRef = null; // static (idle) blocks
  let flash = null; // { normal, inverse } when flashing a session frame
  let flashQr = null; // { data (0/1 modules), size } when flashing a QR code
  let phase = false; // false = normal (black-on-white), true = inverted
  let lastToggle = 0;
  let lastW = 0;
  let lastH = 0;
  let dirty = false;
  let running = false;

  function paint() {
    const w = canvas.width;
    const h = canvas.height;
    if (w === 0 || h === 0) return;
    if (flashQr) {
      // QR render: fillRect per module, fast. Normal phase = black modules
      // on white; inverted phase = white modules on black (the flash).
      if (w === lastW && h === lastH && !dirty) return;
      lastW = w;
      lastH = h;
      dirty = false;
      const n = flashQr.size;
      const mp = Math.max(1, (w / n) | 0);
      const bg = phase ? 0 : 255;
      const fg = phase ? 255 : 0;
      ctx.fillStyle = `rgb(${bg},${bg},${bg})`;
      ctx.fillRect(0, 0, w, h);
      ctx.fillStyle = `rgb(${fg},${fg},${fg})`;
      const data = flashQr.data;
      for (let r = 0; r < n; r++) {
        for (let c = 0; c < n; c++) {
          if (data[r * n + c]) ctx.fillRect(c * mp, r * mp, mp, mp);
        }
      }
      return;
    }
    const active = flash ? (phase ? flash.inverse : flash.normal) : blocksRef;
    if (!active) return;
    if (w === lastW && h === lastH && !dirty) return;
    lastW = w;
    lastH = h;
    dirty = false;
    const img = ctx.createImageData(w, h);
    const d = img.data;
    const bw = w / core.GRID_W;
    const bh = h / core.GRID_H;
    for (let r = 0; r < core.GRID_H; r++) {
      for (let c = 0; c < core.GRID_W; c++) {
        const col = pal[active[r * core.GRID_W + c]];
        const x0 = Math.floor(c * bw);
        const x1 = Math.floor((c + 1) * bw);
        const y0 = Math.floor(r * bh);
        const y1 = Math.floor((r + 1) * bh);
        for (let y = y0; y < y1; y++) {
          let idx = (y * w + x0) * 4;
          const n = x1 - x0;
          for (let x = 0; x < n; x++) {
            d[idx] = col[0];
            d[idx + 1] = col[1];
            d[idx + 2] = col[2];
            d[idx + 3] = 255;
            idx += 4;
          }
        }
      }
    }
    ctx.putImageData(img, 0, 0);
  }

  function tick() {
    if (!running) return;
    if (flash || flashQr) {
      const now = performance.now();
      if (now - lastToggle >= FLASH_PHASE_MS) {
        lastToggle = now;
        phase = !phase;
        dirty = true; // repaint only on phase flips, not every rAF tick
      }
    }
    paint();
    requestAnimationFrame(tick);
  }

  function setBlocks(blocks) {
    blocksRef = blocks;
    flash = null;
    flashQr = null;
    dirty = true;
    if (!running) {
      running = true;
      requestAnimationFrame(tick);
    }
  }

  // Flash a protocol frame: render it and its 3-bit inverse alternately.
  function setFrame(frame) {
    const normal = core.renderBlocks(frame);
    flash = { normal, inverse: core.inverseBlocks(normal) };
    flashQr = null;
    phase = false;
    lastToggle = performance.now();
    dirty = true;
    if (!running) {
      running = true;
      requestAnimationFrame(tick);
    }
  }

  // Flash an actual QR code: alternate black-on-white / white-on-black.
  // `qr` = { data: Uint8Array of 0/1 modules, size: N }.
  function setQr(qr) {
    flashQr = qr;
    flash = null;
    phase = false;
    lastToggle = performance.now();
    dirty = true;
    if (!running) {
      running = true;
      requestAnimationFrame(tick);
    }
  }

  return {
    setBlocks,
    setFrame,
    setQr,
    stop() {
      running = false;
      flash = null;
      flashQr = null;
    },
  };
}

// Keep a canvas pixel size matched to its own rendered (CSS) box, so the
// grid uses the full display area without letterboxing.
export function fitCanvas(canvas) {
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
}