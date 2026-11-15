// Module smoke tests: every browser-side ES module must parse and load,
// and the view mounts must actually RUN under a fake DOM without runtime
// errors. A plain import cannot catch closure-level bugs (an undefined
// identifier inside a callback only throws when the callback fires — a
// shipped "fpsShown is not defined" froze the receiver preview exactly
// that way), so this harness mounts the real views, drives the camera
// callback through the stubbed rAF loop, and fails on any console.error.
// Run: node tests/module_smoke.test.mjs (from web/)

import test from "node:test";
import assert from "node:assert/strict";
import * as path from "node:path";
import * as url from "node:url";
import * as fs from "node:fs/promises";

const here = url.fileURLToPath(new URL(".", import.meta.url));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const modules = [
  "../js/core.js",
  "../js/light.js",
  "../js/qr.js",
  "../js/qrdecode.js",
  "../js/tx.js",
  "../js/rx.js",
  "../js/frame_codec.js",
  "../js/frame_encoder.js",
];

for (const mod of modules) {
  const name = path.basename(mod);
  test(`module ${name} loads cleanly`, async () => {
    const imported = await import(mod);
    assert.ok(imported, `${name} did not export anything`);
    if (mod.endsWith("tx.js")) {
      assert.equal(typeof imported.mountTx, "function");
      assert.equal(typeof imported.PING_DWELL_MS, "number");
      assert.equal(typeof imported.CHUNK_DWELL_MS, "number");
    }
    if (mod.endsWith("rx.js")) assert.equal(typeof imported.mountRx, "function");
    if (mod.endsWith("light.js")) {
      assert.equal(typeof imported.makeEmitter, "function");
      assert.equal(typeof imported.openCamera, "function");
      assert.equal(typeof imported.FLASH_PHASE_MS, "number");
    }
    if (mod.endsWith("qrdecode.js")) {
      assert.equal(typeof imported.qrDecodeFromPixels, "function");
      assert.equal(typeof imported.qrDecodeVideo, "function");
    }
  });
}

// The index page must reference every element id the mount code queries,
// or a null-element query throws the moment a view opens.
test("index.html carries every id the views query", async () => {
  const html = await fs.readFile(path.join(here, "..", "index.html"), "utf8");
  const ids = [
    "landing", "btn-tx", "btn-rx",
    "tx-view", "tx-canvas", "tx-camera", "tx-status", "tx-meta", "tx-progress",
    "tx-file", "tx-filename", "tx-flash", "tx-framesize", "tx-codes", "tx-back",
    "rx-view", "rx-video", "rx-canvas", "rx-emit", "rx-status", "rx-progress",
    "rx-lock", "rx-back",
  ];
  const missing = ids.filter((id) => !html.includes(`id="${id}"`));
  assert.deepEqual(missing, [], `index.html is missing ids: ${missing.join(", ")}`);
});

// ---------- fake DOM harness ----------

function makeCtxStub() {
  return {
    imageSmoothingEnabled: true,
    imageSmoothingQuality: "high",
    drawImage() {},
    clearRect() {},
    fillRect() {},
    createImageData: (w, h) => ({ data: new Uint8ClampedArray(w * h * 4), width: w, height: h }),
    putImageData() {},
    getImageData: (x, y, w, h) => ({ data: new Uint8ClampedArray(w * h * 4), width: w, height: h }),
  };
}

function fakeDom() {
  const els = new Map();
  const makeEl = () => {
    const el = {
      textContent: "",
      value: "",
      style: {},
      width: 0,
      height: 0,
      clientWidth: 320,
      clientHeight: 240,
      parentElement: { observe() {} },
      _listeners: [],
      getContext: () => makeCtxStub(),
    };
    el.addEventListener = (t, fn) => el._listeners.push([t, fn]);
    return el;
  };
  const root = {
    querySelector: (sel) => {
      if (!els.has(sel)) els.set(sel, makeEl());
      return els.get(sel);
    },
  };
  return { root, els };
}

function installBrowserGlobals(errors) {
  const g = globalThis;
  const navDesc = Object.getOwnPropertyDescriptor(g, "navigator");
  const prev = {
    document: g.document,
    ResizeObserver: g.ResizeObserver,
    requestAnimationFrame: g.requestAnimationFrame,
    consoleError: console.error,
  };
  g.document = {
    createElement: () => ({ width: 0, height: 0, getContext: () => makeCtxStub() }),
    addEventListener() {},
    removeEventListener() {},
    body: { appendChild() {} },
  };
  g.ResizeObserver = class {
    observe() {}
    disconnect() {}
  };
  g.requestAnimationFrame = (cb) => setTimeout(cb, 1);
  if (g.URL) {
    g.URL.createObjectURL = () => "blob:fake";
    g.URL.revokeObjectURL = () => {};
  }
  // navigator is a getter-only global in Node: install a fake with a
  // working mediaDevices via defineProperty, so the camera callback path
  // (fps measurement, decode loop, hints) actually executes.
  try {
    const base = navDesc ? navDesc.value || {} : {};
    Object.defineProperty(g, "navigator", {
      configurable: true,
      get: () =>
        Object.assign(Object.create(Object.getPrototypeOf(base) || null), base, {
          mediaDevices: {
            getUserMedia: () =>
              Promise.resolve({
                getTracks: () => [],
                getVideoTracks: () => [
                  { stop() {}, getCapabilities: () => ({}), getSettings: () => ({}), applyConstraints: async () => {} },
                ],
              }),
          },
        }),
    });
  } catch (e) {
    // last resort: the camera error path is still exercised
  }
  console.error = (...args) => errors.push(args.map(String).join(" "));
  return {
    video: {
      videoWidth: 1280,
      videoHeight: 720,
      muted: true,
      playsInline: true,
      srcObject: null,
      play: () => Promise.resolve(),
    },
    restore: () => {
      g.document = prev.document;
      g.ResizeObserver = prev.ResizeObserver;
      g.requestAnimationFrame = prev.requestAnimationFrame;
      if (navDesc) Object.defineProperty(g, "navigator", navDesc);
      console.error = prev.consoleError;
    },
  };
}

test("mountRx drives its camera callback without runtime errors", async () => {
  const errors = [];
  const { root } = fakeDom();
  const { video, restore } = installBrowserGlobals(errors);
  const rx = await import("../js/rx.js");
  let handle;
  try {
    handle = rx.mountRx(root, {}, () => {});
    // deliver fake frames through the stubbed rAF loop
    video.requestVideoFrameCallback = null; // force the rAF fallback path
    await sleep(200); // ~dozens of frames at the 1 ms stub rate
    const status = root.querySelector("#rx-status").textContent;
    assert.ok(
      typeof status === "string" && status.length > 0,
      "rx status was never written"
    );
  } finally {
    if (handle) handle.stop();
    restore();
  }
  assert.deepEqual(errors, [], `runtime errors in the rx mount loop:\n${errors.join("\n")}`);
});

test("mountTx mounts, flash input retunes timing without errors", async () => {
  const errors = [];
  const { root } = fakeDom();
  const { restore } = installBrowserGlobals(errors);
  const tx = await import("../js/tx.js");
  let handle;
  try {
    handle = tx.mountTx(root, {}, () => {});
    const flashInput = root.querySelector("#tx-flash");
    flashInput.value = "600";
    const change = flashInput._listeners.find(([t]) => t === "change");
    assert.ok(change, "flash input has no change listener");
    change[1](); // apply 600 ms
    await sleep(50);
    const status = root.querySelector("#tx-status").textContent;
    assert.ok(String(status).includes("600"), `flash change did not retune status: ${status}`);
  } finally {
    if (handle) handle.stop();
    restore();
  }
  assert.deepEqual(errors, [], `runtime errors in the tx mount loop:\n${errors.join("\n")}`);
});
