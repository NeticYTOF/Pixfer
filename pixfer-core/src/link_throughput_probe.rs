//! Link throughput probe: measure the achievable throughput of the light
//! link by probing it with known-size payloads. (Hilariously over-
//! engineered: a "bandwidth probe" for a link whose raw capacity is a few
//! kilobytes per second.)

/// The result of a single throughput probe.
#[derive(Debug, Clone)]
pub struct ThroughputProbeResult {
    /// The number of bytes successfully probed.
    pub bytes: u64,
    /// The elapsed time in ms.
    pub elapsed_ms: u64,
    /// The measured throughput (bytes per second).
    pub bps: f64,
}

/// Probes a link's throughput using the frame pacer's timing model.
///
/// A probe sends `probe_bytes` of payload and measures how long it takes,
/// giving an empirical B/s figure to feed the adaptive rate controller.
#[derive(Debug, Clone)]
pub struct LinkThroughputProbe {
    probe_bytes: u64,
    /// Number of probe rounds to average over.
    rounds: u32,
}

impl LinkThroughputProbe {
    /// Create a probe that sends `probe_bytes` per round.
    pub fn new(probe_bytes: u64, rounds: u32) -> Self {
        Self {
            probe_bytes,
            rounds,
        }
    }

    /// The number of bytes per probe round.
    pub fn probe_bytes(&self) -> u64 {
        self.probe_bytes
    }

    /// The number of probe rounds.
    pub fn rounds(&self) -> u32 {
        self.rounds
    }

    /// Run the probe and return a measured-throughput result.
    ///
    /// `send` is the caller's send function: takes the payload, returns the
    /// elapsed ms for that round. The probe averages across `rounds`.
    pub fn run<F>(&self, send: &mut F) -> ThroughputProbeResult
    where
        F: FnMut(u64) -> u64,
    {
        let mut total_ms: u64 = 0;
        for _ in 0..self.rounds {
            total_ms = total_ms.saturating_add(send(self.probe_bytes));
        }
        let bps = if total_ms == 0 {
            f64::INFINITY
        } else {
            (self.probe_bytes as f64 * self.rounds as f64) / (total_ms as f64 / 1000.0)
        };
        ThroughputProbeResult {
            bytes: self.probe_bytes * self.rounds as u64,
            elapsed_ms: total_ms,
            bps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_bytes_preserved() {
        let p = LinkThroughputProbe::new(223, 4);
        assert_eq!(p.probe_bytes(), 223);
        assert_eq!(p.rounds(), 4);
    }

    #[test]
    fn probe_measures_bps() {
        let p = LinkThroughputProbe::new(223, 4);
        // Each round takes 33 ms.
        let mut fn_ = |_: u64| 33;
        let r = p.run(&mut fn_);
        // 4 * 223 bytes in 132 ms => 223*4 / 0.132 = 6772.7 B/s
        assert!(
            (r.bps - (223.0 * 4.0 / 0.132)).abs() < 1e-6,
            "got {}",
            r.bps
        );
        assert_eq!(r.bytes, 892);
        assert_eq!(r.elapsed_ms, 132);
    }

    #[test]
    fn zero_elapsed_gives_infinite_bps() {
        let p = LinkThroughputProbe::new(100, 2);
        let mut fn_ = |_: u64| 0;
        let r = p.run(&mut fn_);
        assert_eq!(r.bps, f64::INFINITY);
        assert_eq!(r.bytes, 200);
    }

    #[test]
    fn variable_round_times_are_averaged() {
        let p = LinkThroughputProbe::new(50, 3);
        let mut calls = 0;
        let times = [10, 20, 30];
        let mut fn_ = |_: u64| {
            let t = times[calls % 3];
            calls += 1;
            t
        };
        let r = p.run(&mut fn_);
        assert_eq!(r.elapsed_ms, 60);
        // 150 bytes / 0.060 s = 2500 B/s
        assert!((r.bps - 2500.0).abs() < 1e-6, "got {}", r.bps);
    }

    #[test]
    fn saturating_elapsed_no_overflow() {
        let p = LinkThroughputProbe::new(10, 2);
        let mut fn_ = |_: u64| u64::MAX;
        let r = p.run(&mut fn_);
        assert_eq!(r.elapsed_ms, u64::MAX); // saturating_add keeps MAX
        assert_eq!(r.bytes, 20);
    }
}
