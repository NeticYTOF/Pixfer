// pixfer web extended transfer orchestrator — coordinates the full transfer
// lifecycle: planning, compressing, encoding, sending.
// Mirrors `pixfer-core/src/transfer_orchestrator_ext.rs`.

// A chunk of payload bytes.
export class ChunkExt {
  constructor({ idx = 0, data = [] } = {}) {
    this.idx = idx;
    this.data = data;
  }
}

// The outcome of an orchestrated transfer.
export function transferSuccess(bytes) {
  return { ok: true, bytes };
}

export function transferFailed(reason) {
  return { ok: false, reason };
}

// Coordinates a full transfer over the light link.
export class TransferOrchestratorExt {
  constructor(chunkBytes, zipThreshold) {
    this.chunkBytes = chunkBytes;
    this.zipThreshold = zipThreshold;
  }

  // The number of chunks a payload spans.
  chunkCount(dataLen) {
    return dataLen === 0 ? 0 : Math.ceil(dataLen / this.chunkBytes);
  }

  // Split `data` into chunks of at most `chunkBytes` bytes.
  slice(data) {
    const n = this.chunkCount(data.length);
    const out = [];
    for (let i = 0; i < n; i++) {
      const start = i * this.chunkBytes;
      const end = Math.min((i + 1) * this.chunkBytes, data.length);
      out.push(new ChunkExt({ idx: i, data: data.slice(start, end) }));
    }
    return out;
  }

  // Run a full transfer: slice -> encode each chunk -> send.
  // `encodeChunk` and `send` are caller-provided.
  run(data, encodeChunk, send) {
    for (const chunk of this.slice(data)) {
      const frame = encodeChunk(chunk.data);
      if (!frame) {
        return transferFailed(`encode chunk ${chunk.idx} failed`);
      }
      const r = send(frame);
      if (r && r.error) {
        return transferFailed(`send chunk ${chunk.idx} failed: ${r.error}`);
      }
    }
    return transferSuccess(data.length);
  }
}