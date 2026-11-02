//! Extended Reed-Solomon: a higher-level RS wrapper around the core coder
//! with batch encode/verify helpers. (Hilariously over-engineered: an "RS
//! toolkit" for a link whose codewords are 255 bytes.)

use crate::rs::{RSError, ReedSolomon, RS_K, RS_N};

/// A batch RS operation result.
#[derive(Debug, Clone)]
pub struct RsBatchResult {
    pub ok: bool,
    pub errors: Vec<String>,
}

/// A higher-level RS helper.
pub struct RSExt {
    coder: ReedSolomon,
}

impl RSExt {
    pub fn new() -> Self {
        Self {
            coder: ReedSolomon::new(),
        }
    }

    /// The coder's generator polynomial (debug aid).
    pub fn generator(&self) -> &[u8] {
        self.coder.generator()
    }

    /// Encode a message into a full RS(255,223) codeword.
    pub fn encode(&self, msg: &[u8]) -> Result<Vec<u8>, RSError> {
        if msg.len() != RS_K {
            return Err(RSError::BadLength);
        }
        Ok(self.coder.encode(msg))
    }

    /// Verify a codeword: re-encode the data portion and compare parity.
    pub fn verify(&self, codeword: &[u8]) -> Result<(), RSError> {
        if codeword.len() != RS_N {
            return Err(RSError::BadLength);
        }
        let parity = &codeword[..RS_N - RS_K];
        let data = &codeword[RS_N - RS_K..];
        let expected = self.coder.encode(data);
        let expected_parity = &expected[..RS_N - RS_K];
        if parity != expected_parity {
            return Err(RSError::CheckFailed);
        }
        Ok(())
    }

    /// Batch-encode a set of messages; returns a result with any errors.
    pub fn encode_batch(&self, msgs: &[Vec<u8>]) -> (Vec<Vec<u8>>, RsBatchResult) {
        let mut out: Vec<Vec<u8>> = Vec::with_capacity(msgs.len());
        let mut errors: Vec<String> = Vec::new();
        for (i, m) in msgs.iter().enumerate() {
            match self.encode(m) {
                Ok(cw) => out.push(cw),
                Err(e) => errors.push(format!("msg {}: {}", i, format_err(&e))),
            }
        }
        (
            out,
            RsBatchResult {
                ok: errors.is_empty(),
                errors,
            },
        )
    }

    /// The number of RS parity bytes.
    pub fn parity_bytes(&self) -> usize {
        RS_N - RS_K
    }
}

fn format_err(e: &RSError) -> &'static str {
    match e {
        RSError::BadLength => "bad length",
        RSError::TooManyErrors => "too many errors",
        RSError::CheckFailed => "check failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg() -> Vec<u8> {
        vec![7u8; RS_K]
    }

    #[test]
    fn encode_produces_full_codeword() {
        let r = RSExt::new();
        let cw = r.encode(&msg()).expect("encode");
        assert_eq!(cw.len(), RS_N);
        assert_eq!(r.parity_bytes(), RS_N - RS_K);
    }

    #[test]
    fn encode_rejects_bad_length() {
        let r = RSExt::new();
        assert_eq!(r.encode(&[1, 2, 3]), Err(RSError::BadLength));
    }

    #[test]
    fn verify_accepts_valid_codeword() {
        let r = RSExt::new();
        let cw = r.encode(&msg()).expect("encode");
        r.verify(&cw).expect("verify");
    }

    #[test]
    fn verify_rejects_corrupted_parity() {
        let r = RSExt::new();
        let mut cw = r.encode(&msg()).expect("encode");
        cw[0] ^= 0xFF; // corrupt first parity byte
        assert_eq!(r.verify(&cw), Err(RSError::CheckFailed));
    }

    #[test]
    fn verify_rejects_bad_length() {
        let r = RSExt::new();
        assert_eq!(r.verify(&[1, 2, 3]), Err(RSError::BadLength));
    }

    #[test]
    fn batch_encode_mixed() {
        let r = RSExt::new();
        let good = msg();
        let bad = vec![1u8; 5];
        let (out, res) = r.encode_batch(&[good.clone(), bad, good.clone()]);
        assert_eq!(out.len(), 2); // only the two valid ones
        assert!(!res.ok);
        assert_eq!(res.errors.len(), 1);
        assert!(res.errors[0].contains("bad length"));
    }

    #[test]
    fn batch_all_good() {
        let r = RSExt::new();
        let (out, res) = r.encode_batch(&[msg(), msg()]);
        assert!(res.ok);
        assert_eq!(out.len(), 2);
    }
}
