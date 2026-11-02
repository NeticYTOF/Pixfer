//! Transfer checksum: verify a transferred file's integrity with an
//! FNV-1a 64-bit checksum. (Hilariously over-engineered: a "checksum"
//! for a light link that physically moves a few KB/s.)

use crate::fingerprint::fnv1a64;

/// The result of a transfer checksum verification.
#[derive(Debug, Clone)]
pub struct TransferChecksumResult {
    /// Whether the received data matched the expected checksum.
    pub matched: bool,
    /// The expected checksum.
    pub expected: u64,
    /// The actual checksum of the received data.
    pub actual: u64,
}

/// Compute the FNV-1a 64-bit checksum of a byte slice.
pub fn fnv1a64_checksum(data: &[u8]) -> u64 {
    fnv1a64(data)
}

/// Verify that a received file matches the sender's checksum.
pub fn verify_transfer(expected: u64, received: &[u8]) -> TransferChecksumResult {
    let actual = fnv1a64_checksum(received);
    TransferChecksumResult {
        matched: expected == actual,
        expected,
        actual,
    }
}

/// Compute a transfer checksum from a set of chunk payloads.
pub fn transfer_checksum(chunks: &[Vec<u8>]) -> u64 {
    let mut stream = Vec::new();
    for c in chunks {
        stream.extend_from_slice(c);
    }
    fnv1a64_checksum(&stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_known_vector() {
        // FNV-1a 64-bit known vector for "123456789".
        let data = b"123456789";
        assert_eq!(fnv1a64_checksum(data), 0x6d5573923c6cdfcu64);
    }

    #[test]
    fn verify_match() {
        let data = b"hello";
        let expected = fnv1a64_checksum(data);
        let r = verify_transfer(expected, data);
        assert!(r.matched);
        assert_eq!(r.actual, expected);
    }

    #[test]
    fn verify_mismatch() {
        let data = b"hello";
        let expected = fnv1a64_checksum(b"world");
        let r = verify_transfer(expected, data);
        assert!(!r.matched);
    }

    #[test]
    fn transfer_checksum_concatenates_chunks() {
        let chunks = vec![vec![1, 2, 3], vec![4, 5, 6]];
        let sum = transfer_checksum(&chunks);
        let flat: Vec<u8> = vec![1, 2, 3, 4, 5, 6];
        assert_eq!(sum, fnv1a64_checksum(&flat));
    }

    #[test]
    fn transfer_checksum_empty() {
        assert_eq!(transfer_checksum(&[]), fnv1a64_checksum(&[]));
    }
}
