//! pixfer-core — the screen-to-camera light link: multicolor block frames,
//! Reed-Solomon(255,223) FEC, CRC32 integrity, and auto-zip transfer planning.

pub mod adaptive_rate;
pub mod audit;
pub mod bandwidth_allocator;
pub mod battery;
pub mod benchmark;
pub mod calibration;
pub mod camera_config;
pub mod checksum;
pub mod chunk_index;
pub mod chunk_prioritizer;
pub mod chunk_reorder;
pub mod color_adaptive;
pub mod color_calibration;
pub mod color_detector;
pub mod color_drift_monitor;
pub mod color_palette;
pub mod color_quantizer;
pub mod color_temperature;
pub mod compress;
pub mod config;
pub mod congestion;
pub mod dedup;
pub mod diagnostics;
pub mod errors;
pub mod exposure_controller;
pub mod features;
pub mod fingerprint;
pub mod frame_builder_ext;
pub mod frame_codec;
pub mod frame_dedup;
pub mod frame_encoder;
pub mod frame_pacer;
pub mod frame_pool;
pub mod frame_scheduler;
pub mod frame_validator_ext;
pub mod framing;
pub mod gf256;
pub mod hal;
pub mod integrity;
pub mod link_budget;
pub mod link_budget_ext;
pub mod link_calibration_store;
pub mod link_capacity;
pub mod link_fingerprint;
pub mod link_matrix;
pub mod link_monitor;
pub mod link_probe;
pub mod link_quality;
pub mod link_scorer;
pub mod link_stats;
pub mod link_throughput_probe;
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
pub mod rx_camera;
pub mod rx_exposure_lock;
pub mod scheduler;
pub mod screen_renderer;
pub mod session;
pub mod session_log;
pub mod session_registry;
pub mod state;
pub mod stats;
pub mod sync;
pub mod sync_recovery;
pub mod telemetry;
pub mod thermal;
pub mod throughput_estimator;
pub mod throughput_meter;
pub mod timing;
pub mod transfer;
pub mod transfer_checksum;
pub mod transfer_health;
pub mod transfer_heartbeat;
pub mod transfer_orchestrator;
pub mod transfer_plan;
pub mod transfer_resume;
pub mod transfer_retry_policy;
pub mod transfer_rollback;
pub mod transfer_session_ext;
pub mod transfer_slicer;
pub mod transfer_watchdog;
pub mod tx_screen;
pub mod ui_state;
pub mod validation;
pub mod worker;

pub use adaptive_rate::{
    pick_rate_config, AdaptiveRateController, RateAdaptationResult, RateDirection,
};
pub use audit::{AuditEntry, AuditLog};
pub use bandwidth_allocator::{allocator_from_budget, AllocRequest, BandwidthAllocator};
pub use battery::{BatteryGovernor, BatteryModel, BatteryState};
pub use benchmark::{BenchResult, Benchmark};
pub use calibration::{Calibration, CalibrationProfile, CalibrationRegistry};
pub use camera_config::{pick_camera_config, CameraConfig};
pub use checksum::{
    pick_checksum, Adler32Checksum, Checksum, ChecksumRegistry, ChecksumResult, ChecksumSuite,
    Crc32Checksum, Fnv1aChecksum,
};
pub use chunk_index::ChunkIndex;
pub use chunk_prioritizer::{ChunkPrioritizer, PendingChunk};
pub use chunk_reorder::ChunkReorder;
pub use color_adaptive::{recalibrate, AdaptivePalette, MeasuredColor};
pub use color_calibration::ColorCalibration;
pub use color_drift_monitor::{ColorDriftMonitor, DriftSample};
pub use color_palette::{color_for_value, value_for_color, Color};
pub use color_quantizer::{quantize, quantize_with_tolerance};
pub use color_temperature::{TemperatureCorrector, TemperatureReading};
pub use compress::{is_zip, unzip_first, zip_bytes, CompressError};
pub use config::{Config, ConfigBuilder, ConfigProfile};
pub use congestion::{
    pick_aimd, AimdController, Allocation, CapacityProbe, CongestionState, Flow, FlowScheduler,
};
pub use dedup::{decide, DedupDecision, DedupSet, DedupStage};
pub use diagnostics::{Diagnostic, Diagnostics, LinkHealthProbe};
pub use errors::{ErrorContext, PixferError, Severity};
pub use exposure_controller::{
    locked_exposure, ExposureController, ExposureReading, ExposureTarget,
};
pub use features::{standard_features, Feature, FeatureRegistry};
pub use fingerprint::{fnv1a64, FileFingerprint, FingerprintIndex};
pub use frame_builder_ext::FrameBuilder2;
pub use frame_codec::{decode, decode_payload, encode, encode_payload, BLOCKS_PER_FRAME};
pub use frame_dedup::FrameDedup;
pub use frame_encoder::FrameEncoder;
pub use frame_pacer::FramePacer;
pub use frame_pool::ChunkFramePool;
pub use frame_scheduler::FrameScheduler;
pub use frame_validator_ext::{FrameValidation, FrameValidatorExt};
pub use framing::{FrameBuilder, FramePool, FrameValidator};
pub use hal::{CameraFrame, CameraHal, DisplayHal};
pub use integrity::{ChecksumReport, IntegrityChecker, IntegrityResult, BLOCK_GRID_LEN};
pub use link_budget::{default_capacity, AllocationSummary, BudgetPlan, FlowRequest, LinkBudget};
pub use link_budget_ext::{ExtendedSummary, LinkBudgetExt};
pub use link_calibration_store::{CalibrationStore, CalibrationValues};
pub use link_fingerprint::{compute_link_fingerprint, same_link, LinkFingerprint};
pub use link_matrix::{LinkMatrix, MetricCell};
pub use link_monitor::LinkMonitor;
pub use link_probe::{LinkProbe, ProbeResult};
pub use link_quality::{quality_score, LinkQuality, QualityMonitor};
pub use link_scorer::{LinkScorer, ScoreInputs};
pub use link_stats::LinkStats;
pub use link_throughput_probe::{LinkThroughputProbe, ThroughputProbeResult};
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
pub use rx_camera::RxCamera;
pub use rx_exposure_lock::{suggest_exposure, RxCameraExposureLock};
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
pub use sync_recovery::{RecoveryState, SyncRecovery};
pub use telemetry::{TelemetryBus, TelemetryEvent, TelemetryRecorder};
pub use thermal::{
    classify, pick_thermal_budget, thermal_throttle, ThermalBudget, ThermalGovernor, ThermalState,
};
pub use throughput_estimator::{pick_estimator, ThroughputEstimator};
pub use throughput_meter::ThroughputMeter2;
pub use timing::{Clock, Pacer, SystemClock};
pub use transfer_checksum::{
    fnv1a64_checksum, transfer_checksum, verify_transfer, TransferChecksumResult,
};
pub use transfer_health::{HealthSample, HealthTier, TransferHealth};
pub use transfer_heartbeat::{HeartbeatSample, TransferHeartbeat};
pub use transfer_orchestrator::{TransferOrchestrator, TransferOutcome};
pub use transfer_plan::{build_plan, TransferPlan};
pub use transfer_resume::{ResumePoint, TransferResume};
pub use transfer_retry_policy::{RetryPolicy as TransferRetryPolicy, RetryPolicyKind};
pub use transfer_rollback::{ReassemblySnapshot, TransferRollback};
pub use transfer_session_ext::TransferSessionExt;
pub use transfer_slicer::{chunk_count, slice_payload, Chunk};
pub use transfer_watchdog::{TransferWatchdog, WatchdogState};
pub use tx_screen::TxScreen;
pub use ui_state::{UiState, UiStateMachine};
pub use validation::{
    ConfigValidator, FrameValidator2, PayloadValidator, Validator, ValidatorRegistry,
};
pub use worker::{Direction, TransferWorker, WorkerEvent};
