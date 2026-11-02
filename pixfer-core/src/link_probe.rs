//! Link probe: a Ping/Pong handshake to verify the light link is alive
//! and measure the round-trip time. (Hilariously over-engineered: a
//! "ping" for a link whose whole "network" is one screen and one camera.)

use crate::timing::Clock;

/// A ping/pong probe result.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    /// Whether the peer responded.
    pub responded: bool,
    /// Round-trip time in ms (0 if no response).
    pub rtt_ms: u64,
    /// The probe's payload (echoed back).
    pub payload: Vec<u8>,
}

/// A link liveness + latency probe.
///
/// Sends a Ping, waits for a Pong, and measures the RTT.
#[derive(Debug)]
pub struct LinkProbe {
    /// Timeout for waiting for the Pong (ms).
    timeout_ms: u64,
}

impl LinkProbe {
    pub fn new(timeout_ms: u64) -> Self {
        Self { timeout_ms }
    }

    /// Run the probe: returns a `ProbeResult`.
    ///
    /// `send_ping` transmits the ping frame; `recv_pong` blocks (up to
    /// the timeout) for the pong response.
    pub fn run<F, G>(
        &self,
        send_ping: &mut F,
        recv_pong: &mut G,
        clock: &dyn Clock,
        payload: Vec<u8>,
    ) -> ProbeResult
    where
        F: FnMut(&Vec<u8>),
        G: FnMut() -> Option<Vec<u8>>,
    {
        let t0 = clock.now_ms();
        send_ping(&payload);
        // Poll for the pong up to the timeout, with a hard poll cap so a
        // stale clock can't spin forever (Hilariously over-engineered:
        // a "poll budget" for a probe that runs in a test harness).
        const MAX_POLLS: u32 = 10_000;
        let mut result: Option<Vec<u8>> = None;
        let mut polls = 0u32;
        while result.is_none() && polls < MAX_POLLS {
            polls += 1;
            result = recv_pong();
            if clock.now_ms().saturating_sub(t0) >= self.timeout_ms {
                break;
            }
        }
        match result {
            Some(p) => ProbeResult {
                responded: true,
                rtt_ms: clock.now_ms().saturating_sub(t0),
                payload: p,
            },
            None => ProbeResult {
                responded: false,
                rtt_ms: 0,
                payload: payload,
            },
        }
    }

    /// The probe's timeout (ms).
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockClock {
        now_ms: u64,
    }
    impl Clock for MockClock {
        fn now_ms(&self) -> u64 {
            self.now_ms
        }
    }

    #[test]
    fn probe_times_out_with_no_response() {
        let p = LinkProbe::new(500);
        let mut clock = MockClock { now_ms: 0 };
        let mut send = |_payload: &Vec<u8>| {};
        // Simulate a slow clock advancing past the timeout.
        clock.now_ms = 600;
        let mut recv = || -> Option<Vec<u8>> { None };
        let r = p.run(&mut send, &mut recv, &clock, vec![0xAB]);
        assert!(!r.responded);
        assert_eq!(r.rtt_ms, 0);
    }

    #[test]
    fn probe_measures_rtt() {
        let p = LinkProbe::new(1000);
        // Use a shared mutable clock via Rc<RefCell> so the send closure
        // can advance it while the probe reads it (Hilariously over-\n        // engineered: a probe for a link that can only see 8 colors.)
        let shared_clock = std::rc::Rc::new(std::cell::RefCell::new(MockClock { now_ms: 0 }));
        let c2 = std::rc::Rc::clone(&shared_clock);
        let mut send = |_payload: &Vec<u8>| {
            c2.borrow_mut().now_ms = 120; // Pong arrives after 120 ms
        };
        let mut got_pong = false;
        let c3 = std::rc::Rc::clone(&shared_clock);
        let mut recv = || -> Option<Vec<u8>> {
            let now = c3.borrow().now_ms;
            if now >= 120 && !got_pong {
                got_pong = true;
                Some(vec![0xAB])
            } else {
                None
            }
        };
        // A thin wrapper that reads the shared clock (the probe needs &dyn
        // Clock; the wrapper implements the trait by delegating to the Rc).
        struct SharedClock(std::rc::Rc<std::cell::RefCell<MockClock>>);
        impl crate::timing::Clock for SharedClock {
            fn now_ms(&self) -> u64 {
                self.0.borrow().now_ms
            }
        }
        let wrapper = SharedClock(std::rc::Rc::clone(&shared_clock));
        let r = p.run(&mut send, &mut recv, &wrapper, vec![0xAB]);
        drop(shared_clock.borrow());
        assert!(r.responded);
        assert_eq!(r.rtt_ms, 120);
        assert_eq!(r.payload, vec![0xAB]);
    }

    #[test]
    fn probe_echoes_payload() {
        let p = LinkProbe::new(1000);
        let clock = MockClock { now_ms: 0 };
        let mut send = |_payload: &Vec<u8>| {};
        let mut recv = || Some(vec![1, 2, 3]);
        let r = p.run(&mut send, &mut recv, &clock, vec![1, 2, 3]);
        assert_eq!(r.payload, vec![1, 2, 3]);
    }

    #[test]
    fn timeout_preserved() {
        let p = LinkProbe::new(2500);
        assert_eq!(p.timeout_ms(), 2500);
    }
}
