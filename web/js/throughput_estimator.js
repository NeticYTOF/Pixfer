// pixfer web throughput estimator — an exponential moving average of
// observed bytes/sec, with a confidence threshold. Mirrors
// `pixfer-core/src/throughput_estimator.rs`.

export class ThroughputEstimator {
  constructor(alpha = 0.3, minSamplesForConfidence = 5) {
    if (alpha < 0 || alpha > 1) throw new RangeError("alpha must be in [0, 1]");
    this.alpha = alpha;
    this.minSamplesForConfidence = minSamplesForConfidence;
    this.estimate = 0;
    this.samples = 0;
  }

  observe(bytesPerSec) {
    this.samples += 1;
    if (this.samples === 1) {
      this.estimate = bytesPerSec;
    } else {
      this.estimate =
        this.alpha * bytesPerSec + (1 - this.alpha) * this.estimate;
    }
  }

  isConfident() {
    return this.samples >= this.minSamplesForConfidence;
  }

  sampleCount() {
    return this.samples;
  }

  value() {
    return this.estimate;
  }
}