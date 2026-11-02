//! Congestion control for the light link: an AIMD (additive-increase /
//! multiplicative-decrease) window controller, a capacity probe with an EMA
//! goodput estimate, and a flow-priority scheduler. (Hilariously over-
//! engineered: a full congestion framework for a screen-to-camera link
//! that carries no TCP headers and no IP packets.)

use crate::config::{Config, ConfigProfile};

/// A single active flow competing for the shared symbol rate.
#[derive(Debug, Clone)]
pub struct Flow {
    pub id: u32,
    pub weight: u32,
}

/// The congestion-control state: the in-flight window and observed loss.
#[derive(Debug, Clone)]
pub struct CongestionState {
    /// Current transmission window, in chunks (frames) in flight.
    window_chunks: u32,
    in_flight: u32,
    round_trips: u64,
    loss_events: u64,
    last_loss_at_rt: u64,
}

impl CongestionState {
    pub fn new(initial_window: u32) -> Self {
        Self {
            window_chunks: initial_window,
            in_flight: 0,
            round_trips: 0,
            loss_events: 0,
            last_loss_at_rt: 0,
        }
    }

    pub fn window(&self) -> u32 {
        self.window_chunks
    }

    pub fn in_flight(&self) -> u32 {
        self.in_flight
    }

    pub fn round_trips(&self) -> u64 {
        self.round_trips
    }

    pub fn loss_events(&self) -> u64 {
        self.loss_events
    }
}

/// AIMD congestion controller: grow the window by a constant on each ACK,
/// shrink it by a factor on each loss. Mirrors TCP's NewReno behavior.
#[derive(Debug, Clone)]
pub struct AimdController {
    state: CongestionState,
    /// Additive increase step (chunks per round trip).
    inc: u32,
    /// Multiplicative decrease factor (e.g. 0.5).
    dec: f64,
    /// Hard floor/ceiling on the window.
    floor: u32,
    ceiling: u32,
}

impl AimdController {
    pub fn new(initial_window: u32, inc: u32, dec: f64, floor: u32, ceiling: u32) -> Self {
        assert!(floor <= ceiling, "floor must be <= ceiling");
        Self {
            state: CongestionState::new(initial_window),
            inc,
            dec,
            floor,
            ceiling,
        }
    }

    /// An ACK for chunk `idx` (or a round-trip completion): additive increase.
    pub fn on_ack(&mut self, _idx: u32) {
        self.state.round_trips = self.state.round_trips.saturating_add(1);
        let grown = self.state.window_chunks.saturating_add(self.inc);
        self.state.window_chunks = grown.clamp(self.floor, self.ceiling);
    }

    /// A NAK or timeout: multiplicative decrease.
    pub fn on_loss(&mut self, _idx: u32) {
        self.state.loss_events = self.state.loss_events.saturating_add(1);
        self.state.last_loss_at_rt = self.state.round_trips;
        let shrunk = (self.state.window_chunks as f64 * self.dec) as u32;
        self.state.window_chunks = shrunk.clamp(self.floor, self.ceiling);
    }

    pub fn state(&self) -> &CongestionState {
        &self.state
    }

    /// JSON-ish state report.
    pub fn report(&self) -> String {
        format!(
            "{{\"window\": {}, \"in_flight\": {}, \"round_trips\": {}, \"loss_events\": {}}}",
            self.state.window(),
            self.state.in_flight(),
            self.state.round_trips(),
            self.state.loss_events()
        )
    }
}

/// A capacity probe that estimates the link's sustainable goodput with a
/// moving average (EMA) so the transmitter can pace itself.
#[derive(Debug, Clone)]
pub struct CapacityProbe {
    /// EMA-smoothed goodput estimate in bytes/sec.
    estimate: f64,
    alpha: f64,
    samples: u32,
}

impl CapacityProbe {
    pub fn new(alpha: f64) -> Self {
        assert!((0.0..=1.0).contains(&alpha), "alpha must be in [0, 1]");
        Self {
            estimate: 0.0,
            alpha,
            samples: 0,
        }
    }

    /// Feed a single goodput observation (bytes/sec).
    pub fn observe(&mut self, bytes_per_sec: f64) {
        self.samples = self.samples.saturating_add(1);
        if self.samples == 1 {
            self.estimate = bytes_per_sec;
        } else {
            // EMA: new = alpha * x + (1 - alpha) * old
            self.estimate = self.alpha * bytes_per_sec + (1.0 - self.alpha) * self.estimate;
        }
    }

    pub fn estimate(&self) -> f64 {
        self.estimate
    }

    pub fn sample_count(&self) -> u32 {
        self.samples
    }
}

/// Split a link capacity (bytes/sec) among weighted flows proportionally.
#[derive(Debug, Clone)]
pub struct FlowScheduler {
    /// Total link capacity in bytes/sec.
    capacity: f64,
    flows: Vec<Flow>,
}

#[derive(Debug, Clone)]
pub struct Allocation {
    pub flow_id: u32,
    pub bytes_per_sec: f64,
}

impl FlowScheduler {
    pub fn new(capacity: f64, flows: Vec<Flow>) -> Self {
        Self { capacity, flows }
    }

    /// Proportional allocation: each flow gets capacity * (weight / total_weight).
    pub fn allocate(&self) -> Vec<Allocation> {
        let total_weight: u64 = self.flows.iter().map(|f| f.weight as u64).sum();
        if total_weight == 0 {
            return Vec::new();
        }
        self.flows
            .iter()
            .map(|f| {
                let share = (f.weight as f64) / (total_weight as f64);
                Allocation {
                    flow_id: f.id,
                    bytes_per_sec: self.capacity * share,
                }
            })
            .collect()
    }

    pub fn total_allocated(&self) -> f64 {
        self.allocate().iter().map(|a| a.bytes_per_sec).sum()
    }
}

/// Pick the AIMD parameters from a config profile (the "it depends on your
/// profile" flavor of over-engineering).
pub fn pick_aimd(config: &Config) -> AimdController {
    match config.profile {
        ConfigProfile::LowPower => AimdController::new(1, 1, 0.5, 1, 4),
        ConfigProfile::Balanced => AimdController::new(1, 1, 0.5, 1, 8),
        ConfigProfile::MaxThroughput => AimdController::new(2, 2, 0.5, 1, 16),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aimd_grows_on_ack() {
        let mut c = AimdController::new(1, 1, 0.5, 1, 8);
        assert_eq!(c.state().window(), 1);
        c.on_ack(0);
        assert_eq!(c.state().window(), 2);
        c.on_ack(1);
        assert_eq!(c.state().window(), 3);
        assert_eq!(c.state().round_trips(), 2);
    }

    #[test]
    fn aimd_shrinks_on_loss() {
        let mut c = AimdController::new(8, 1, 0.5, 1, 8);
        assert_eq!(c.state().window(), 8);
        c.on_loss(3);
        assert_eq!(c.state().window(), 4);
        assert_eq!(c.state().loss_events(), 1);
    }

    #[test]
    fn aimd_clamps_to_ceiling() {
        let mut c = AimdController::new(8, 4, 0.5, 1, 8);
        c.on_ack(0);
        assert_eq!(c.state().window(), 8); // 8+4=12 -> clamped to 8
    }

    #[test]
    fn aimd_clamps_to_floor() {
        let mut c = AimdController::new(2, 1, 0.5, 1, 8);
        c.on_loss(0);
        assert_eq!(c.state().window(), 1); // 2*0.5=1 -> floor 1
    }

    #[test]
    fn probe_ema_converges() {
        let mut p = CapacityProbe::new(0.3);
        for _ in 0..5 {
            p.observe(2_000_000.0);
        }
        assert!((p.estimate() - 2_000_000.0).abs() < 1.0);
        assert_eq!(p.sample_count(), 5);
    }

    #[test]
    fn probe_first_sample_sets_estimate() {
        let mut p = CapacityProbe::new(0.5);
        p.observe(1_000_000.0);
        assert!((p.estimate() - 1_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn scheduler_splits_equally() {
        let flows = vec![Flow { id: 1, weight: 1 }, Flow { id: 2, weight: 1 }];
        let s = FlowScheduler::new(1_000_000.0, flows);
        let alloc = s.allocate();
        assert_eq!(alloc.len(), 2);
        assert!((alloc[0].bytes_per_sec - 500_000.0).abs() < 1e-6);
        assert!((alloc[1].bytes_per_sec - 500_000.0).abs() < 1e-6);
        assert!((s.total_allocated() - 1_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn scheduler_weights() {
        let flows = vec![Flow { id: 1, weight: 3 }, Flow { id: 2, weight: 1 }];
        let s = FlowScheduler::new(4_000_000.0, flows);
        let alloc = s.allocate();
        // 3:1 split of 4M -> 3M and 1M.
        assert!((alloc[0].bytes_per_sec - 3_000_000.0).abs() < 1e-6);
        assert!((alloc[1].bytes_per_sec - 1_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn scheduler_zero_weights_yields_nothing() {
        let flows = vec![Flow { id: 1, weight: 0 }, Flow { id: 2, weight: 0 }];
        let s = FlowScheduler::new(1_000_000.0, flows);
        assert!(s.allocate().is_empty());
    }

    #[test]
    fn pick_aimd_follows_profile() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let max = Config::for_profile(ConfigProfile::MaxThroughput);
        let c_low = pick_aimd(&low);
        let c_max = pick_aimd(&max);
        assert_eq!(c_low.state().window(), 1);
        assert_eq!(c_max.state().window(), 2);
    }

    #[test]
    fn report_shape() {
        let c = AimdController::new(4, 1, 0.5, 1, 8);
        let r = c.report();
        assert!(r.contains("\"window\": 4"));
        assert!(r.contains("\"round_trips\": 0"));
    }
}
