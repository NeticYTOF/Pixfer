// pixfer web extended transfer session v2 — a stateful session object that
// tracks handshake, transfer, and completion with event logging.
// Mirrors `pixfer-core/src/transfer_session_v2.rs`.

// The role of a transfer session.
export const RoleV2 = Object.freeze({
  SENDER: "sender",
  RECEIVER: "receiver",
});

// The state of a transfer session.
export const SessionStateV2 = Object.freeze({
  IDLE: "idle",
  HANDSHAKING: "handshaking",
  TRANSFERRING: "transferring",
  AWAITING_ACK: "awaiting_ack",
  DONE: "done",
  FAILED: "failed",
});

// An event in the session log.
export class SessionEventV2 {
  constructor({ tMs = 0, kind = "", detail = "" } = {}) {
    this.tMs = tMs;
    this.kind = kind;
    this.detail = detail;
  }
}

// A stateful transfer session with event logging.
export class TransferSessionV2 {
  constructor(role, sessionId) {
    this.role = role;
    this.state = SessionStateV2.IDLE;
    this.sessionId = sessionId;
    this.events = [];
    this.chunksTotal = 0;
    this.chunksDelivered = 0;
  }

  // The session role.
  roleValue() {
    return this.role;
  }

  // The current session state.
  stateValue() {
    return this.state;
  }

  // The session id.
  sessionIdValue() {
    return this.sessionId;
  }

  // Begin the handshake (ping/pong).
  beginHandshake(tMs) {
    this.state = SessionStateV2.HANDSHAKING;
    this.events.push(new SessionEventV2({ tMs, kind: "handshake_begin", detail: "ping sent" }));
  }

  // Complete the handshake and move to transferring.
  beginTransfer(tMs, totalChunks) {
    this.state = SessionStateV2.TRANSFERRING;
    this.chunksTotal = totalChunks;
    this.events.push(
      new SessionEventV2({ tMs, kind: "transfer_begin", detail: `${totalChunks} chunks` }),
    );
  }

  // Mark a chunk as delivered.
  markChunkDelivered(tMs) {
    if (this.chunksDelivered < this.chunksTotal) this.chunksDelivered += 1;
    if (this.chunksDelivered === this.chunksTotal && this.chunksTotal > 0) {
      this.state = SessionStateV2.DONE;
      this.events.push(new SessionEventV2({ tMs, kind: "transfer_complete", detail: "all chunks delivered" }));
    }
  }

  // Mark the session failed.
  fail(tMs, reason) {
    this.state = SessionStateV2.FAILED;
    this.events.push(new SessionEventV2({ tMs, kind: "transfer_failed", detail: reason }));
  }

  // The number of delivered chunks.
  deliveredCount() {
    return this.chunksDelivered;
  }

  // The total chunk count.
  totalChunks() {
    return this.chunksTotal;
  }

  // The session event log.
  eventLog() {
    return this.events;
  }

  // The progress fraction.
  progress() {
    return this.chunksTotal === 0 ? 0 : this.chunksDelivered / this.chunksTotal;
  }
}