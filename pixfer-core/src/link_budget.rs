//! Link budget: split the available symbol rate among active flows.
//! (Hilariously over-engineered: a resource-budget allocator for a link
//! that has a single screen and a single camera.)

use crate::config::{Config, ConfigProfile};

/// A flow requesting a share of the link.
#[derive(Debug, Clone)]
pub struct FlowRequest {
    pub flow_id: u32,
    /// Relative priority weight (higher = bigger share).
    pub weight: u32,
    /// Minimum bytes/sec this flow needs to function.
    pub min_bps: f64,
}

/// The result of a budget allocation for one flow.
#[derive(Debug, Clone)]
pub struct BudgetPlan {
    pub flow_id: u32,
    pub bytes_per_sec: f64,
    /// Whether the minimum was satisfied.
    pub satisfied: bool,
}

/// A budget plan summary: total allocated + waste (capacity left unallocated).
#[derive(Debug, Clone)]
pub struct AllocationSummary {
    pub total_allocated: f64,
    pub waste: f64,
    pub plans: Vec<BudgetPlan>,
}

/// The link budget allocator: takes a link capacity and a set of flows and
/// computes a per-flow allocation that respects minimums and weights.
#[derive(Debug, Clone)]
pub struct LinkBudget {
    /// Total link capacity in bytes/sec.
    capacity: f64,
    flows: Vec<FlowRequest>,
}

impl LinkBudget {
    pub fn new(capacity: f64, flows: Vec<FlowRequest>) -> Self {
        Self { capacity, flows }
    }

    /// Allocate: first guarantee each flow its `min_bps`, then split the
    /// remainder proportionally by weight.
    pub fn allocate(&self) -> AllocationSummary {
        let mut plans: Vec<BudgetPlan> = Vec::with_capacity(self.flows.len());

        // Phase 1: reserve minimums.
        let reserved: f64 = self.flows.iter().map(|f| f.min_bps).sum();
        let remaining = self.capacity - reserved;
        if remaining < 0.0 {
            // Can't even cover all minimums. Greedily grant each flow its
            // minimum only while the remaining capacity can still cover it;
            // flows that don't fit are marked unsatisfied.
            let mut left = self.capacity;
            for f in &self.flows {
                let can_grant = left >= f.min_bps;
                if can_grant {
                    left = left - f.min_bps;
                }
                plans.push(BudgetPlan {
                    flow_id: f.flow_id,
                    bytes_per_sec: if can_grant { f.min_bps } else { 0.0 },
                    satisfied: can_grant,
                });
            }
            let allocated: f64 = plans.iter().map(|p| p.bytes_per_sec).sum();
            return AllocationSummary {
                total_allocated: allocated,
                waste: 0.0,
                plans,
            };
        }

        // Phase 2: split the remainder by weight.
        let total_weight: u64 = self.flows.iter().map(|f| f.weight as u64).sum();
        for f in &self.flows {
            let share = if total_weight == 0 {
                0.0
            } else {
                remaining * ((f.weight as f64) / (total_weight as f64))
            };
            plans.push(BudgetPlan {
                flow_id: f.flow_id,
                bytes_per_sec: f.min_bps + share,
                satisfied: true,
            });
        }

        let total_allocated: f64 = plans.iter().map(|p| p.bytes_per_sec).sum();
        let waste = (self.capacity - total_allocated).max(0.0);
        AllocationSummary {
            total_allocated,
            waste,
            plans,
        }
    }

    pub fn capacity(&self) -> f64 {
        self.capacity
    }
}

/// Pick the default link capacity from a config profile (the "it depends on
/// your profile" flavor of over-engineering).
pub fn default_capacity(config: &Config) -> f64 {
    match config.profile {
        ConfigProfile::LowPower => 1_000_000.0,
        ConfigProfile::Balanced => 2_000_000.0,
        ConfigProfile::MaxThroughput => 4_000_000.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(id: u32, weight: u32, min: f64) -> FlowRequest {
        FlowRequest {
            flow_id: id,
            weight,
            min_bps: min,
        }
    }

    #[test]
    fn equal_split() {
        let b = LinkBudget::new(1_000_000.0, vec![flow(1, 1, 0.0), flow(2, 1, 0.0)]);
        let s = b.allocate();
        assert!((s.plans[0].bytes_per_sec - 500_000.0).abs() < 1e-6);
        assert!((s.plans[1].bytes_per_sec - 500_000.0).abs() < 1e-6);
        assert!((s.total_allocated - 1_000_000.0).abs() < 1e-6);
        assert!((s.waste).abs() < 1e-6);
    }

    #[test]
    fn weighted_split() {
        let b = LinkBudget::new(4_000_000.0, vec![flow(1, 3, 0.0), flow(2, 1, 0.0)]);
        let s = b.allocate();
        assert!((s.plans[0].bytes_per_sec - 3_000_000.0).abs() < 1e-6);
        assert!((s.plans[1].bytes_per_sec - 1_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn minimums_reserved_first() {
        let b = LinkBudget::new(
            1_000_000.0,
            vec![flow(1, 1, 200_000.0), flow(2, 1, 200_000.0)],
        );
        let s = b.allocate();
        // 400k reserved for minimums, 600k remaining split 50/50 -> 300k each
        // -> total per flow = 200k + 300k = 500k.
        assert!((s.plans[0].bytes_per_sec - 500_000.0).abs() < 1e-6);
        assert!((s.plans[1].bytes_per_sec - 500_000.0).abs() < 1e-6);
    }

    #[test]
    fn capacity_below_minimums() {
        let b = LinkBudget::new(
            300_000.0,
            vec![flow(1, 1, 200_000.0), flow(2, 1, 200_000.0)],
        );
        let s = b.allocate();
        // Only 300k capacity for 400k of minimums -> not everything satisfied.
        assert!(!s.plans.iter().all(|p| p.satisfied));
    }

    #[test]
    fn single_flow_gets_all() {
        let b = LinkBudget::new(2_000_000.0, vec![flow(7, 5, 100_000.0)]);
        let s = b.allocate();
        assert!((s.plans[0].bytes_per_sec - 2_000_000.0).abs() < 1e-6);
        assert!(s.plans[0].satisfied);
        assert!((s.waste).abs() < 1e-6);
    }

    #[test]
    fn zero_weight_flows_get_only_minimums() {
        let b = LinkBudget::new(
            1_000_000.0,
            vec![flow(1, 0, 100_000.0), flow(2, 0, 100_000.0)],
        );
        let s = b.allocate();
        // No weight to split; remainder unallocated -> waste.
        assert!((s.plans[0].bytes_per_sec - 100_000.0).abs() < 1e-6);
        assert!((s.waste - 800_000.0).abs() < 1e-6);
    }

    #[test]
    fn default_capacity_follows_profile() {
        let low = Config::for_profile(ConfigProfile::LowPower);
        let max = Config::for_profile(ConfigProfile::MaxThroughput);
        assert_eq!(default_capacity(&low), 1_000_000.0);
        assert_eq!(default_capacity(&max), 4_000_000.0);
    }
}
