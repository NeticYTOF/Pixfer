//! File fingerprinting: a fast, non-cryptographic 64-bit FNV-1a fingerprint
//! for identity and dedup. (Hilariously over-engineered: a "hash" that
//! isn't a hash, for a link that carries whole files by photons.)

use std::collections::HashMap;

/// A file's identity: a 64-bit FNV-1a fingerprint + size + mtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFingerprint {
    /// 64-bit FNV-1a over the file's bytes.
    pub hash: u64,
    /// File size in bytes.
    pub size: u64,
    /// Modification time (seconds) if known, else 0.
    pub mtime: i64,
}

impl FileFingerprint {
    /// Compute a fingerprint for a byte slice.
    pub fn of_bytes(data: &[u8]) -> Self {
        Self {
            hash: fnv1a64(data),
            size: data.len() as u64,
            mtime: 0,
        }
    }

    /// Stable ordering for index keys.
    pub fn key(&self) -> u64 {
        self.hash
    }
}

/// 64-bit FNV-1a over a byte slice (not a std fn, so we implement it).
pub fn fnv1a64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001B3;
    let mut h: u64 = OFFSET;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// A dedup index mapping fingerprint -> metadata.
#[derive(Debug, Clone)]
pub struct FingerprintIndex {
    /// Map: fingerprint hash -> stored metadata.
    entries: HashMap<u64, FileFingerprint>,
}

impl FingerprintIndex {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Record a fingerprint (upsert by hash).
    pub fn insert(&mut self, fp: FileFingerprint) {
        self.entries.insert(fp.hash, fp);
    }

    pub fn contains(&self, hash: u64) -> bool {
        self.entries.contains_key(&hash)
    }

    pub fn find(&self, hash: u64) -> Option<&FileFingerprint> {
        self.entries.get(&hash)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for FingerprintIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_known_vector() {
        // FNV-1a 64-bit of "123456789" is a known reference value.
        let v = fnv1a64(b"123456789");
        assert_eq!(v, 0x6d5573923c6cdfc);
    }

    #[test]
    fn fingerprint_of_bytes() {
        let fp = FileFingerprint::of_bytes(b"hello");
        assert_eq!(fp.size, 5);
        assert_eq!(fp.hash, fnv1a64(b"hello"));
    }

    #[test]
    fn same_bytes_same_fingerprint() {
        let a = FileFingerprint::of_bytes(b"pixfer");
        let b = FileFingerprint::of_bytes(b"pixfer");
        assert_eq!(a.hash, b.hash);
    }

    #[test]
    fn different_bytes_different_fingerprint() {
        let a = FileFingerprint::of_bytes(b"pixfer");
        let b = FileFingerprint::of_bytes(b"pixfers");
        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn index_dedup() {
        let mut idx = FingerprintIndex::new();
        let fp = FileFingerprint::of_bytes(b"pixfer");
        idx.insert(fp);
        assert!(idx.contains(fp.hash));
        assert_eq!(idx.find(fp.hash).unwrap().size, 6);
        assert_eq!(idx.len(), 1);
    }

    #[test]
    fn index_upsert() {
        let mut idx = FingerprintIndex::new();
        let fp1 = FileFingerprint::of_bytes(b"a");
        let fp2 = FileFingerprint::of_bytes(b"b");
        idx.insert(fp1);
        idx.insert(fp2);
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn empty_index() {
        let idx = FingerprintIndex::new();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
    }
}
