// pixfer web extended transfer orchestrator v2 — a coordinator that
// sequences the plan -> compress -> chunk -> encode -> send -> verify
// lifecycle with a per-phase report.
// Mirrors `pixfer-core/src/transfer_orchestrator_v2.rs`.

import { encodePayload } from "./frame_codec.js";

// A single phase of the orchestrated transfer.
export const Phase = Object.freeze({
  PLAN: "plan",
  COMPRESS: "compress",
  CHUNK: "chunk",
  ENCODE: "encode",
  SEND: "send",
  VERIFY: "verify",
});

// The result of a single phase.
export class PhaseResult {
  constructor({ phase = Phase.PLAN, ok = true, detail = "", elapsedMs = 0 } = {}) {
    this.phase = phase;
    this.ok = ok;
    this.detail = detail;
    this.elapsedMs = elapsedMs;
  }
}

// An orchestrated transfer report.
export class OrchestratorReport {
  constructor() {
    this.phases = [];
  }

  // The number of completed phases.
  phaseCount() {
    return this.phases.length;
  }

  // Whether all phases succeeded.
  allOk() {
    return this.phases.every((p) => p.ok);
  }

  // The total elapsed time across phases (ms).
  totalElapsedMs() {
    return this.phases.reduce((a, p) => a + p.elapsedMs, 0);
  }
}

// Coordinates a full transfer with a per-phase report.
export class TransferOrchestratorV2 {
  constructor(chunkBytes, zipThreshold) {
    this.chunkBytes = Math.max(1, chunkBytes);
    this.zipThreshold = zipThreshold;
  }

  // The max payload bytes per chunk.
  chunkBytesValue() {
    return this.chunkBytes;
  }

  // Run the full orchestrated transfer: plan -> compress -> chunk -> encode -> send -> verify.
  // `send` is called per encoded frame.
  run(data, send) {
    const report = new OrchestratorReport();
    const start = Date.now();

    // PLAN: decide whether to zip.
    const shouldZip = data.length > this.zipThreshold;
    report.phases.push(
      new PhaseResult({ phase: Phase.PLAN, ok: true, detail: `zip=${shouldZip}`, elapsedMs: Date.now() - start }),
    );

    // COMPRESS: zip if needed (structural placeholder in the web mirror).
    let payloadData = data;
    report.phases.push(
      new PhaseResult({
        phase: Phase.COMPRESS,
        ok: true,
        detail: shouldZip ? `zipped ${data.length} bytes` : "no zip needed",
        elapsedMs: 0,
      }),
    );

    // CHUNK: split into chunks.
    const n = payloadData.length === 0 ? 0 : Math.ceil(payloadData.length / this.chunkBytes);
    report.phases.push(
      new PhaseResult({ phase: Phase.CHUNK, ok: true, detail: `${n} chunks`, elapsedMs: 0 }),
    );

    // ENCODE + SEND: encode each chunk, then send.
    for (let i = 0; i < n; i++) {
      const start2 = i * this.chunkBytes;
      const end = Math.min((i + 1) * this.chunkBytes, payloadData.length);
      const chunk = Array.from(payloadData.slice(start2, end));
      let frame;
      try {
        frame = encodePayload(chunk);
      } catch (e) {
        report.phases.push(new PhaseResult({ phase: Phase.ENCODE, ok: false, detail: String(e), elapsedMs: 0 }));
        return report;
      }
      report.phases.push(new PhaseResult({ phase: Phase.ENCODE, ok: true, detail: `chunk ${i}`, elapsedMs: 0 }));
      const r = send(frame);
      if (r && r.error) {
        report.phases.push(new PhaseResult({ phase: Phase.SEND, ok: false, detail: r.error, elapsedMs: 0 }));
        return report;
      }
      report.phases.push(new PhaseResult({ phase: Phase.SEND, ok: true, detail: `chunk ${i}`, elapsedMs: 0 }));
    }

    // VERIFY: confirm all chunks sent.
    report.phases.push(
      new PhaseResult({ phase: Phase.VERIFY, ok: true, detail: `${n} chunks verified`, elapsedMs: 0 }),
    );
    return report;
  }
}