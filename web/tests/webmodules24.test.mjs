// Behavioral tests for web/js/link_etiquette.js — the light-link
// etiquette state machine.
// Run: node tests/webmodules24.test.mjs (from web/)

import {
  Etiquette,
  ETIQUETTE_STATE,
  capsToBytes,
  capsFromBytes,
  agreeCaps,
} from "../js/link_etiquette.js";

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

function caps() {
  return { maxFps: 30, multicolor: true, chunkBytes: 223, rsParity: 16 };
}

// ---------- capabilities ----------
{
  const c = caps();
  const bytes = capsToBytes(c);
  const back = capsFromBytes(bytes);
  check("etiq_caps: capabilities serialize roundtrip",
    back.maxFps === 30 && back.multicolor === true && back.chunkBytes === 223 && back.rsParity === 16);

  check("etiq_caps: bad length rejected", capsFromBytes([1, 2, 3]) === null);
  check("etiq_caps: null rejected", capsFromBytes(null) === null);

  const peer = { maxFps: 15, multicolor: false, chunkBytes: 512, rsParity: 8 };
  const agreed = agreeCaps(c, peer);
  check("etiq_caps: agree takes conservative side",
    agreed.maxFps === 15 && agreed.multicolor === false &&
    agreed.chunkBytes === 223 && agreed.rsParity === 8);
}

// ---------- state machine ----------
{
  const e = new Etiquette({ sessionId: 1, caps: caps(), timeoutMs: 5000, maxRetries: 3 });
  check("etiq_sm: starts idle", e.state === ETIQUETTE_STATE.IDLE && !e.canTransfer());

  const g = e.greet();
  check("etiq_sm: greet builds Ping with caps",
    g.kind === "Ping" && g.sessionId === 1 && g.payload.length === 5);
  check("etiq_sm: greet moves to greeting", e.state === ETIQUETTE_STATE.GREETING);
  check("etiq_sm: caps payload decodes", capsFromBytes(g.payload).chunkBytes === 223);

  check("etiq_sm: peer greeting advances to capabilities",
    e.onEvent("PeerGreeting") === ETIQUETTE_STATE.CAPABILITIES);
  check("etiq_sm: peer capabilities opens transfer window",
    e.onEvent("PeerCapabilities") === ETIQUETTE_STATE.READY && e.canTransfer());
  check("etiq_sm: transfer start", e.onEvent("TransferStart") === ETIQUETTE_STATE.TRANSFERRING);
  check("etiq_sm: transfer done begins teardown", e.onEvent("TransferDone") === ETIQUETTE_STATE.CLOSING);

  const e2 = new Etiquette({ caps: caps() });
  e2.greet();
  check("etiq_sm: peer bye from any state", e2.onEvent("PeerBye") === ETIQUETTE_STATE.CLOSING);

  const e3 = new Etiquette({ caps: caps() });
  check("etiq_sm: transfer start requires ready", e3.onEvent("TransferStart") === ETIQUETTE_STATE.IDLE);
}

// ---------- retries, backoff, cooldown ----------
{
  const e = new Etiquette({ caps: caps(), maxRetries: 3 });
  e.greet();
  check("etiq_retry: first timeout keeps greeting",
    e.onEvent("Timeout") === ETIQUETTE_STATE.GREETING && e.retriesLeft() === 2);
  check("etiq_retry: backoff doubles", e.backoffMs() === 400);
  e.onEvent("Timeout");
  check("etiq_retry: second timeout", e.backoffMs() === 800 && e.retriesLeft() === 1);
  check("etiq_retry: third timeout enters cooldown",
    e.onEvent("Timeout") === ETIQUETTE_STATE.COOLDOWN && e.retriesLeft() === 0);
  const g = e.greet();
  check("etiq_retry: cooldown allows re-greeting",
    g !== null && e.state === ETIQUETTE_STATE.GREETING && e.retriesLeft() === 3);
  check("etiq_retry: backoff capped", e.backoffMs() === 200);

  const e2 = new Etiquette({ caps: caps(), maxRetries: 3 });
  e2.greet();
  e2.onEvent("Timeout");
  e2.onEvent("Timeout");
  e2.onEvent("Timeout");
  check("etiq_backoff: 200 * 2^3 = 1600", e2.backoffMs() === 1600);

  const e3 = new Etiquette({ caps: caps() });
  e3.greet();
  check("etiq_sm: error returns to idle",
    e3.onEvent("Error") === ETIQUETTE_STATE.IDLE && e3.retriesLeft() === 3);
}

// ---------- event log ----------
{
  const e = new Etiquette({ caps: caps() });
  e.greet();
  e.onEvent("PeerGreeting");
  e.onEvent("PeerCapabilities");
  const log = e.logEvents();
  check("etiq_log: records events in order",
    log.length === 2 && log[0].seq === 1 && log[1].seq === 2 &&
    log[0].event === "PeerGreeting" && log[1].event === "PeerCapabilities");

  const e2 = new Etiquette({ caps: caps() });
  for (let i = 0; i < 40; i++) {
    e2.greet();
    e2.onEvent("PeerGreeting");
    e2.onEvent("PeerCapabilities");
    e2.onEvent("TransferStart");
    e2.onEvent("TransferDone");
    e2.onEvent("PeerBye");
  }
  check("etiq_log: caps at 64 entries", e2.logLen() === 64);
  const log2 = e2.logEvents();
  let mono = true;
  for (let i = 1; i < log2.length; i++) {
    if (log2[i].seq <= log2[i - 1].seq) mono = false;
  }
  check("etiq_log: retained window seq monotonic", mono);

  const e3 = new Etiquette({ caps: caps() });
  e3.greet();
  e3.onEvent("PeerGreeting");
  e3.reset();
  check("etiq_reset: clears state retries and log",
    e3.state === ETIQUETTE_STATE.IDLE && e3.retriesLeft() === 3 && e3.logLen() === 0);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}