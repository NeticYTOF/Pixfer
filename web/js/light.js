// Shared light engine: camera capture + block sampling + screen emission.
// Both sides run the same two primitives: their screen emits light (canvas)
// and their camera reads the peer's screen. Keep in lockstep with the
// wire format in core.js.

import * as core from "./core.js";
import { COLOR_HUES } from "./qr.js";

// Open the environment (back) camera and call onFrame(video) on every new
// frame. Prefers requestVideoFrameCallback (Chrome 107+); falls back to
// requestAnimationFrame polling.
export async function openCamera(video, onFrame) {
  // Ask for the highest resolution AND a solid frame rate. Phones
  // silently deliver 10-15 fps when the resolution ask is unconstrained
  // or in dim rooms (long exposure per frame) — and the flash capture
  // budget depends on exposure being short relative to a 200 ms phase.
  const stream = await navigator.mediaDevices.getUserMedia({
    video: {
      facingMode: "environment",
      width: { ideal: 1920 },
      height: { ideal: 1080 },
      frameRate: { ideal: 30 },
    },
    audio: false,
  });
  video.srcObject = stream;
  video.muted = true;
  video.playsInline = true;
  await video.play();
  let stopped = false;
  const tick = () => {
    if (stopped) return;
    // A decode bug must never kill the capture loop: an exception in the
    // frame callback would otherwise skip the re-request below and the
    // preview freezes on the last frame with no way back.
    try {
      onFrame(video);
    } catch (e) {
      console.error("pixfer: camera frame handler error", e);
    }
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

// Blink period for flashing frames. 200 ms per phase → 5 phase changes
// per second (a 2.5 Hz full blink cycle — still clearly flashing). Real
// phone cameras have rolling shutters, auto-exposure that hunts the
// blink, and dim-light frame integration of 30-70 ms; at 5 flips/sec the
// exposure fits inside a single phase almost always, AE settles between
// steps, and even 10 fps cameras catch 3+ clean frames per phase.
export const FLASH_PHASE_MS = 200;

// Quiet-zone width (modules) and the dark-module palette for QR output.
// Palette colors are luminance-dark (gray < 128) AND strongly saturated
// with a near-zero minimum channel, so the peer's camera classifies them
// as dark even after gain/white-balance shifts (isDarkPixel in
// qrdecode.js). Color carries no data — only the dark/light classification
// matters — it makes the flashing code readable to the eye.
export const QR_QUIET_MODULES = 4;
export const QR_DARK_COLORS = [
  [0, 0, 0], // black
  [192, 0, 0], // red
  [0, 144, 0], // green
  [0, 0, 192], // blue
  [192, 96, 0], // amber
  [128, 0, 160], // purple
  [0, 150, 150], // cyan
  [160, 0, 160], // magenta
];

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
  let flashPhaseMs = FLASH_PHASE_MS; // settable for slow cameras (see setFlashPhase)
  let blocksRef = null; // static (idle) blocks
  let flash = null; // { normal, inverse } when flashing a session frame
  let flashQr = null; // { data (0/1 or color 0..4 modules), size, rot, colorMode } when flashing a QR code
  let flashGrid = null; // { qrs: [...], cols, rows } when flashing a multi-QR grid
  let qrRot = 0; // per-QR palette rotation for the color cycle
  let phase = false; // false = normal (colored-on-white), true = inverted
  let lastToggle = 0;
  let lastW = 0;
  let lastH = 0;
  let dirty = false;
  let running = false;

  // Fill one QR into a cell of the canvas. Handles both frame formats:
  // legacy 0/1 matrices (decorative rotating palette on dark modules) and
  // color frames (values: 0 light, 1 black, 2..4 = COLOR_HUES[value-1]).
  // The inverted flash phase swaps the background and channel-complements
  // every color.
  function paintQrInCell(qr, cellX, cellY, cellW, cellH, rot) {
    const n = qr.size;
    const quiet = qr.colorMode ? 2 : QR_QUIET_MODULES; // grid cells get tighter quiet zones
    const units = n + 2 * quiet;
    const mp = Math.max(1, Math.floor(Math.min(cellW, cellH) / units));
    const box = mp * units;
    const ox = Math.floor(cellX + (cellW - box) / 2) + quiet * mp;
    const oy = Math.floor(cellY + (cellH - box) / 2) + quiet * mp;
    const data = qr.data;
    const k = QR_DARK_COLORS.length;
    for (let r = 0; r < n; r++) {
      for (let c = 0; c < n; c++) {
        const val = data[r * n + c];
        if (!val) continue; // light module: background in both phases
        if (phase) {
          // inverted flash: dark values render as their channel
          // complements — exactly what the receiver's hue classifier
          // compensates for
          if (qr.colorMode) {
            if (val === 1) ctx.fillStyle = "rgb(255,255,255)";
            else {
              const h = COLOR_HUES[val - 1];
              ctx.fillStyle = `rgb(${255 - h[0]},${255 - h[1]},${255 - h[2]})`;
            }
          } else {
            ctx.fillStyle = "rgb(255,255,255)";
          }
        } else if (qr.colorMode) {
          if (val === 1) ctx.fillStyle = "rgb(0,0,0)";
          else {
            const h = COLOR_HUES[val - 1];
            ctx.fillStyle = `rgb(${h[0]},${h[1]},${h[2]})`;
          }
        } else {
          const col = QR_DARK_COLORS[(r * 7 + c * 11 + rot) % k];
          ctx.fillStyle = `rgb(${col[0]},${col[1]},${col[2]})`;
        }
        ctx.fillRect(ox + c * mp, oy + r * mp, mp, mp);
      }
    }
  }

  function paint() {
    const w = canvas.width;
    const h = canvas.height;
    if (w === 0 || h === 0) return;
    const grid = flashGrid;
    const single = flashQr;
    if (grid || single) {
      if (w === lastW && h === lastH && !dirty) return;
      lastW = w;
      lastH = h;
      dirty = false;
      const bg = phase ? 0 : 255;
      ctx.fillStyle = `rgb(${bg},${bg},${bg})`;
      ctx.fillRect(0, 0, w, h);
      if (single) {
        paintQrInCell(single, 0, 0, w, h, single.rot | 0);
      } else {
        const cols = grid.cols;
        const rows = grid.rows;
        const cw = w / cols;
        const chh = h / rows;
        for (let i = 0; i < grid.qrs.length; i++) {
          const qr = grid.qrs[i];
          const gx = i % cols;
          const gy = (i / cols) | 0;
          paintQrInCell(qr, gx * cw, gy * chh, cw, chh, (qrRot + i) | 0);
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
    if (flash || flashQr || flashGrid) {
      const now = performance.now();
      if (now - lastToggle >= flashPhaseMs) {
        lastToggle = now;
        phase = !phase;
        dirty = true; // repaint only on phase flips, not every rAF tick
      }
    }
    paint();
    requestAnimationFrame(tick);
  }

  // Set the flash phase duration (ms). Slow cameras integrate long
  // exposures — the receiver measures its own fps and tells the user a
  // value to type here, so captures stop blending across the phase flip.
  function setFlashPhase(ms) {
    flashPhaseMs = Math.max(50, Math.round(ms));
  }

  function setBlocks(blocks) {
    blocksRef = blocks;
    flash = null;
    flashQr = null;
    flashGrid = null;
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

  // Flash an actual QR code: alternate colored-on-white / white-on-black.
  // `qr` = { data, size, colorMode? } — legacy 0/1 matrices or color 0..4.
  // Each new QR shifts the color rotation so consecutive frames cycle
  // through the palette.
  function setQr(qr) {
    qrRot = (qrRot + 1) % QR_DARK_COLORS.length;
    flashQr = { data: qr.data, size: qr.size, colorMode: !!qr.colorMode, rot: qrRot };
    flashGrid = null;
    flash = null;
    phase = false;
    lastToggle = performance.now();
    dirty = true;
    if (!running) {
      running = true;
      requestAnimationFrame(tick);
    }
  }

  // Flash several codes at once (multi-QR grid). `qrs` is the array of
  // encoded matrices for this dwell; the layout (cols x rows) is fixed by
  // the grid capacity so positions stay steady across frames — the
  // receiver's warm-start geometry cache exploits exactly that.
  function setQrGrid(qrs, cols, rows) {
    if (!qrs || !qrs.length) return;
    qrRot = (qrRot + 1) % QR_DARK_COLORS.length;
    flashQr = null;
    flashGrid = { qrs, cols, rows };
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
    setQrGrid,
    setFlashPhase,
    stop() {
      running = false;
      flash = null;
      flashQr = null;
      flashGrid = null;
    },
  };
}

// Keep the screen awake while a transfer runs — phones otherwise dim or
// lock mid-loop and the transfer stalls silently. Returns a release
// function; safe to call anywhere (no-ops when unsupported). The lock
// auto-releases when the tab hides, so it is re-requested on return to
// visibility until released.
export async function keepScreenAwake() {
  let lock = null;
  let released = false;
  try {
    if (typeof navigator !== "undefined" && "wakeLock" in navigator) {
      const request = async () => {
        if (released) return;
        try {
          lock = await navigator.wakeLock.request("screen");
        } catch (e) {
          /* unsupported or the tab is hidden */
        }
      };
      await request();
      if (typeof document !== "undefined") {
        document.addEventListener("visibilitychange", request);
      }
      return () => {
        released = true;
        if (typeof document !== "undefined") {
          document.removeEventListener("visibilitychange", request);
        }
        try {
          if (lock) lock.release();
        } catch (e) {
          /* already released */
        }
        lock = null;
      };
    }
  } catch (e) {
    /* unsupported */
  }
  return () => {};
}

// Keep a canvas pixel size matched to its rendered (CSS) box, sharpened
// by the device pixel ratio so QR modules stay crisp on hi-DPI screens.
// The QR painter derives its module pitch from the pixel size, so a
// sharper canvas means sharper module edges for the peer's camera.
export function fitCanvas(canvas) {
  const dpr = typeof window === "undefined" ? 1 : Math.max(1, Math.min(3, window.devicePixelRatio || 1));
  const w = Math.max(1, Math.round((canvas.clientWidth || 1) * dpr));
  const h = Math.max(1, Math.round((canvas.clientHeight || 1) * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
}