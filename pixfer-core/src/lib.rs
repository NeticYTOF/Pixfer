//! pixfer-core — the screen-to-camera light link: multicolor block frames,
//! Reed-Solomon(255,223) FEC, CRC32 integrity, and auto-zip transfer planning.

pub mod compress;
pub mod gf256;
pub mod protocol;
pub mod rs;
pub mod session;
pub mod transfer;

pub use compress::{is_zip, unzip_first, zip_bytes, CompressError};
pub use protocol::{Frame, FrameKind, Protocol, ProtocolError};
pub use rs::{ReedSolomon, RS_K, RS_N, RS_T, RSError};
pub use session::{Role, SessionState, TransferSession};
