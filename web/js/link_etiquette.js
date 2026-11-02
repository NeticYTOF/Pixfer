// pixfer web link etiquette — a light-link state machine governing the
// social protocol: greeting, capability exchange, transfer window, and
// polite teardown.
// Mirrors `pixfer-core/src/link_etiquette.rs`.

export const ETIQUETTE_STATE = Object.freeze({
  IDLE: 'idle',
  GREETING: 'greeting',
  CAPABILITIES: 'capabilities',
  READY: 'ready',
  TRANSFERRING: 'transferring',
  CLOSING: 'closing',
  COOLDOWN: 'cooldown',
});

// Serialize capabilities into the 5-byte wire layout:
// [maxFps, multicolor, chunkBytesLo, chunkBytesHi, rsParity].
export function capsToBytes(caps) {
  return new Uint8Array([
    caps.maxFps & 0xff,
    caps.multicolor ? 1 : 0,
    caps.chunkBytes & 0xff,
    (caps.chunkBytes >> 8) & 0xff,
    caps.rsParity & 0xff,
  ]);
}

export function capsFromBytes(b) {
  if (!b || b.length !== 5) return null;
  return {
    maxFps: b[0],
    multicolor: b[1] !== 0,
    chunkBytes: b[2] | (b[3] << 8),
    rsParity: b[4],
  };
}

// The agreed operating point: the conservative (weaker) side wins.
export function agreeCaps(mine, peer) {
  return {
    maxFps: Math.min(mine.maxFps, peer.maxFps),
    multicolor: mine.multicolor && peer.multicolor,
    chunkBytes: Math.min(mine.chunkBytes, peer.chunkBytes),
    rsParity: Math.min(mine.rsParity, peer.rsParity),
  };
}

// The etiquette state machine.
export class Etiquette {
  constructor({ sessionId = 1, caps, timeoutMs = 5000, maxRetries = 3 } = {}) {
    this.state = ETIQUETTE_STATE.IDLE;
    this.sessionId = sessionId;
    this.caps = caps;
    this.timeoutMs = timeoutMs;
    this.retries = 0;
    this.maxRetries = maxRetries;
    this.baseBackoffMs = 200;
    this.maxBackoffMs = 4000;
    this.log = [];
    this.logSeq = 0;
    this.logCap = 64;
  }

  // Build the greeting frame (a Ping carrying capabilities) and move to
  // the Greeting state. Returns null when greeting is not allowed.
  greet() {
    if (this.state !== ETIQUETTE_STATE.IDLE && this.state !== ETIQUETTE_STATE.COOLDOWN) return null;
    this.retries = 0;
    this.state = ETIQUETTE_STATE.GREETING;
    return {
      kind: 'Ping',
      sessionId: this.sessionId,
      chunkIdx: 0,
      totalChunks: 0,
      payload: Array.from(capsToBytes(this.caps)),
    };
  }

  // Observe an event and advance the state machine. Returns the new state.
  onEvent(ev) {
    this.logEvent(ev);
    const S = ETIQUETTE_STATE;
    if (this.state === S.GREETING && ev === 'PeerGreeting') {
      this.state = S.CAPABILITIES;
    } else if (this.state === S.CAPABILITIES && ev === 'PeerCapabilities') {
      this.state = S.READY;
    } else if (this.state === S.READY && ev === 'TransferStart') {
      this.state = S.TRANSFERRING;
    } else if (this.state === S.TRANSFERRING && ev === 'TransferDone') {
      this.state = S.CLOSING;
    } else if (ev === 'PeerBye') {
      this.state = S.CLOSING;
    } else if (this.state === S.CLOSING && ev === 'PeerGreeting') {
      this.state = S.IDLE;
    } else if (
      (this.state === S.GREETING || this.state === S.CAPABILITIES || this.state === S.READY) &&
      ev === 'Timeout'
    ) {
      if (this.retries < this.maxRetries) {
        this.retries += 1;
        if (this.retries === this.maxRetries) this.state = S.COOLDOWN;
      } else {
        this.state = S.COOLDOWN;
      }
    } else if (ev === 'Error') {
      this.state = S.IDLE;
      this.retries = 0;
    }
    return this.state;
  }

  // Exponential backoff: base * 2^retries, capped.
  backoffMs() {
    const shift = Math.min(this.retries, 12);
    return Math.min(this.baseBackoffMs * (1 << shift), this.maxBackoffMs);
  }

  canTransfer() {
    return this.state === ETIQUETTE_STATE.READY;
  }

  retriesLeft() {
    return Math.max(0, this.maxRetries - this.retries);
  }

  logEvents() {
    return this.log.slice();
  }

  logLen() {
    return this.log.length;
  }

  reset() {
    this.state = ETIQUETTE_STATE.IDLE;
    this.retries = 0;
    this.log = [];
    this.logSeq = 0;
  }

  logEvent(ev) {
    this.logSeq += 1;
    this.log.push({ seq: this.logSeq, event: ev });
    if (this.log.length > this.logCap) this.log.shift();
  }
}