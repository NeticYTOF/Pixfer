// pixfer web link probe — Ping/Pong handshake + RTT measurement.
// Mirrors `pixfer-core/src/link_probe.rs`.

// A ping/pong probe result.
class ProbeResult {
  constructor(responded, rttMs, payload) {
    this.responded = responded;
    this.rttMs = rttMs;
    this.payload = payload;
  }
}

// A link liveness + latency probe.
export class LinkProbe {
  constructor(timeoutMs = 5000) {
    this.timeoutMs = timeoutMs;
  }

  // Run the probe: send a ping, poll for a pong up to the timeout.
  // sendPing(payload) and recvPong() are provided by the caller; nowMs
  // supplies the current time. Returns a ProbeResult.
  run({ sendPing, recvPong, nowMs, payload }) {
    const t0 = nowMs();
    sendPing(payload);
    const MAX_POLLS = 10000;
    let result = null;
    let polls = 0;
    while (result === null && polls < MAX_POLLS) {
      polls++;
      result = recvPong();
      if (nowMs() - t0 >= this.timeoutMs) break;
    }
    if (result !== null) {
      return new ProbeResult(true, nowMs() - t0, result);
    }
    return new ProbeResult(false, 0, payload);
  }

  // The probe's timeout (ms).
  timeout() {
    return this.timeoutMs;
  }
}