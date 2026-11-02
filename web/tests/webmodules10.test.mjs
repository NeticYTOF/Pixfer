// Behavioral tests for the new web modules: frame_encoder, rx_camera,
// tx_screen.
// Run: node tests/webmodules10.test.mjs (from web/)

import { FrameEncoder } from "../js/frame_encoder.js";
import { RxCamera } from "../js/rx_camera.js";
import { TxScreen } from "../js/tx_screen.js";
import { renderBlocks } from "../js/core.js";

let pass = 0;
let fail = 0;
const failures = [];

function check(name, cond, detail) {
  if (cond) {
    pass++;
    console.log(`ok - ${name}`);
  } else {
    fail++;
    failures.push(name);
    console.log(`FAIL - ${name}${detail ? ` (${detail})` : ""}`);
  }
}

// ---------- frame_encoder.js ----------
{
  const e = new FrameEncoder();
  const frame = { kind: "chunk", sessionId: 5, chunkIdx: 0, totalChunks: 4, payload: [1, 1, 1] };
  const blocks = e.encode(frame);
  check("encoder: produces full grid", blocks.length === 270 * 480, `got ${blocks.length}`);
  check("encoder: block count", e.blockCount() === 270 * 480);
  check("encoder: default margin 0", e.dwellMargin() === 0);

  const e2 = new FrameEncoder(15);
  check("encoder: configurable margin", e2.dwellMargin() === 15);

  const e3 = new FrameEncoder();
  const f2 = { kind: "chunk", sessionId: 5, chunkIdx: 0, totalChunks: 4, payload: [9, 9, 9] };
  const blocks2 = e3.encode(f2);
  check("encoder: roundtrip via decode", JSON.stringify(blocks2) !== JSON.stringify(blocks));
}

// ---------- rx_camera.js ----------
{
  // A fake HAL that returns a pre-rendered frame.
  function fakeHal(blocks, { failOpen = false, noFrame = false, lockOk = true } = {}) {
    let opened = false;
    return {
      open() {
        if (failOpen) throw new Error("camera offline");
        opened = true;
        return null;
      },
      close() {
        opened = false;
      },
      pollFrame() {
        if (noFrame) return null;
        return { blocks, tsMs: 0, seq: 0 };
      },
      lockExposure() {
        return lockOk;
      },
    };
  }

  const goodBlocks = renderBlocks({ kind: "chunk", sessionId: 5, chunkIdx: 0, totalChunks: 4, payload: [1, 1, 1] });
  const cam = new RxCamera(fakeHal(goodBlocks));
  const decoded = cam.captureAndDecode();
  // decoded is { kind, sessionId, chunkIdx, totalChunks, payload: Uint8Array }
  check("camera: decodes a frame", decoded.sessionId === 5 && Array.from(decoded.payload).join(",") === "1,1,1",
    `got sessionId=${decoded.sessionId} payload=${Array.from(decoded.payload)}`);

  let threwOpen = false;
  try {
    const camFail = new RxCamera(fakeHal([], { failOpen: true }));
    camFail.captureAndDecode();
  } catch (e) {
    threwOpen = true;
  }
  check("camera: open failure propagates", threwOpen);

  let threwNoFrame = false;
  try {
    const camNoFrame = new RxCamera(fakeHal(goodBlocks, { noFrame: true }));
    camNoFrame.captureAndDecode();
  } catch (e) {
    threwNoFrame = e.message === "no frame available";
  }
  check("camera: no frame propagates", threwNoFrame);

  let threwBadGrid = false;
  try {
    const camBad = new RxCamera(fakeHal(new Array(10).fill(0)));
    camBad.captureAndDecode();
  } catch (e) {
    threwBadGrid = true;
  }
  check("camera: malformed grid fails decode", threwBadGrid);
}

// ---------- tx_screen.js ----------
{
  function fakeDisplay({ multicolor = true, failRender = false } = {}) {
    let opened = false;
    let lastBlocks = null;
    const disp = {
      open() {
        opened = true;
      },
      close() {
        opened = false;
      },
      render(blocks) {
        if (failRender) throw new Error("display fault");
        lastBlocks = blocks;
        return null;
      },
      supportsMulticolor() {
        return multicolor;
      },
    };
    return { disp, getLast: () => lastBlocks };
  }

  const { disp, getLast } = fakeDisplay();
  const tx = new TxScreen(disp);
  const blocks = new Array(270 * 480).fill(0);
  tx.render(blocks);
  check("screen: renders full grid", JSON.stringify(getLast()) === JSON.stringify(blocks));
  check("screen: supports multicolor", tx.supportsMulticolor() === true);

  const { disp: disp2 } = fakeDisplay({ multicolor: false });
  const tx2 = new TxScreen(disp2);
  check("screen: no multicolor", tx2.supportsMulticolor() === false);

  const { disp: disp3 } = fakeDisplay({ failRender: true });
  const tx3 = new TxScreen(disp3);
  let threw = false;
  try {
    tx3.render([0, 1, 2]);
  } catch (e) {
    threw = true;
  }
  check("screen: render failure propagates", threw);

  const { disp: disp4 } = fakeDisplay();
  const tx4 = new TxScreen(disp4);
  tx4.close();
  check("screen: close resets state", true);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}