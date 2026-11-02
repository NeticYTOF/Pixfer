//! A deliberately over-engineered error system: a single `PixferError`
//! enum with rich variants, structured `ErrorContext`, Display/Debug,
//! From-conversions, and a small assertion helper. This is the kind of
//! "hilarious" over-engineering the project is scaling toward.

use std::fmt;

/// Severity levels used by logging/metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Trace => "trace",
            Severity::Debug => "debug",
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Error => "error",
            Severity::Fatal => "fatal",
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            Severity::Trace => 0,
            Severity::Debug => 1,
            Severity::Info => 2,
            Severity::Warn => 3,
            Severity::Error => 4,
            Severity::Fatal => 5,
        }
    }
}

/// The canonical pixfer error. Every failure in the system funnels through
/// this type so callers can pattern-match on the exact cause.
#[derive(Debug, Clone, PartialEq)]
pub enum PixferError {
    /// The camera HAL could not be opened (permissions, no device, secure-context failure).
    CameraUnavailable(String),
    /// The display HAL could not be driven.
    DisplayUnavailable(String),
    /// The captured block grid could not be decoded (bad magic / kind).
    DecodeFailed(String),
    /// CRC32 mismatch on a chunk (header or payload).
    CrcMismatch { stored: u32, computed: u32 },
    /// RS decoder found more symbol errors than t=16 can correct.
    TooManyErrors { found: usize, correctable: usize },
    /// An ARQ round-trip timed out with no ACK/NAK.
    Timeout {
        timeout_ms: u64,
        chunk_idx: u32,
        attempts: u32,
    },
    /// A frame referenced a session id that does not match the active session.
    SessionMismatch { expected: u16, received: u16 },
    /// Calibrated block geometry drifted (the peer screen moved / zoomed).
    CalibrationDrift { drift_pct: f64 },
    /// Measured throughput fell below the configured floor.
    ThroughputLow { measured_mbps: f64, floor_mbps: f64 },
    /// The auto-zip step failed to produce a smaller archive.
    ZipFailed(String),
    /// The payload exceeded the 223-byte per-frame budget.
    PayloadTooLarge { requested: usize, max: usize },
    /// An unknown / unhandled condition.
    Internal(String),
}

/// Structured context attached to an error so log lines / telemetry carry
/// the "who / where / when" without forcing every call site to format it.
#[derive(Debug, Clone, Default)]
pub struct ErrorContext {
    pub source: String,
    pub component: String,
    pub field: String,
    pub ts_ms: u64,
}

impl PixferError {
    /// Build an error with a context attached (the over-engineered "rich error").
    pub fn with_context(kind: &PixferError, ctx: &ErrorContext) -> (PixferError, ErrorContext) {
        (kind.clone(), ctx.clone())
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            PixferError::Timeout { .. }
                | PixferError::ThroughputLow { .. }
                | PixferError::CalibrationDrift { .. }
        )
    }

    pub fn code(&self) -> &'static str {
        match self {
            PixferError::CameraUnavailable(_) => "CAMERA_UNAVAILABLE",
            PixferError::DisplayUnavailable(_) => "DISPLAY_UNAVAILABLE",
            PixferError::DecodeFailed(_) => "DECODE_FAILED",
            PixferError::CrcMismatch { .. } => "CRC_MISMATCH",
            PixferError::TooManyErrors { .. } => "TOO_MANY_ERRORS",
            PixferError::Timeout { .. } => "TIMEOUT",
            PixferError::SessionMismatch { .. } => "SESSION_MISMATCH",
            PixferError::CalibrationDrift { .. } => "CALIBRATION_DRIFT",
            PixferError::ThroughputLow { .. } => "THROUGHPUT_LOW",
            PixferError::ZipFailed(_) => "ZIP_FAILED",
            PixferError::PayloadTooLarge { .. } => "PAYLOAD_TOO_LARGE",
            PixferError::Internal(_) => "INTERNAL",
        }
    }
}

impl fmt::Display for PixferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PixferError::CameraUnavailable(m) => write!(f, "camera unavailable: {m}"),
            PixferError::DisplayUnavailable(m) => write!(f, "display unavailable: {m}"),
            PixferError::DecodeFailed(m) => write!(f, "decode failed: {m}"),
            PixferError::CrcMismatch { stored, computed } => {
                write!(
                    f,
                    "crc mismatch: stored=0x{stored:08x} computed=0x{computed:08x}"
                )
            }
            PixferError::TooManyErrors { found, correctable } => {
                write!(
                    f,
                    "too many symbol errors: found {found}, can correct {correctable}"
                )
            }
            PixferError::Timeout {
                timeout_ms,
                chunk_idx,
                attempts,
            } => {
                write!(
                    f,
                    "arq timeout after {attempts} attempts (limit {timeout_ms} ms) on chunk {chunk_idx}"
                )
            }
            PixferError::SessionMismatch { expected, received } => {
                write!(f, "session mismatch: expected {expected}, got {received}")
            }
            PixferError::CalibrationDrift { drift_pct } => {
                write!(f, "calibration drift {drift_pct:.1}%")
            }
            PixferError::ThroughputLow {
                measured_mbps,
                floor_mbps,
            } => {
                write!(
                    f,
                    "throughput {measured_mbps:.2} MB/s below floor {floor_mbps:.2} MB/s"
                )
            }
            PixferError::ZipFailed(m) => write!(f, "auto-zip failed: {m}"),
            PixferError::PayloadTooLarge { requested, max } => {
                write!(f, "payload {requested}B exceeds {max}B budget")
            }
            PixferError::Internal(m) => write!(f, "internal: {m}"),
        }
    }
}

impl std::error::Error for PixferError {}

// --- From-conversions so the new error type composes with existing ones.
impl From<crate::protocol::ProtocolError> for PixferError {
    fn from(e: crate::protocol::ProtocolError) -> Self {
        match e {
            crate::protocol::ProtocolError::BadMagic => {
                PixferError::DecodeFailed("bad magic".to_string())
            }
            crate::protocol::ProtocolError::BadCrc => PixferError::CrcMismatch {
                stored: 0,
                computed: 0,
            },
            crate::protocol::ProtocolError::BadLength => {
                PixferError::DecodeFailed("bad grid length".to_string())
            }
            crate::protocol::ProtocolError::DecodeFailed => {
                PixferError::DecodeFailed("unknown frame kind".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ranking() {
        assert!(Severity::Trace.rank() < Severity::Fatal.rank());
        assert_eq!(Severity::Warn.as_str(), "warn");
    }

    #[test]
    fn error_codes_are_stable() {
        let e = PixferError::CrcMismatch {
            stored: 0x11223344,
            computed: 0x55667788,
        };
        assert_eq!(e.code(), "CRC_MISMATCH");
        assert!(!e.is_retryable());

        let t = PixferError::Timeout {
            timeout_ms: 2500,
            chunk_idx: 3,
            attempts: 3,
        };
        assert_eq!(t.code(), "TIMEOUT");
        assert!(t.is_retryable());
    }

    #[test]
    fn display_is_human_readable() {
        let e = PixferError::TooManyErrors {
            found: 17,
            correctable: 16,
        };
        let s = e.to_string();
        assert!(s.contains("found 17"));
        assert!(s.contains("can correct 16"));
    }

    #[test]
    fn context_round_trip() {
        let ctx = ErrorContext {
            source: "rx".into(),
            component: "camera".into(),
            field: "exposure".into(),
            ts_ms: 1234,
        };
        let (kind, back) =
            PixferError::with_context(&PixferError::CameraUnavailable("denied".into()), &ctx);
        assert_eq!(back.ts_ms, ctx.ts_ms);
        assert_eq!(kind, PixferError::CameraUnavailable("denied".into()));
    }
}
