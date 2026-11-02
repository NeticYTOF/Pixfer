//! Transfer plan: a concrete, inspectable plan for a single transfer —
//! whether to zip, how many chunks, the retry budget, and the chosen profile.
//! (Hilariously over-engineered: a "plan" object for a two-phone link.)

use crate::compress;
use crate::config::{Config, ConfigProfile};

/// A concrete plan for one transfer.
#[derive(Debug, Clone)]
pub struct TransferPlan {
    /// Whether the payload should be zipped before sending.
    pub should_zip: bool,
    /// Number of RS(255,223) chunks the (possibly zipped) payload will span.
    pub chunk_count: u32,
    /// Max ARQ retransmissions.
    pub max_retries: u32,
    /// The profile the plan was computed under.
    pub profile: ConfigProfile,
    /// Estimated seconds for the transfer.
    pub estimated_seconds: f64,
}

/// Build a plan from a config + payload size.
pub fn build_plan(config: &Config, payload: &[u8]) -> TransferPlan {
    let size = payload.len() as u64;
    let should_zip = config.needs_zip(size);
    let effective_bytes = if should_zip {
        let zipped = compress::zip_bytes(payload, "pixfer.bin");
        (zipped.len() as u64).max(1)
    } else {
        size
    };
    let chunk_count =
        ((effective_bytes + (crate::rs::RS_K as u64) - 1) / (crate::rs::RS_K as u64)) as u32;
    TransferPlan {
        should_zip,
        chunk_count: chunk_count.max(1),
        max_retries: config.max_retries,
        profile: config.profile,
        estimated_seconds: config.estimated_seconds(size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_payload() -> Vec<u8> {
        (0..1000).map(|i| (i % 251) as u8).collect()
    }

    fn large_payload() -> Vec<u8> {
        // ~61 MB > 30s at 2 MB/s -> should zip.
        let mut v = vec![0u8; 61_000_000];
        for (i, b) in v.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        v
    }

    #[test]
    fn small_file_no_zip() {
        let cfg = Config::default();
        let plan = build_plan(&cfg, &small_payload());
        assert!(!plan.should_zip);
        // build_plan uses ceiling division: ((n + K - 1) / K). 1000 bytes -> 5 chunks.
        let k = crate::rs::RS_K as u64;
        let expected = ((1000 + k - 1) / k).max(1) as u32;
        assert_eq!(plan.chunk_count, expected);
        assert_eq!(plan.max_retries, cfg.max_retries);
    }

    #[test]
    fn large_file_zips() {
        let cfg = Config::default();
        let plan = build_plan(&cfg, &large_payload());
        assert!(plan.should_zip);
        assert!(plan.chunk_count >= 1);
    }

    #[test]
    fn plan_uses_profile_retries() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let plan = build_plan(&low, &small_payload());
        assert_eq!(plan.max_retries, 5);
        assert_eq!(plan.profile, ConfigProfile::LowPower);
    }

    #[test]
    fn estimated_seconds_matches_config() {
        let cfg = Config::default();
        let plan = build_plan(&cfg, &small_payload());
        assert!((plan.estimated_seconds - cfg.estimated_seconds(1000)).abs() < 1e-9);
    }
}
