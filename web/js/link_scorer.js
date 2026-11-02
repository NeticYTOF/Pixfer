// pixfer web link scorer — composite 0-100 quality score. Mirrors
// `pixfer-core/src/link_scorer.rs`.

// Sub-scores (each 0..=100).

// FEC sub-score: 100 * (1 - failed/(corrected+failed)); 100 if no fec events.
function fecSubscore(fecCorrected, fecFailed) {
  const total = fecCorrected + fecFailed;
  if (total === 0) return 100;
  const failedRatio = fecFailed / total;
  return Math.max(0, Math.min(100, 100 * (1 - failedRatio)));
}

// Retry sub-score: 100 at 0 retries/sample, 0 at 5+ retries/sample.
function retrySubscore(retries, samples) {
  if (samples === 0) return 100;
  const perSample = retries / samples;
  return Math.max(0, Math.min(100, 100 * (1 - Math.min(1, perSample / 5))));
}

// Jitter sub-score: 100 at 0 ms, 0 at 200 ms, linear between.
function jitterSubscore(jitterMs) {
  return Math.max(0, Math.min(100, 100 * (1 - Math.min(1, jitterMs / 200))));
}

// Throughput sub-score: 0 at ratio<=0.25, 100 at ratio>=2, 50 at ratio=1.
function throughputSubscore(tpMbps, floorMbps) {
  if (tpMbps <= 0 || floorMbps <= 0) return 50;
  const ratio = tpMbps / floorMbps;
  if (ratio >= 2.0) return 100;
  if (ratio <= 0.25) return 0;
  const t = (ratio - 0.25) / 1.75;
  return Math.max(0, Math.min(100, 100 * t));
}

// Composite weighted score.
export function linkScore({
  fecCorrected = 0,
  fecFailed = 0,
  retries = 0,
  syncLoss = 0,
  samples = 0,
  jitterMs = 0,
  throughputMbps = 0,
  floorMbps = 1.0,
  weights = { fec: 0.35, retry: 0.25, jitter: 0.2, throughput: 0.2 },
} = {}) {
  const totalW = weights.fec + weights.retry + weights.jitter + weights.throughput;
  if (totalW <= 0 || samples === 0) return 0;
  const fec = fecSubscore(fecCorrected, fecFailed);
  const retry = retrySubscore(retries, samples);
  const jitter = jitterSubscore(jitterMs);
  const tp = throughputSubscore(throughputMbps, floorMbps);
  const composite =
    (weights.fec * fec + weights.retry * retry + weights.jitter * jitter + weights.throughput * tp) / totalW;
  return Math.max(0, Math.min(100, composite));
}

// Score from a LinkStats-shaped snapshot.
export function linkScoreFromStats(stats, floorMbps = 1.0) {
  const ratio = stats.chunksTotal === 0 ? 0 : stats.chunksDelivered / stats.chunksTotal;
  const fec = Math.max(0, Math.min(100, ratio * 100));
  const tp = stats.elapsedMs === 0 ? 0 : stats.bytesSent / (stats.elapsedMs / 1000) / 1048576;
  return linkScore({
    fecCorrected: 0,
    fecFailed: 0,
    retries: stats.retries,
    samples: stats.chunksTotal,
    jitterMs: 0,
    throughputMbps: tp,
    floorMbps,
  });
}