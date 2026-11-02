// pixfer web transfer checkpoint — persist resumable transfer state so a
// dropped light link can be continued.
// Mirrors `pixfer-core/src/transfer_checkpoint.rs`.

// A resumable checkpoint for a transfer.
export class TransferCheckpoint {
  constructor({ sessionId = 0, totalChunks = 0, version = 1 } = {}) {
    this.sessionId = sessionId;
    this.totalChunks = totalChunks;
    this.delivered = new Set();
    this.version = version;
  }

  // A checkpoint is "complete" when every chunk has been delivered.
  isComplete() {
    return this.delivered.size === this.totalChunks;
  }

  // The number of chunks still pending.
  pendingCount() {
    return Math.max(0, this.totalChunks - this.delivered.size);
  }
}

// Create a fresh checkpoint for a transfer.
export function newCheckpoint(sessionId, totalChunks) {
  return new TransferCheckpoint({ sessionId, totalChunks, version: 1 });
}

// Manages transfer checkpoints keyed by session id.
export class CheckpointStore {
  constructor() {
    this.bySession = new Map();
  }

  // The number of stored checkpoints.
  size() {
    return this.bySession.size;
  }

  isEmpty() {
    return this.bySession.size === 0;
  }

  // Store a checkpoint (creates or overwrites for the session).
  save(cp) {
    this.bySession.set(cp.sessionId, cp);
  }

  // Retrieve a checkpoint for a session (null if absent).
  load(sessionId) {
    return this.bySession.has(sessionId) ? this.bySession.get(sessionId) : null;
  }

  // Remove a session's checkpoint.
  dropSession(sessionId) {
    this.bySession.delete(sessionId);
  }
}