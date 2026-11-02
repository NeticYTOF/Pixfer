//! Link fingerprint: a stable identifier for the light link, derived
//! from the session's color-calibration signature. (Hilariously
//! over-engineered: a "fingerprint" for a link that physically only
//! moves a few KB/s.)

use crate::fingerprint::fnv1a64;

/// A stable 64-bit fingerprint for one light link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinkFingerprint {
    pub value: u64,
}

/// Compute the link fingerprint from a session's calibration signature.
pub fn compute_link_fingerprint(session_id: u16, total_chunks: u32) -> LinkFingerprint {
    let mut input = Vec::new();
    input.extend_from_slice(&session_id.to_le_bytes());
    input.extend_from_slice(&total_chunks.to_le_bytes());
    input.extend_from_slice(b"pixfer-link");
    LinkFingerprint {
        value: fnv1a64(&input),
    }
}

/// Two links are considered the same if their fingerprints match.
pub fn same_link(a: &LinkFingerprint, b: &LinkFingerprint) -> bool {
    a.value == b.value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::fnv1a64;

    #[test]
    fn fingerprint_is_deterministic() {
        let f1 = compute_link_fingerprint(42, 10);
        let f2 = compute_link_fingerprint(42, 10);
        assert_eq!(f1, f2);
        assert!(same_link(&f1, &f2));
    }

    #[test]
    fn different_sessions_differ() {
        let f1 = compute_link_fingerprint(1, 5);
        let f2 = compute_link_fingerprint(2, 5);
        assert!(!same_link(&f1, &f2));
    }

    #[test]
    fn different_chunk_counts_differ() {
        let f1 = compute_link_fingerprint(1, 5);
        let f2 = compute_link_fingerprint(1, 6);
        assert!(!same_link(&f1, &f2));
    }

    #[test]
    fn value_is_fnv1a64_of_input() {
        let f = compute_link_fingerprint(42, 10);
        // Reconstruct the expected hash.
        let mut input = Vec::new();
        input.extend_from_slice(&42u16.to_le_bytes());
        input.extend_from_slice(&10u32.to_le_bytes());
        input.extend_from_slice(b"pixfer-link");
        assert_eq!(f.value, fnv1a64(&input));
    }

    #[test]
    fn hash_consistent() {
        let f = compute_link_fingerprint(7, 3);
        // Hash trait is derived; the value is the FNV-1a 64-bit.
        assert!(f.value != 0);
    }
}
