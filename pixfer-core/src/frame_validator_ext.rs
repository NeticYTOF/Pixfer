//! Extended frame validator: validates every field of a frame against the
//! wire-format invariants. (Hilariously over-engineered: a "validator" that
//! re-checks things the protocol already checks.)

use crate::protocol::{Frame, FrameKind};
use crate::rs::RS_K;

/// The result of validating a frame.
#[derive(Debug, Clone)]
pub struct FrameValidation {
    pub ok: bool,
    pub reason: Option<&'static str>,
}

/// Validate a frame's fields.
pub struct FrameValidatorExt;

impl FrameValidatorExt {
    /// Validate the frame kind is a known variant.
    pub fn validate(&self, frame: &Frame) -> FrameValidation {
        if FrameKind::from_u8(frame.kind as u8).is_none() {
            return FrameValidation {
                ok: false,
                reason: Some("unknown_kind"),
            };
        }
        if frame.kind == FrameKind::Chunk {
            if frame.total_chunks == 0 {
                return FrameValidation {
                    ok: false,
                    reason: Some("zero_total_chunks"),
                };
            }
            if (frame.payload.len() as u64) > (RS_K as u64) {
                return FrameValidation {
                    ok: false,
                    reason: Some("payload_too_large"),
                };
            }
            if frame.chunk_idx >= frame.total_chunks {
                return FrameValidation {
                    ok: false,
                    reason: Some("chunk_idx_out_of_range"),
                };
            }
        }
        FrameValidation {
            ok: true,
            reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_frame(chunk_idx: u32, total: u32, payload_len: usize) -> Frame {
        Frame::new(
            FrameKind::Chunk,
            1,
            chunk_idx,
            total,
            vec![0u8; payload_len],
        )
    }

    #[test]
    fn valid_chunk_passes() {
        let f = chunk_frame(0, 4, RS_K);
        let v = FrameValidatorExt.validate(&f);
        assert!(v.ok);
    }

    #[test]
    fn zero_total_rejected() {
        let f = chunk_frame(0, 0, RS_K);
        let v = FrameValidatorExt.validate(&f);
        assert!(!v.ok);
        assert_eq!(v.reason, Some("zero_total_chunks"));
    }

    #[test]
    fn chunk_idx_out_of_range_rejected() {
        let f = chunk_frame(4, 4, RS_K); // idx == total -> out of range
        let v = FrameValidatorExt.validate(&f);
        assert!(!v.ok);
        assert_eq!(v.reason, Some("chunk_idx_out_of_range"));
    }

    #[test]
    fn oversized_payload_rejected() {
        let f = chunk_frame(0, 4, RS_K + 1);
        let v = FrameValidatorExt.validate(&f);
        assert!(!v.ok);
        assert_eq!(v.reason, Some("payload_too_large"));
    }

    #[test]
    fn control_frames_pass() {
        let f = Frame::new(FrameKind::Ping, 1, 0, 0, Vec::new());
        let v = FrameValidatorExt.validate(&f);
        assert!(v.ok);
    }
}
