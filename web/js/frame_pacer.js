// pixfer web frame pacer — fps-based pacing + jitter tracking.
// Mirrors `pixfer-core/src/frame_pacer.rs`.

const WIN = 64;

export class FramePacer {
  constructor(fps) {
    this.targetFps = Math.max(1, fps);
    this.framePeriodMs = 1000 / this.targetFps;
    this.intervals = [];
    this.jitterMs = 0;
  }

  // Minimum dwell (ms) a frame must stay on screen to be captured:
  // 2x the camera frame period, plus an optional margin.
  minDwellMs(marginMs = 0) {
    return Math.max(this.framePeriodMs * 2, this.framePeriodMs * 2) + marginMs;
  }

  // Total transfer time estimate for nChunks frames (s).
  transferTimeS(nChunks) {
    const perChunk = this.minDwellMs() + this.framePeriodMs;
    return (nChunks * perChunk) / 1000;
  }

  // Record an observed inter-frame interval (ms); updates rolling jitter.
  observeInterval(intervalMs) {
    this.intervals.push(intervalMs);
    if (this.intervals.length > WIN) this.intervals.shift();
    this.jitterMs = stdDev(this.intervals);
  }

  // Target fps this pacer was built for.
  targetFps() {
    return this.targetFps;
  }
}

// Population std-dev (ms); 0 for <2 samples.
function stdDev(values) {
  if (values.length < 2) return 0;
  const n = values.length;
  const mean = values.reduce((a, b) => a + b, 0) / n;
  const sq = values.reduce((a, v) => a + (v - mean) * (v - mean), 0) / n;
  return Math.sqrt(sq);
}