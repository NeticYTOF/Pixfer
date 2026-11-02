// pixfer web transfer rollback — transactional transfer state restore.
// Mirrors `pixfer-core/src/transfer_rollback.rs`.

// A snapshot of the receiver's reassembly state before a transfer.
class ReassemblySnapshot {
  constructor(received, totalChunks, sessionId, complete) {
    this.received = received;
    this.totalChunks = totalChunks;
    this.sessionId = sessionId;
    this.complete = complete;
  }
}

// A rollback manager for in-progress transfers.
export class TransferRollback {
  constructor() {
    this.snapshot = null;
    this.active = false;
  }

  // Capture the current reassembly state as a snapshot.
  // reassembler: { receivedIndices(), totalChunks(), sessionId(), isComplete() }
  begin(reassembler) {
    this.snapshot = new ReassemblySnapshot(
      reassembler.receivedIndices(),
      reassembler.totalChunks(),
      reassembler.sessionId(),
      reassembler.isComplete(),
    );
    this.active = true;
  }

  // Build a fresh reassembler in the snapshot's state.
  restored() {
    if (!this.snapshot) return null;
    const s = this.snapshot;
    // Return a simple object with the snapshot's state.
    return {
      received: s.received.slice(),
      totalChunks: s.totalChunks,
      sessionId: s.sessionId,
      complete: s.complete,
    };
  }

  // Mark the transfer as rolled back (clears the active flag).
  rollback() {
    if (!this.active) return null;
    const restored = this.restored();
    this.active = false;
    this.snapshot = null;
    return restored;
  }

  // Mark the transfer as successfully completed (no rollback needed).
  commit() {
    this.active = false;
    this.snapshot = null;
  }

  // Whether a transfer is currently in progress.
  isActive() {
    return this.active;
  }
}