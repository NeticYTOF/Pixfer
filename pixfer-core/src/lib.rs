//! pixfer-core — the screen-to-camera light link: multicolor block frames,
//! Reed-Solomon(255,223) FEC, CRC32 integrity, and auto-zip transfer planning.

pub mod adaptive_rate;
pub mod audit;
pub mod battery;
pub mod benchmark;
pub mod calibration;
pub mod camera_config;
pub mod checksum;
pub mod chunk_index;
pub mod color_calibration;
pub mod color_palette;
pub mod compress;
pub mod config;
pub mod congestion;
pub mod dedup;
pub mod diagnostics;
pub mod errors;
pub mod features;
pub mod fingerprint;
pub mod frame_builder_ext;
pub mod frame_pool;
pub mod frame_validator_ext;
pub mod framing;
pub mod gf256;
pub mod hal;
pub mod integrity;
pub mod link_budget;
pub mod link_matrix;
pub mod link_quality;
pub mod link_stats;
pub mod logging;
pub mod metrics;
pub mod modulation;
pub mod observability;
pub mod pipeline;
pub mod policy;
pub mod power;
pub mod progress;
pub mod protocol;
pub mod protocol_codec;
pub mod queue;
pub mod reassembly;
pub mod reassembly_ext;
pub mod reliability;
pub mod resilience;
pub mod retransmit;
pub mod rs;
pub mod scheduler;
pub mod screen_renderer;
pub mod session;
pub mod session_log;
pub mod session_registry;
pub mod state;
pub mod stats;
pub mod sync;
pub mod telemetry;
pub mod thermal;
pub mod throughput_estimator;
pub mod throughput_meter;
pub mod timing;
pub mod transfer;
pub mod transfer_orchestrator;
pub mod transfer_plan;
pub mod ui_state;
pub mod validation;
pub mod worker;

pub use adaptive_rate::{
    pick_rate_config, AdaptiveRateController, RateAdaptationResult, RateDirection,
};
pub use audit::{AuditEntry, AuditLog};
pub use battery::{BatteryGovernor, BatteryModel, BatteryState};
pub use benchmark::{BenchResult, Benchmark};
pub use calibration::{Calibration, CalibrationProfile, CalibrationRegistry};
pub use camera_config::{pick_camera_config, CameraConfig};
pub use checksum::{
    pick_checksum, Adler32Checksum, Checksum, ChecksumRegistry, ChecksumResult, ChecksumSuite,
    Crc32Checksum, Fnv1aChecksum,
};
pub use chunk_index::ChunkIndex;
pub use color_calibration::ColorCalibration;
pub use color_palette::{color_for_value, value_for_color, Color};
pub use compress::{is_zip, unzip_first, zip_bytes, CompressError};
pub use config::{Config, ConfigBuilder, ConfigProfile};
pub use congestion::{
    pick_aimd, AimdController, Allocation, CapacityProbe, CongestionState, Flow, FlowScheduler,
};
pub use dedup::{decide, DedupDecision, DedupSet, DedupStage};
pub use diagnostics::{Diagnostic, Diagnostics, LinkHealthProbe};
pub use errors::{ErrorContext, PixferError, Severity};
pub use features::{standard_features, Feature, FeatureRegistry};
pub use fingerprint::{fnv1a64, FileFingerprint, FingerprintIndex};
pub use frame_builder_ext::FrameBuilder2;
pub use frame_pool::ChunkFramePool;
pub use frame_validator_ext::{FrameValidation, FrameValidatorExt};
pub use framing::{FrameBuilder, FramePool, FrameValidator};
pub use hal::{CameraFrame, CameraHal, DisplayHal};
pub use integrity::{ChecksumReport, IntegrityChecker, IntegrityResult, BLOCK_GRID_LEN};
pub use link_budget::{default_capacity, AllocationSummary, BudgetPlan, FlowRequest, LinkBudget};
pub use link_matrix::{LinkMatrix, MetricCell};
pub use link_quality::{quality_score, LinkQuality, QualityMonitor};
pub use link_stats::LinkStats;
pub use logging::{
    DefaultLog, LogLevel, LogRecord, LogSink, Logger, MemorySink, SharedSink, WriterSink,
};
pub use metrics::{Counter, Gauge, Histogram, Metrics, ThroughputMeter};
pub use modulation::{
    pick_strategy, CskStrategy, Modulation, ModulationStrategy, OokStrategy, VppmStrategy,
};
pub use observability::Observability;
pub use pipeline::{
    ChunkStage, CompressStage, EncodeStage, Pipeline, PipelineContext, PipelineError,
    PipelineStage, ValidateStage,
};
pub use policy::{
    pick_policy, ExponentialBackoffPolicy, FixedDelayPolicy, LinearBackoffPolicy, RetryBudget,
    RetryPolicy, RetryScheduler,
};
pub use power::{pick_power_profile, PowerBudget, PowerGovernor, PowerProfile, PowerReport};
pub use progress::{ProgressSnapshot, ProgressTracker};
pub use protocol::{Frame, FrameKind, Protocol, ProtocolError};
pub use protocol_codec::{DecodeResult, ProtocolCodec};
pub use queue::WorkQueue;
pub use reassembly::Reassembler;
pub use reassembly_ext::ReassemblyExt;
pub use reliability::{ReliabilitySnapshot, ReliabilityTracker};
pub use resilience::{BreakerState, CircuitBreaker, DegradationMode, HealthScore};
pub use retransmit::{RetransmitEntry, RetransmitScheduler};
pub use rs::{RSError, ReedSolomon, RS_K, RS_N, RS_T};
pub use scheduler::{
    FifoPolicy, PriorityPolicy, RoundRobinPolicy, SchedulerPolicy, TransferJob, TransferScheduler,
};
pub use screen_renderer::{ScreenBuffer, ScreenRenderer};
pub use session::{Role, SessionState, TransferSession};
pub use session_log::{SessionEvent, SessionLog};
pub use session_registry::{SessionEntry, SessionRegistry};
pub use state::{FsmEvent, FsmState, TransferFsm};
pub use stats::{SessionStatsMap, TransferStats};
pub use sync::{SyncDetector, SyncPattern};
pub use telemetry::{TelemetryBus, TelemetryEvent, TelemetryRecorder};
pub use thermal::{
    classify, pick_thermal_budget, thermal_throttle, ThermalBudget, ThermalGovernor, ThermalState,
};
pub use throughput_estimator::{pick_estimator, ThroughputEstimator};
pub use throughput_meter::ThroughputMeter2;
pub use timing::{Clock, Pacer, SystemClock};
pub use transfer_orchestrator::{TransferOrchestrator, TransferOutcome};
pub use transfer_plan::{build_plan, TransferPlan};
pub use ui_state::{UiState, UiStateMachine};
pub use validation::{
    ConfigValidator, FrameValidator2, PayloadValidator, Validator, ValidatorRegistry,
};
pub use worker::{Direction, TransferWorker, WorkerEvent};
