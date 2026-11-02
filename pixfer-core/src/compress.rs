//! Minimal single-entry ZIP container (raw deflate, method 8) so large files
//! can be zipped, transferred over the light link, and auto-unzipped at the
//! other end.

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressError {
    NotZip,
    BadZip,
    InflateFailed,
}

const ZIP_LOCAL_SIG: u32 = 0x04034b50;
const ZIP_CENTRAL_SIG: u32 = 0x02014b50;
const ZIP_EOCD_SIG: u32 = 0x06054b50;

/// True if `data` starts with the ZIP local-file-header magic "PK\x03\x04".
pub fn is_zip(data: &[u8]) -> bool {
    data.len() >= 4 && u32::from_le_bytes(data[0..4].try_into().unwrap()) == ZIP_LOCAL_SIG
}

/// Wrap `data` in a single-entry ZIP (deflate) under `name`.
pub fn zip_bytes(data: &[u8], name: &str) -> Vec<u8> {
    let name_bytes = name.as_bytes();
    let crc = crc32fast::hash(data);

    let mut deflated: Vec<u8> = Vec::new();
    {
        let mut enc = DeflateEncoder::new(&mut deflated, Compression::fast());
        enc.write_all(data).expect("deflate write");
        enc.finish().expect("deflate finish");
    }

    let mut out: Vec<u8> = Vec::new();
    // Local file header (30-byte fixed part)
    out.extend_from_slice(&ZIP_LOCAL_SIG.to_le_bytes());
    out.extend_from_slice(&20u16.to_le_bytes()); // version needed
    out.extend_from_slice(&0u16.to_le_bytes()); // flags
    out.extend_from_slice(&8u16.to_le_bytes()); // method: deflate
    out.extend_from_slice(&0u16.to_le_bytes()); // mod time
    out.extend_from_slice(&0u16.to_le_bytes()); // mod date
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(deflated.len() as u32).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // extra field length
    out.extend_from_slice(name_bytes);
    out.extend_from_slice(&deflated);

    let cd_start = out.len() as u32;

    // Central directory entry (46-byte fixed part)
    out.extend_from_slice(&ZIP_CENTRAL_SIG.to_le_bytes());
    out.extend_from_slice(&20u16.to_le_bytes()); // version made by
    out.extend_from_slice(&20u16.to_le_bytes()); // version needed
    out.extend_from_slice(&0u16.to_le_bytes()); // flags
    out.extend_from_slice(&8u16.to_le_bytes()); // method
    out.extend_from_slice(&0u16.to_le_bytes()); // mod time
    out.extend_from_slice(&0u16.to_le_bytes()); // mod date
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(deflated.len() as u32).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // extra
    out.extend_from_slice(&0u16.to_le_bytes()); // comment
    out.extend_from_slice(&0u16.to_le_bytes()); // disk number
    out.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
    out.extend_from_slice(&0u32.to_le_bytes()); // external attrs
    out.extend_from_slice(&0u32.to_le_bytes()); // local header offset
    out.extend_from_slice(name_bytes);
    let cd_size = (out.len() as u32) - cd_start;

    // End of central directory record
    out.extend_from_slice(&ZIP_EOCD_SIG.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk number
    out.extend_from_slice(&0u16.to_le_bytes()); // disk with central dir
    out.extend_from_slice(&1u16.to_le_bytes()); // entries on this disk
    out.extend_from_slice(&1u16.to_le_bytes()); // total entries
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment length
    out
}

/// Extract the first entry of a single-entry ZIP produced by `zip_bytes`.
pub fn unzip_first(data: &[u8]) -> Result<(String, Vec<u8>), CompressError> {
    // Locate the EOCD (within the last 65557 bytes)
    let scan_start = data.len().saturating_sub(65557);
    let mut eocd: Option<usize> = None;
    for i in ((scan_start)..data.len().saturating_sub(3)).rev() {
        if u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) == ZIP_EOCD_SIG {
            eocd = Some(i);
            break;
        }
    }
    let e = eocd.ok_or(CompressError::NotZip)?;
    if e + 18 > data.len() {
        return Err(CompressError::BadZip);
    }
    let entry_count = u16::from_le_bytes(data[e + 8..e + 10].try_into().unwrap()) as usize;
    let cd_start = u32::from_le_bytes(data[e + 16..e + 20].try_into().unwrap()) as usize;
    if entry_count == 0 || cd_start >= data.len() {
        return Err(CompressError::BadZip);
    }
    let p = cd_start;
    if p + 46 > data.len()
        || u32::from_le_bytes(data[p..p + 4].try_into().unwrap()) != ZIP_CENTRAL_SIG
    {
        return Err(CompressError::BadZip);
    }
    let name_len = u16::from_le_bytes(data[p + 28..p + 30].try_into().unwrap()) as usize;
    let extra_len = u16::from_le_bytes(data[p + 30..p + 32].try_into().unwrap()) as usize;
    let comp_size = u32::from_le_bytes(data[p + 20..p + 24].try_into().unwrap()) as usize;
    let local_off = u32::from_le_bytes(data[p + 42..p + 46].try_into().unwrap()) as usize;

    let name = String::from_utf8_lossy(&data[p + 46..p + 46 + name_len]).into_owned();
    let data_off = local_off + 30 + name_len + extra_len;
    if data_off + comp_size > data.len() {
        return Err(CompressError::BadZip);
    }
    let comp = &data[data_off..data_off + comp_size];
    let mut dec = DeflateDecoder::new(comp);
    let mut out = Vec::new();
    io::copy(&mut dec, &mut out).map_err(|_| CompressError::InflateFailed)?;
    Ok((name, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_roundtrip() {
        let data = b"hello pixfer light link payload";
        let z = zip_bytes(data, "hello.txt");
        assert!(is_zip(&z));
        let (name, out) = unzip_first(&z).expect("unzip");
        assert_eq!(name, "hello.txt");
        assert_eq!(out, data.to_vec());
    }

    #[test]
    fn is_zip_magic() {
        assert!(is_zip(b"PK\x03\x04\x00"));
        assert!(!is_zip(b"PK\x03\x05\x00"));
        assert!(!is_zip(&[0, 1, 2]));
    }

    #[test]
    fn rejects_garbage() {
        let garbage = [0u8; 40];
        assert_eq!(unzip_first(&garbage), Err(CompressError::NotZip));
    }

    #[test]
    fn zip_empty_roundtrip() {
        let z = zip_bytes(b"", "empty.txt");
        let (name, out) = unzip_first(&z).expect("unzip empty");
        assert_eq!(name, "empty.txt");
        assert!(out.is_empty());
    }

    #[test]
    fn zip_large_roundtrip() {
        let data: Vec<u8> = (0..(1024 * 1024)).map(|i| i as u8).collect();
        let z = zip_bytes(&data, "seq.bin");
        let (name, out) = unzip_first(&z).expect("unzip");
        assert_eq!(name, "seq.bin");
        assert_eq!(out, data);
    }
}
