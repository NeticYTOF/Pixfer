// Module smoke test: every browser-side ES module must parse and load.
// The other test files import the protocol modules, but the view mounts
// (tx.js / rx.js) and the light engine are only ever loaded by the BROWSER —
// a syntax error or bad import there kills the whole page script (every
// button dies) and no other test notices. This file loads each module.
// Run: node tests/module_smoke.test.mjs (from web/)

import test from "node:test";
import assert from "node:assert/strict";
import * as path from "node:path";
import * as url from "node:url";

const here = url.fileURLToPath(new URL(".", import.meta.url));
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
    // the view mounts must expose their entry points
    if (mod.endsWith("tx.js")) assert.equal(typeof imported.mountTx, "function");
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
  const fs = await import("node:fs/promises");
  const html = await fs.readFile(path.join(here, "..", "index.html"), "utf8");
  const ids = [
    "landing", "btn-tx", "btn-rx",
    "tx-view", "tx-canvas", "tx-camera", "tx-status", "tx-meta", "tx-progress",
    "tx-file", "tx-filename", "tx-back",
    "rx-view", "rx-video", "rx-canvas", "rx-emit", "rx-status", "rx-progress",
    "rx-lock", "rx-back",
  ];
  const missing = ids.filter((id) => !html.includes(`id="${id}"`));
  assert.deepEqual(missing, [], `index.html is missing ids: ${missing.join(", ")}`);
});
