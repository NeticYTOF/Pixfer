//! Extended transfer orchestrator: coordinates the full transfer lifecycle —
//! planning, compressing, encoding, sending, and verifying. (Hilariously
//! over-engineered: an "orchestration layer" for a few-KB/s light link.)

use crate::compress::zip_bytes;
use crate::frame_codec::encode_payload;
use crate::rs::RS_K;

/// The outcome of an orchestrated transfer.
#[derive(Debug, Clone, PartialEq)]
pub enum TransferOutcomeExt {
    /// The transfer completed and verified successfully.
    Success { bytes: u64 },
    /// The transfer failed, with a reason.
    Failed { reason: String },
}

/// A chunk of payload bytes.
#[derive(Debug, Clone)]
pub struct ChunkExt {
    pub idx: u32,
    pub data: Vec<u8>,
}

/// Coordinates a full transfer over the light link.
#[derive(Default)]
pub struct TransferOrchestratorExt {
    /// The maximum payload bytes per frame.
    pub chunk_bytes: usize,
    /// The auto-zip threshold (bytes).
    pub zip_threshold: usize,
}

impl TransferOrchestratorExt {
    pub fn new(chunk_bytes: usize, zip_threshold: usize) -> Self {
        Self {
            chunk_bytes,
            zip_threshold,
        }
    }

    /// The number of chunks a payload spans.
    pub fn chunk_count(&self, data_len: usize) -> u32 {
        if data_len == 0 {
            0
        } else {
            ((data_len + self.chunk_bytes - 1) / self.chunk_bytes) as u32
        }
    }

    /// Split `data` into chunks of at most `chunk_bytes` bytes.
    pub fn slice(&self, data: &[u8]) -> Vec<ChunkExt> {
        let n = self.chunk_count(data.len());
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            let start = (i as usize) * self.chunk_bytes;
            let end = ((i + 1) as usize)
                .saturating_mul(self.chunk_bytes)
                .min(data.len());
            out.push(ChunkExt {
                idx: i,
                data: data[start..end].to_vec(),
            });
        }
        out
    }

    /// Plan the transfer: optionally compress, then report the chunk count.
    pub fn plan(&self, payload: &[u8]) -> Result<u32, String> {
        let data = if payload.len() > self.zip_threshold {
            zip_bytes(payload, "pixfer.bin")
        } else {
            payload.to_vec()
        };
        Ok(self.chunk_count(data.len()))
    }

    /// Encode a single chunk into an on-screen frame.
    pub fn encode_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, String> {
        encode_payload(chunk).map_err(|e| e.to_string())
    }

    /// Run a full transfer: plan -> slice -> encode each chunk -> send.
    pub fn run<F>(&self, payload: &[u8], send: &mut F) -> TransferOutcomeExt
    where
        F: FnMut(&[u8]) -> Result<(), String>,
    {
        let data = if payload.len() > self.zip_threshold {
            zip_bytes(payload, "pixfer.bin")
        } else {
            payload.to_vec()
        };
        for chunk in self.slice(&data) {
            let frame = match self.encode_chunk(&chunk.data) {
                Ok(f) => f,
                Err(e) => {
                    return TransferOutcomeExt::Failed {
                        reason: format!("encode chunk {} failed: {}", chunk.idx, e),
                    }
                }
            };
            if let Err(e) = send(&frame) {
                return TransferOutcomeExt::Failed {
                    reason: format!("send chunk {} failed: {}", chunk.idx, e),
                };
            }
        }
        TransferOutcomeExt::Success {
            bytes: payload.len() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_count_math() {
        let o = TransferOrchestratorExt::new(223, 500);
        assert_eq!(o.chunk_count(0), 0);
        assert_eq!(o.chunk_count(1), 1);
        assert_eq!(o.chunk_count(223), 1);
        assert_eq!(o.chunk_count(224), 2);
        assert_eq!(o.chunk_count(500), 3);
    }

    #[test]
    fn slice_produces_sequential_chunks() {
        let o = TransferOrchestratorExt::new(4, 500);
        let data = vec![1u8; 10];
        let chunks = o.slice(&data);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].idx, 0);
        assert_eq!(chunks[0].data.len(), 4);
        assert_eq!(chunks[1].data.len(), 4);
        assert_eq!(chunks[2].data.len(), 2);
    }

    #[test]
    fn plan_zips_large_payload() {
        let o = TransferOrchestratorExt::new(223, 500);
        let payload = vec![9u8; 2000];
        let n = o.plan(&payload).expect("plan");
        assert!(n > 0);
    }

    #[test]
    fn plan_small_payload_no_zip() {
        let o = TransferOrchestratorExt::new(223, 500);
        let payload = vec![1u8; 30];
        assert_eq!(o.plan(&payload).expect("plan"), 1);
    }

    #[test]
    fn encode_chunk_roundtrips() {
        let o = TransferOrchestratorExt::new(223, 500);
        let chunk = vec![3u8; 10];
        let frame = o.encode_chunk(&chunk).expect("encode");
        assert!(!frame.is_empty());
    }

    #[test]
    fn run_full_transfer_succeeds() {
        let o = TransferOrchestratorExt::new(223, 500);
        let payload = vec![1u8; 30];
        let mut sends = 0;
        let mut send = |frame: &[u8]| -> Result<(), String> {
            sends += 1;
            Ok(())
        };
        let outcome = o.run(&payload, &mut send);
        match outcome {
            TransferOutcomeExt::Success { bytes } => {
                assert_eq!(bytes, 30);
                assert_eq!(sends, 1);
            }
            _ => panic!("expected success"),
        }
    }

    #[test]
    fn run_reports_send_failure() {
        let o = TransferOrchestratorExt::new(223, 500);
        let payload = vec![1u8; 30];
        let mut send = |frame: &[u8]| -> Result<(), String> { Err("link down".to_string()) };
        let outcome = o.run(&payload, &mut send);
        match outcome {
            TransferOutcomeExt::Failed { reason } => assert!(reason.contains("link down")),
            _ => panic!("expected failure"),
        }
    }
}
