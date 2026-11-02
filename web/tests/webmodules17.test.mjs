// Behavioral tests for the new web modules: session_id_manager,
// payload_crc_ext.
// Run: node tests/webmodules17.test.mjs (from web/)

import { SessionIdManager, SessionEntryExt } from "../js/session_id_manager.js";
import { PayloadCrcExt, ExtChecksumResult } from "../js/payload_crc_ext.js";

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

const strToBytes = (s) => {
  const b = new Array(s.length);
  for (let i = 0; i < s.length; i++) b[i] = s.charCodeAt(i);
  return b;
};

// ---------- session_id_manager.js ----------
{
  const m = new SessionIdManager();
  check("session: starts empty", m.allocationCount() === 0 && m.liveCount() === 0 && m.minLiveId() === null);

  const m2 = new SessionIdManager();
  const a = m2.allocate();
  const b = m2.allocate();
  const c = m2.allocate();
  check("session: allocates sequential ids", a === 1 && b === 2 && c === 3);
  check("session: live count and allocations", m2.liveCount() === 3 && m2.allocationCount() === 3);
  check("session: isLive reflects state", m2.isLive(1) && m2.isLive(3) && !m2.isLive(4));

  const m3 = new SessionIdManager();
  const x = m3.allocate();
  const y = m3.allocate();
  m3.release(y);
  check("session: release frees id", !m3.isLive(y) && m3.liveCount() === 1);
  const z = m3.allocate();
  check("session: next allocation skips used", z !== y && m3.isLive(z));

  const m4 = new SessionIdManager();
  // Force the wrap: mark every id used except 1.
  for (let i = 0; i <= 0xffff; i++) {
    if (i !== 1) m4.used.add(i);
  }
  const id = m4.allocate();
  check("session: wraps at u16 max", id === 1 && m4.isLive(1));

  const m5 = new SessionIdManager();
  const s1 = m5.allocate();
  const s2 = m5.allocate();
  check("session: min live id tracks", m5.minLiveId() === s1);
  m5.release(s1);
  check("session: min live id advances after release", m5.minLiveId() === s2);

  const entry = new SessionEntryExt({ sessionId: 9, createdMs: 123 });
  check("session: entry shape", entry.sessionId === 9 && entry.createdMs === 123);
}

// ---------- payload_crc_ext.js ----------
{
  const c = new PayloadCrcExt();
  const v123456789 = strToBytes("123456789");
  check("crc: crc32 known vector", c.crc32(v123456789) === 0xcbf43926, `got ${c.crc32(v123456789).toString(16)}`);
  check("crc: adler32 known vector", c.adler32(strToBytes("Wikipedia")) === 0x11e60398, `got ${c.adler32(strToBytes("Wikipedia")).toString(16)}`);
  check("crc: fnv1a64 known vector", c.fnv1a64(v123456789) === 492395637191921148);

  const r1 = c.computeNamed("crc32", v123456789);
  check("crc: named result shape", r1 instanceof ExtChecksumResult && r1.algorithm === "crc32" && r1.value === 0xcbf43926);
  const r2 = c.computeNamed("adler32", strToBytes("Wikipedia"));
  check("crc: adler named result", r2.algorithm === "adler32" && r2.value === 0x11e60398);

  check("crc: empty payload checksums", c.crc32([]) === 0 && c.adler32([]) === 1);
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}