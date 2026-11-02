// pixfer web link throughput probe — measure the achievable throughput of
// the light link by probing it with known-size payloads.
// Mirrors `pixfer-core/src/link_throughput_probe.rs`.

// The result of a single throughput probe.
export class ThroughputProbeResult {
  constructor({ bytes = 0, elapsedMs = 0, bps = 0 } = {}) {
    this.bytes = bytes;
    this.elapsedMs = elapsedMs;
    this.bps = bps;
  }
}

// Probes a link's throughput using the frame pacer's timing model.
export class LinkThroughputProbe {
  constructor(probeBytes, rounds) {
    this.probeBytes = probeBytes;
    this.rounds = rounds;
  }

  // Run the probe and return a measured-throughput result.
  // `send` returns the elapsed ms for a round. `state` carries mutable
  // closure state (mirrors Rust's &mut FnMut).
  run(send, state) {
    let totalMs = 0;
    for (let i = 0; i < this.rounds; i++) {
      totalMs += send(this.probeBytes, state);
    }
    const bps = totalMs === 0 ? Infinity : (this.probeBytes * this.rounds) / (totalMs / 1000);
    return new ThroughputProbeResult({
      bytes: this.probeBytes * this.rounds,
      elapsedMs: totalMs,
      bps,
    });
  }
}