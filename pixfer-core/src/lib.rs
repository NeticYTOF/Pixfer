//! pixfer-core — the screen-to-camera light link: multicolor block frames,
//! Reed-Solomon(255,223) FEC, CRC32 integrity, and auto-zip transfer planning.

pub mod compress;
pub mod config;
pub mod errors;
pub mod features;
pub mod gf256;
pub mod hal;
pub mod logging;
pub mod metrics;
pub mod modulation;
pub mod pipeline;
pub mod protocol;
pub mod rs;
pub mod session;
pub mod stats;
pub mod telemetry;
pub mod timing;
pub mod transfer;
pub mod worker;

pub use compress::{is_zip, unzip_first, zip_bytes, CompressError};
pub use config::{Config, ConfigBuilder, ConfigProfile};
pub use errors::{ErrorContext, PixferError, Severity};
pub use features::{standard_features, Feature, FeatureRegistry};
pub use hal::{CameraFrame, CameraHal, DisplayHal};
pub use logging::{
    DefaultLog, LogLevel, LogRecord, LogSink, Logger, MemorySink, SharedSink, WriterSink,
};
pub use metrics::{Counter, Gauge, Histogram, Metrics, ThroughputMeter};
pub use modulation::{
    pick_strategy, CskStrategy, Modulation, ModulationStrategy, OokStrategy, VppmStrategy,
};
pub use pipeline::{
    ChunkStage, CompressStage, EncodeStage, Pipeline, PipelineContext, PipelineError,
    PipelineStage, ValidateStage,
};
pub use protocol::{Frame, FrameKind, Protocol, ProtocolError};
pub use rs::{RSError, ReedSolomon, RS_K, RS_N, RS_T};
pub use session::{Role, SessionState, TransferSession};
pub use stats::{SessionStatsMap, TransferStats};
pub use telemetry::{TelemetryBus, TelemetryEvent, TelemetryRecorder};
pub use timing::{Clock, Pacer, SystemClock};
pub use worker::{Direction, TransferWorker, WorkerEvent};
