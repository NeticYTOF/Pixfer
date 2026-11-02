// Behavioral tests for the new web modules: link_fingerprint,
// chunk_reorder, transfer_checksum.
// Run: node tests/webmodules7.test.mjs (from web/)

import { computeLinkFingerprint, sameLink } from "../js/link_fingerprint.js";
import { ChunkReorder } from "../js/chunk_reorder.js";
import { fnv1a64Checksum, verifyTransfer, transferChecksum } from "../js/transfer_checksum.js";

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

// ---------- link_fingerprint.js ----------
{
  const f1 = computeLinkFingerprint(42, 10);
  const f2 = computeLinkFingerprint(42, 10);
  check("fingerprint: deterministic", f1.value === f2.value);
  check("fingerprint: same link", sameLink(f1, f2));

  const g1 = computeLinkFingerprint(1, 5);
  const g2 = computeLinkFingerprint(2, 5);
  check("fingerprint: different sessions differ", !sameLink(g1, g2));

  const h1 = computeLinkFingerprint(1, 5);
  const h2 = computeLinkFingerprint(1, 6);
  check("fingerprint: different chunk counts differ", !sameLink(h1, h2));

  check("fingerprint: value nonzero", f1.value !== 0n, `got ${f1.value}`);
}

// ---------- chunk_reorder.js ----------
{
  const r = new ChunkReorder(4);
  r.receive(2);
  r.receive(0);
  r.receive(3);
  const emitted = r.drainInOrder();
  check("reorder: only in-order prefix emitted", JSON.stringify(emitted) === JSON.stringify([0]), `got ${JSON.stringify(emitted)}`);
  check("reorder: next missing", r.nextMissing() === 1);
  r.receive(1);
  const emitted2 = r.drainInOrder();
  check("reorder: rest emitted", JSON.stringify(emitted2) === JSON.stringify([1, 2, 3]), `got ${JSON.stringify(emitted2)}`);
  check("reorder: complete", r.complete());
  check("reorder: no missing", r.nextMissing() === null);

  const r2 = new ChunkReorder(3);
  r2.receive(0);
  check("reorder: in-order passthrough", JSON.stringify(r2.drainInOrder()) === JSON.stringify([0]));
  r2.receive(1);
  check("reorder: second in-order", JSON.stringify(r2.drainInOrder()) === JSON.stringify([1]));
  r2.receive(2);
  check("reorder: third in-order", JSON.stringify(r2.drainInOrder()) === JSON.stringify([2]));
  check("reorder: complete after all", r2.complete());

  const r3 = new ChunkReorder(3);
  r3.receive(0);
  r3.receive(0);
  check("reorder: duplicate ignored", r3.receivedCount() === 1);

  const r4 = new ChunkReorder(3);
  r4.receive(5);
  check("reorder: out-of-range ignored", r4.receivedCount() === 0);

  const r5 = new ChunkReorder(4);
  r5.receive(0);
  r5.receive(1);
  r5.drainInOrder();
  check("reorder: emitted count", r5.emittedCount() === 2);
  check("reorder: received cleared after drain", r5.receivedCount() === 0);

  const r6 = new ChunkReorder(0);
  check("reorder: total 0 never complete", !r6.complete());
  check("reorder: total 0 no missing", r6.nextMissing() === null);
}

// ---------- transfer_checksum.js ----------
{
  // FNV-1a 64-bit known vector for "123456789".
  const data = Buffer.from("123456789");
  check("checksum: fnv1a64 known vector",
    fnv1a64Checksum(data) === 0x6d5573923c6cdfcn,
    `got ${fnv1a64Checksum(data)}`);

  const hello = Buffer.from("hello");
  const expected = fnv1a64Checksum(hello);
  const res = verifyTransfer(expected, hello);
  check("checksum: match", res.matched === true);
  check("checksum: actual equals expected", res.actual === expected);

  const mismatch = verifyTransfer(fnv1a64Checksum(Buffer.from("world")), Buffer.from("hello"));
  check("checksum: mismatch", mismatch.matched === false);

  const chunks = [Buffer.from([1, 2, 3]), Buffer.from([4, 5, 6])];
  const sum = transferChecksum(chunks);
  const flat = Buffer.from([1, 2, 3, 4, 5, 6]);
  check("checksum: chunk concatenation", sum === fnv1a64Checksum(flat));

  check("checksum: empty chunks", transferChecksum([]) === fnv1a64Checksum(Buffer.alloc(0)));
}

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.log("Failures:", failures.join(", "));
  process.exit(1);
}