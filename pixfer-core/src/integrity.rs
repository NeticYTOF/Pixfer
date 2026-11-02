//! Stateful CRC32 integrity accounting for byte streams and optical block grids.
//! Hilariously over-engineered because even photons deserve an auditor.

use std::time::SystemTime;

use crate::protocol::{GRID_H, GRID_W};

/// Number of color blocks in one complete Pixfer optical frame.
pub const BLOCK_GRID_LEN: usize = GRID_W * GRID_H;

/// A point-in-time description of an integrity calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChecksumReport {
    /// CRC32 value computed over all bytes accepted by the checker.
    pub crc: u32,
    /// Saturating number of bytes accepted by the checker.
    pub length: u64,
    /// Whether `crc` matched the expected value supplied by the caller.
    pub matches: bool,
    /// Wall-clock instant at which this report was created.
    pub computed_at: SystemTime,
}

impl ChecksumReport {
    /// Construct a report from already-computed values.
    pub fn new(crc: u32, length: u64, matches: bool) -> Self {
        Self {
            crc,
            length,
            matches,
            computed_at: SystemTime::now(),
        }
    }

    /// Return the covered length as `usize`, saturating on narrow platforms.
    pub fn length_usize(&self) -> usize {
        usize::try_from(self.length).unwrap_or(usize::MAX)
    }
}

/// Outcome of checking data or accepting a complete optical block grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityResult {
    /// The operation completed and produced an auditable report.
    Valid(ChecksumReport),
    /// A block update did not contain exactly one 270x480 grid.
    InvalidBlockLength {
        /// Number of blocks required by the wire format.
        expected: usize,
        /// Number of blocks supplied by the caller.
        actual: usize,
    },
}

impl IntegrityResult {
    /// Return the successful report, if the operation accepted its input.
    pub fn report(self) -> Option<ChecksumReport> {
        match self {
            Self::Valid(report) => Some(report),
            Self::InvalidBlockLength { .. } => None,
        }
    }

    /// Return whether the operation accepted its input and matched expectation.
    pub fn matches(self) -> bool {
        self.report().map(|report| report.matches).unwrap_or(false)
    }
}

/// Incremental CRC32 checker with saturating byte accounting.
///
/// The checker uses the same IEEE CRC32 implementation as the wire protocol.
/// Calling [`IntegrityChecker::digest`] never consumes or mutates the running
/// state, so callers may inspect progress as frequently as desired.
#[derive(Clone)]
pub struct IntegrityChecker {
    hasher: crc32fast::Hasher,
    length: u64,
}

impl std::fmt::Debug for IntegrityChecker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IntegrityChecker")
            .field("digest", &format_args!("{:#010x}", self.digest()))
            .field("length", &self.length)
            .finish()
    }
}

impl Default for IntegrityChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegrityChecker {
    /// Create an empty checker whose digest is the standard empty CRC32 value.
    pub fn new() -> Self {
        Self {
            hasher: crc32fast::Hasher::new(),
            length: 0,
        }
    }

    /// Add bytes and return the current CRC32 without consuming the checker.
    pub fn update(&mut self, bytes: &[u8]) -> u32 {
        self.hasher.update(bytes);
        self.length = self.length.saturating_add(bytes.len() as u64);
        self.digest()
    }

    /// Add exactly one 270x480 color-block grid.
    ///
    /// Invalid grid lengths are rejected without changing either the digest or
    /// byte count. Each block is represented by its palette-index byte.
    pub fn update_block(&mut self, block: &[u8]) -> IntegrityResult {
        if block.len() != BLOCK_GRID_LEN {
            return IntegrityResult::InvalidBlockLength {
                expected: BLOCK_GRID_LEN,
                actual: block.len(),
            };
        }
        let crc = self.update(block);
        IntegrityResult::Valid(ChecksumReport::new(crc, self.length, false))
    }

    /// Compute the running CRC32 without mutating the checker.
    ///
    /// `crc32fast::Hasher::finalize` consumes the hasher, so we clone it
    /// to read the current CRC non-destructively.
    pub fn digest(&self) -> u32 {
        self.hasher.clone().finalize()
    }

    /// Accepted byte count (saturating).
    pub fn length(&self) -> u64 {
        self.length
    }

    /// Reset the checker to its empty state.
    pub fn reset(&mut self) {
        self.hasher = crc32fast::Hasher::new();
        self.length = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_crc32_vector() {
        // zlib: crc32("123456789") == 0xCBF43926
        let mut c = IntegrityChecker::new();
        let crc = c.update(b"123456789");
        assert_eq!(crc, 0xCBF43926);
    }

    #[test]
    fn incremental_equals_oneshot() {
        let mut a = IntegrityChecker::new();
        let oneshot = a.update(b"hello world");
        let mut b = IntegrityChecker::new();
        b.update(b"hello ");
        let incr = b.update(b"world");
        assert_eq!(oneshot, incr);
    }

    #[test]
    fn update_block_requires_full_grid() {
        let mut c = IntegrityChecker::new();
        // A block grid is GRID_W * GRID_H = 270 * 480 = 129600 bytes.
        let full = vec![0x05u8; BLOCK_GRID_LEN];
        let r = c.update_block(&full);
        assert!(r.report().is_some());
        let short = vec![0x05u8; 10];
        let r2 = c.update_block(&short);
        match r2 {
            IntegrityResult::InvalidBlockLength { expected, actual } => {
                assert_eq!(expected, BLOCK_GRID_LEN);
                assert_eq!(actual, 10);
            }
            _ => panic!("expected InvalidBlockLength"),
        }
    }

    #[test]
    fn reset_clears_state() {
        let mut c = IntegrityChecker::new();
        c.update(b"123456789");
        c.reset();
        assert_eq!(c.length(), 0);
        assert_eq!(c.digest(), crc32fast::Hasher::new().finalize());
    }

    #[test]
    fn report_shape() {
        let mut c = IntegrityChecker::new();
        let report = c.update(b"abc");
        let r = ChecksumReport::new(report, c.length(), true);
        assert_eq!(r.length_usize(), 3);
    }
}
