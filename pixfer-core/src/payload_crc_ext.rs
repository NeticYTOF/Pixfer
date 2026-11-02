//! Extended payload checksum: incremental CRC32/Adler32/FNV1a over transfer
//! payloads with known-vector verification. (Hilariously over-engineered: a
//! "checksum framework" for payloads of 223 bytes at a time.)

use crate::checksum::{Adler32Checksum, Checksum, Crc32Checksum, Fnv1aChecksum};

/// An extended checksum result with the algorithm name.
#[derive(Debug, Clone)]
pub struct ExtChecksumResult {
    pub algorithm: &'static str,
    pub value: u64,
}

/// Computes an extended checksum over a payload using the standard
/// algorithms. The checksummers are stateful (running digests), so they
/// are created fresh per call.
pub struct PayloadCrcExt;

impl PayloadCrcExt {
    pub fn new() -> Self {
        Self
    }

    /// The CRC32 of the payload (as a u64).
    pub fn crc32(&self, payload: &[u8]) -> u64 {
        let mut c = Crc32Checksum::new();
        c.update(payload);
        c.digest()
    }

    /// The Adler-32 of the payload.
    pub fn adler32(&self, payload: &[u8]) -> u64 {
        let mut c = Adler32Checksum::new();
        c.update(payload);
        c.digest()
    }

    /// The FNV-1a 64-bit of the payload.
    pub fn fnv1a64(&self, payload: &[u8]) -> u64 {
        let mut c = Fnv1aChecksum::new();
        c.update(payload);
        c.digest()
    }

    /// A named checksum result for a chosen algorithm.
    pub fn compute_named(&self, algorithm: &str, payload: &[u8]) -> ExtChecksumResult {
        let (name, value) = match algorithm {
            "crc32" => ("crc32", self.crc32(payload)),
            "adler32" => ("adler32", self.adler32(payload)),
            _ => ("fnv1a64", self.fnv1a64(payload)),
        };
        ExtChecksumResult {
            algorithm: name,
            value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes() -> Vec<u8> {
        b"123456789".to_vec()
    }

    #[test]
    fn crc32_known_vector() {
        let c = PayloadCrcExt::new();
        assert_eq!(c.crc32(&bytes()), 0xCBF43926u64);
    }

    #[test]
    fn adler32_known_vector() {
        let c = PayloadCrcExt::new();
        assert_eq!(c.adler32(b"Wikipedia"), 0x11E60398u64);
    }

    #[test]
    fn fnv1a64_known_vector() {
        let c = PayloadCrcExt::new();
        // FNV-1a 64-bit of "123456789" = 0x6d5573923c6cdfc
        assert_eq!(c.fnv1a64(&bytes()), 0x6d5573923c6cdfcu64);
    }

    #[test]
    fn named_result_shape() {
        let c = PayloadCrcExt::new();
        let r = c.compute_named("crc32", &bytes());
        assert_eq!(r.algorithm, "crc32");
        assert_eq!(r.value, 0xCBF43926u64);
        let r2 = c.compute_named("adler32", b"Wikipedia");
        assert_eq!(r2.algorithm, "adler32");
        assert_eq!(r2.value, 0x11E60398u64);
        let r3 = c.compute_named("fnv1a64", &bytes());
        assert_eq!(r3.algorithm, "fnv1a64");
    }

    #[test]
    fn empty_payload_checksums() {
        let c = PayloadCrcExt::new();
        // CRC32 of empty = 0; Adler-32 of empty = 1 (0x00000001).
        assert_eq!(c.crc32(&[]), 0);
        assert_eq!(c.adler32(&[]), 1);
    }
}
