// pixfer web session id manager — allocate and track the u16 session
// identifiers the wire-format header carries.
// Mirrors `pixfer-core/src/session_id_manager.rs`.

// A registered session entry.
export class SessionEntryExt {
  constructor({ sessionId = 0, createdMs = 0 } = {}) {
    this.sessionId = sessionId;
    this.createdMs = createdMs;
  }
}

// Manages allocation of the 16-bit session id space.
export class SessionIdManager {
  constructor() {
    this.used = new Set();
    this.next = 1;
    this.allocations = 0;
  }

  // The number of allocations so far.
  allocationCount() {
    return this.allocations;
  }

  // The number of live session ids.
  liveCount() {
    return this.used.size;
  }

  // Allocate a fresh session id, skipping ids already in use.
  allocate() {
    let id = this.next;
    while (this.used.has(id)) {
      id = (id + 1) & 0xffff;
    }
    this.used.add(id);
    this.allocations += 1;
    this.next = (id + 1) & 0xffff;
    return id;
  }

  // Release a session id back to the pool.
  release(sessionId) {
    this.used.delete(sessionId);
  }

  // Whether a session id is currently allocated.
  isLive(sessionId) {
    return this.used.has(sessionId);
  }

  // The smallest live session id (null if none).
  minLiveId() {
    for (const id of this.used) {
      return id;
    }
    return null;
  }
}