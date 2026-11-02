//! Deduplication for the transfer pipeline: skip files the receiver already
//! has, using the FNV-1a fingerprint as the identity. (Hilariously over-
//! engineered: a dedup gate for a link that already retransmits on NAKs.)

use std::collections::HashMap;

use crate::fingerprint::FileFingerprint;

/// The result of a dedup check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupDecision {
    /// The receiver lacks this file; transfer it.
    Transfer,
    /// The receiver already has a file with this fingerprint; skip it.
    Skip,
}

/// A set of fingerprints the receiver already holds.
#[derive(Debug, Clone)]
pub struct DedupSet {
    seen: HashMap<u64, u64>, // hash -> size
}

impl DedupSet {
    pub fn new() -> Self {
        Self {
            seen: HashMap::new(),
        }
    }

    /// Register a file the receiver already has.
    pub fn add(&mut self, fp: &FileFingerprint) {
        self.seen.insert(fp.hash, fp.size);
    }

    /// Does the receiver already have this file?
    pub fn has(&self, fp: &FileFingerprint) -> bool {
        self.seen.contains_key(&fp.hash)
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }
}

impl Default for DedupSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Decide whether a file should be transferred.
pub fn decide(fp: &FileFingerprint, rx: &DedupSet) -> DedupDecision {
    if rx.has(fp) {
        DedupDecision::Skip
    } else {
        DedupDecision::Transfer
    }
}

/// A pipeline-stage wrapper: only forward files that aren't already present
/// on the receiver.
pub struct DedupStage {
    rx: DedupSet,
    skipped: u32,
    transferred: u32,
}

impl DedupStage {
    pub fn new(rx: DedupSet) -> Self {
        Self {
            rx,
            skipped: 0,
            transferred: 0,
        }
    }

    /// Run one file through the dedup gate.
    pub fn process(&mut self, fp: &FileFingerprint) -> DedupDecision {
        match decide(fp, &self.rx) {
            DedupDecision::Skip => {
                self.skipped = self.skipped.saturating_add(1);
                DedupDecision::Skip
            }
            DedupDecision::Transfer => {
                self.transferred = self.transferred.saturating_add(1);
                DedupDecision::Transfer
            }
        }
    }

    pub fn skipped_count(&self) -> u32 {
        self.skipped
    }

    pub fn transferred_count(&self) -> u32 {
        self.transferred
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp_bytes(b: &str) -> FileFingerprint {
        FileFingerprint::of_bytes(b.as_bytes())
    }

    #[test]
    fn transfer_when_absent() {
        let rx = DedupSet::new();
        let fp = fp_bytes("unique-file.bin");
        assert_eq!(decide(&fp, &rx), DedupDecision::Transfer);
    }

    #[test]
    fn skip_when_present() {
        let mut rx = DedupSet::new();
        let fp = fp_bytes("already-here.txt");
        rx.add(&fp);
        assert_eq!(decide(&fp, &rx), DedupDecision::Skip);
    }

    #[test]
    fn dedup_stage_counts() {
        let mut rx = DedupSet::new();
        let mut stage = DedupStage::new(rx);
        let present = fp_bytes("present");
        let absent = fp_bytes("absent");
        // Manually mark "present" as already held.
        // (DedupStage takes ownership of rx, so register before moving.)
        // Reconstruct: build stage with a pre-populated set.
        let mut rx2 = DedupSet::new();
        rx2.add(&present);
        let mut stage2 = DedupStage::new(rx2);
        assert_eq!(stage2.process(&present), DedupDecision::Skip);
        assert_eq!(stage2.process(&absent), DedupDecision::Transfer);
        assert_eq!(stage2.skipped_count(), 1);
        assert_eq!(stage2.transferred_count(), 1);
    }

    #[test]
    fn set_len() {
        let mut rx = DedupSet::new();
        let a = fp_bytes("a");
        let b = fp_bytes("b");
        rx.add(&a);
        rx.add(&b);
        assert_eq!(rx.len(), 2);
        assert!(rx.has(&a));
    }
}
