//! pixfer-core — the screen-to-camera light link: multicolor block frames,
//! Reed-Solomon(255,223) FEC, CRC32 integrity, and auto-zip transfer planning.

pub mod adaptive_rate;
pub mod audit;
pub mod benchmark;
pub mod calibration;
pub mod compress;
pub mod config;
pub mod congestion;
pub mod diagnostics;
pub mod errors;
pub mod features;
pub mod framing;
pub mod gf256;
pub mod hal;
pub mod integrity;
pub mod link_budget;
pub mod logging;
pub mod metrics;
pub mod modulation;
pub mod pipeline;
pub mod policy;
pub mod protocol;
pub mod queue;
pub mod reassembly;
pub mod resilience;
pub mod rs;
pub mod scheduler;
pub mod session;
pub mod state;
pub mod stats;
pub mod sync;
pub mod telemetry;
pub mod timing;
pub mod transfer;
pub mod validation;
pub mod worker;

pub use adaptive_rate::{
    pick_rate_config, AdaptiveRateController, RateAdaptationResult, RateDirection,
};
pub use audit::{AuditEntry, AuditLog};
pub use benchmark::{BenchResult, Benchmark};
pub use calibration::{Calibration, CalibrationProfile, CalibrationRegistry};
pub use compress::{is_zip, unzip_first, zip_bytes, CompressError};
pub use config::{Config, ConfigBuilder, ConfigProfile};
pub use congestion::{
    pick_aimd, AimdController, Allocation, CapacityProbe, CongestionState, Flow, FlowScheduler,
};
pub use diagnostics::{Diagnostic, Diagnostics, LinkHealthProbe};
pub use errors::{ErrorContext, PixferError, Severity};
pub use features::{standard_features, Feature, FeatureRegistry};
pub use framing::{FrameBuilder, FramePool, FrameValidator};
pub use hal::{CameraFrame, CameraHal, DisplayHal};
pub use integrity::{ChecksumReport, IntegrityChecker, IntegrityResult, BLOCK_GRID_LEN};
pub use link_budget::{default_capacity, AllocationSummary, BudgetPlan, FlowRequest, LinkBudget};
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
pub use policy::{
    pick_policy, ExponentialBackoffPolicy, FixedDelayPolicy, LinearBackoffPolicy, RetryBudget,
    RetryPolicy, RetryScheduler,
};
pub use protocol::{Frame, FrameKind, Protocol, ProtocolError};
pub use queue::WorkQueue;
pub use reassembly::Reassembler;
pub use resilience::{BreakerState, CircuitBreaker, DegradationMode, HealthScore};
pub use rs::{RSError, ReedSolomon, RS_K, RS_N, RS_T};
pub use scheduler::{
    FifoPolicy, PriorityPolicy, RoundRobinPolicy, SchedulerPolicy, TransferJob, TransferScheduler,
};
pub use session::{Role, SessionState, TransferSession};
pub use state::{FsmEvent, FsmState, TransferFsm};
pub use stats::{SessionStatsMap, TransferStats};
pub use sync::{SyncDetector, SyncPattern};
pub use telemetry::{TelemetryBus, TelemetryEvent, TelemetryRecorder};
pub use timing::{Clock, Pacer, SystemClock};
pub use validation::{
    ConfigValidator, FrameValidator2, PayloadValidator, Validator, ValidatorRegistry,
};
pub use worker::{Direction, TransferWorker, WorkerEvent};
