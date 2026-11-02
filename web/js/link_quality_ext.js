// pixfer web extended link quality — a composite quality score combining
// throughput, jitter, and FEC failure rate.
// Mirrors `pixfer-core/src/link_quality_ext.rs`.

// The inputs to the quality score.
export class QualityInputsExt {
  constructor({ throughputBps = 0, capacityBps = 0, jitterMs = 0, fecCorrections = 0, frames = 0 } = {}) {
    this.throughputBps = throughputBps;
    this.capacityBps = capacityBps;
    this.jitterMs = jitterMs;
    this.fecCorrections = fecCorrections;
    this.frames = frames;
  }
}

// Computes a composite link-quality score in [0, 100].
export function qualityScoreExt(inputs) {
  const tpRatio = inputs.capacityBps <= 0 ? 0 : Math.min(inputs.throughputBps / inputs.capacityBps, 1.0);
  const jitterScore = 100 - Math.min(Math.max(inputs.jitterMs, 0), 100);
  const fecRate = inputs.frames === 0 ? 0 : Math.min(inputs.fecCorrections / inputs.frames, 1.0);
  const fecScore = (1 - fecRate) * 100;
  const score = tpRatio * 40 + (jitterScore / 100) * 30 + (fecScore / 100) * 30;
  return Math.min(Math.max(score, 0), 100);
}