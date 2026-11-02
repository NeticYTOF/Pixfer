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
export function sampleBlocks(video, opts = {}) {
  const { blockPx = 4, offX = 0, offY = 0, gain = 1 } = opts;
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
      const x = Math.max(0, Math.min(vw - 1, (offX + c * blockPx + (blockPx / 2) | 0));
      const y = Math.max(0, Math.min(vh - 1, (offY + r * blockPx + (blockPx / 2) | 0));
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

// Emit a block grid through a screen: pre-render one 1080x1920 ImageData per
// frame, then blit-scaled on every animation frame (no per-pixel re-render
// in the hot loop).
export function makeEmitter(canvas) {
  const off = document.createElement("canvas");
  off.width = core.GRID_W * core.BLOCK_PX; // 1080
  off.height = core.GRID_H * core.BLOCK_PX; // 1920
  const octx = off.getContext("2d");
  const ctx = canvas.getContext("2d");
  const pal = core.PALETTE;
  let img = null;
  let running = false;

  function blit() {
    if (!running || !img) return;
    octx.putImageData(img, 0, 0);
    const vw = canvas.width;
    const vh = canvas.height;
    if (vw > 0 && vh > 0) {
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, vw, vh);
      const scale = Math.min(vw / off.width, vh / off.height);
      const dw = off.width * scale;
      const dh = off.height * scale;
      ctx.drawImage(off, (vw - dw) / 2, (vh - dh) / 2, dw, dh);
    }
    requestAnimationFrame(blit);
  }

  function setBlocks(blocks) {
    if (!img) img = octx.createImageData(off.width, off.height);
    const d = img.data;
    for (let r = 0; r < core.GRID_H; r++) {
      for (let c = 0; c < core.GRID_W; c++) {
        const col = pal[blocks[r * core.GRID_W + c]];
        const x0 = c * core.BLOCK_PX;
        const y0 = r * core.BLOCK_PX;
        for (let dy = 0; dy < core.BLOCK_PX; dy++) {
          let idx = ((y0 + dy) * off.width + x0) * 4;
          for (let dx = 0; dx < core.BLOCK_PX; dx++) {
            d[idx] = col[0];
            d[idx + 1] = col[1];
            d[idx + 2] = col[2];
            d[idx + 3] = 255;
            idx += 4;
          }
        }
      }
    }
    if (!running) {
      running = true;
      requestAnimationFrame(blit);
    }
  }

  return {
    setBlocks,
    stop() {
      running = false;
    },
  };
}

// Keep a canvas pixel size matched to its container.
export function fitCanvas(canvas) {
  const parent = canvas.parentElement;
  if (!parent) return;
  const w = parent.clientWidth;
  const h = parent.clientHeight;
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
}