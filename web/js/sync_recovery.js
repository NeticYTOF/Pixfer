// pixfer web sync recovery — re-acquire the sync word after a drop.
// Mirrors `pixfer-core/src/sync_recovery.rs`.

// State names.
export const RecoveryState = Object.freeze({
  LOCKED: "locked",
  ACQUIRING: "acquiring",
  FAILED: "failed",
});

// A bounded scanner that re-acquires the sync word.
export class SyncRecovery {
  // pattern: array of 3-bit symbols; maxScans: budget before giving up.
  constructor(pattern, maxScans = 10) {
    this.pattern = pattern.slice();
    this.maxScans = maxScans;
    this.scans = 0;
    this.state = RecoveryState.ACQUIRING;
  }

  // Scan one candidate window; returns true on match.
  scan(candidate) {
    if (this.state === RecoveryState.LOCKED) {
      return findMatch(this.pattern, candidate) !== -1;
    }
    if (this.state === RecoveryState.FAILED) {
      return false;
    }
    const matched = findMatch(this.pattern, candidate) !== -1;
    this.scans += 1;
    if (matched) {
      this.state = RecoveryState.LOCKED;
      this.scans = 0;
    } else if (this.scans >= this.maxScans) {
      this.state = RecoveryState.FAILED;
    }
    return matched;
  }

  reset() {
    this.state = RecoveryState.ACQUIRING;
    this.scans = 0;
  }

  getState() {
    return this.state;
  }

  scanCount() {
    return this.scans;
  }
}

// Find `pattern` in `stream` (3-bit symbols). Returns index or -1.
function findMatch(pattern, stream) {
  const n = pattern.length;
  if (n === 0) return 0;
  for (let i = 0; i + n <= stream.length; i++) {
    let ok = true;
    for (let j = 0; j < n; j++) {
      if ((stream[i + j] & 0b111) !== (pattern[j] & 0b111)) {
        ok = false;
        break;
      }
    }
    if (ok) return i;
  }
  return -1;
}