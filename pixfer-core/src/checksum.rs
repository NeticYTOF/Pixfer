//! Pluggable checksum framework: a `Checksum` trait, three concrete
//! implementations (CRC32, Adler-32, FNV-1a 64), a registry, and a suite
//! that runs several checksums over one input. (Hilariously over-engineered:
//! a checksum "framework" for a link that only needs one CRC.)

use std::collections::HashMap;

use crate::fingerprint::fnv1a64;

/// A pluggable checksum strategy.
pub trait Checksum: Send {
    /// Feed bytes into the running checksum.
    fn update(&mut self, bytes: &[u8]);
    /// Finalize and return the digest.
    fn digest(&self) -> u64;
    fn name(&self) -> &'static str;
}

/// Wraps `crc32fast`'s CRC32 as a 64-bit digest.
#[derive(Default)]
pub struct Crc32Checksum {
    buf: Vec<u8>,
}

impl Crc32Checksum {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }
}

impl Checksum for Crc32Checksum {
    fn update(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }
    fn digest(&self) -> u64 {
        crc32fast::hash(&self.buf) as u64
    }
    fn name(&self) -> &'static str {
        "crc32"
    }
}

/// A minimal running Adler-32 implementation.
#[derive(Default)]
pub struct Adler32Checksum {
    a: u32,
    b: u32,
}

impl Adler32Checksum {
    pub fn new() -> Self {
        Self { a: 1, b: 0 }
    }
}

impl Checksum for Adler32Checksum {
    fn update(&mut self, bytes: &[u8]) {
        const MOD: u32 = 65535;
        for &byte in bytes {
            self.a = (self.a + byte as u32).wrapping_rem(MOD + 1);
            self.b = (self.b + self.a).wrapping_rem(MOD + 1);
        }
    }
    fn digest(&self) -> u64 {
        (self.b as u64) << 16 | self.a as u64
    }
    fn name(&self) -> &'static str {
        "adler32"
    }
}

/// A 64-bit FNV-1a checksum over the full buffer.
pub struct Fnv1aChecksum {
    buf: Vec<u8>,
}

impl Fnv1aChecksum {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }
}

impl Default for Fnv1aChecksum {
    fn default() -> Self {
        Self::new()
    }
}

impl Checksum for Fnv1aChecksum {
    fn update(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }
    fn digest(&self) -> u64 {
        fnv1a64(&self.buf)
    }
    fn name(&self) -> &'static str {
        "fnv1a64"
    }
}

/// Pick a checksum implementation by name (the "it depends on your profile"
/// flavor of over-engineering).
pub fn pick_checksum(name: &str) -> Box<dyn Checksum> {
    match name {
        "crc32" => Box::new(Crc32Checksum::new()),
        "adler32" => Box::new(Adler32Checksum::new()),
        "fnv1a64" => Box::new(Fnv1aChecksum::new()),
        _ => Box::new(Crc32Checksum::new()),
    }
}

/// A registry of named checksum strategies.
#[derive(Default)]
pub struct ChecksumRegistry {
    checksums: HashMap<&'static str, Box<dyn Checksum>>,
}

impl ChecksumRegistry {
    pub fn new() -> Self {
        let mut r = Self {
            checksums: HashMap::new(),
        };
        r.register(Box::new(Crc32Checksum::new()));
        r.register(Box::new(Adler32Checksum::new()));
        r.register(Box::new(Fnv1aChecksum::new()));
        r
    }

    pub fn register(&mut self, c: Box<dyn Checksum>) {
        self.checksums.insert(c.name(), c);
    }

    pub fn get(&mut self, name: &str) -> Option<&mut Box<dyn Checksum>> {
        self.checksums.get_mut(name)
    }

    pub fn names(&self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = self.checksums.keys().copied().collect();
        v.sort();
        v
    }
}

/// The result of running a checksum over a buffer.
#[derive(Debug, Clone)]
pub struct ChecksumResult {
    pub name: &'static str,
    pub digest: u64,
}

/// Runs every registered checksum over the same input and reports them.
pub struct ChecksumSuite {
    registry: ChecksumRegistry,
}

impl ChecksumSuite {
    pub fn new() -> Self {
        Self {
            registry: ChecksumRegistry::new(),
        }
    }

    /// Compute all checksums over `data` in one pass.
    pub fn run(&mut self, data: &[u8]) -> Vec<ChecksumResult> {
        let mut out = Vec::with_capacity(self.registry.names().len());
        for name in self.registry.names() {
            if let Some(c) = self.registry.get(name) {
                c.update(data);
                out.push(ChecksumResult {
                    name,
                    digest: c.digest(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adler32_known_vector() {
        let mut c = Adler32Checksum::new();
        c.update(b"Wikipedia");
        // Adler-32 of "Wikipedia" = 0x11e60398
        assert_eq!(c.digest(), 0x11e60398);
    }

    #[test]
    fn crc32_known_vector() {
        let mut c = Crc32Checksum::new();
        c.update(b"123456789");
        assert_eq!(c.digest(), 0xcbf43926);
    }

    #[test]
    fn fnv1a64_matches_fingerprint() {
        let mut c = Fnv1aChecksum::new();
        c.update(b"123456789");
        assert_eq!(c.digest(), fnv1a64(b"123456789"));
    }

    #[test]
    fn registry_has_all_three() {
        let r = ChecksumRegistry::new();
        let names = r.names();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"crc32"));
        assert!(names.contains(&"adler32"));
        assert!(names.contains(&"fnv1a64"));
    }

    #[test]
    fn suite_runs_all() {
        let mut s = ChecksumSuite::new();
        let reports = s.run(b"123456789");
        assert_eq!(reports.len(), 3);
        // The crc32 report must match the known vector.
        let crc = reports.iter().find(|r| r.name == "crc32").unwrap();
        assert_eq!(crc.digest, 0xcbf43926);
    }

    #[test]
    fn registry_get_returns_mut() {
        let mut r = ChecksumRegistry::new();
        let c = r.get("adler32").unwrap();
        c.update(b"abc");
        let d = c.digest();
        assert_eq!(d, {
            let mut a = 1u32;
            let mut b = 0u32;
            for &byte in b"abc" {
                a = (a + byte as u32) % 65536;
                b = (b + a) % 65536;
            }
            (b as u64) << 16 | a as u64
        });
    }
}
