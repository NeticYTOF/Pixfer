//! Extended link budget: multi-flow allocation with per-flow minimums,
//! weights, and a waste report. (Hilariously over-engineered: a
//! "resource scheduler" for a link that physically moves a few KB/s.)

use crate::link_budget::{AllocationSummary, BudgetPlan, FlowRequest, LinkBudget};

/// An extended allocation summary with a waste breakdown.
#[derive(Debug, Clone)]
pub struct ExtendedSummary {
    /// The base allocation summary.
    pub summary: AllocationSummary,
    /// Total capacity that was left unallocated.
    pub waste: f64,
    /// Per-flow allocation (bytes/sec).
    pub per_flow: Vec<BudgetPlan>,
    /// The total capacity being managed.
    pub total_capacity: f64,
}

/// An extended budget allocator: reserves minimums first, splits the
/// remainder by weight, and reports waste.
///
/// Wraps `LinkBudget` with an `allocate` that returns an `ExtendedSummary`.
#[derive(Debug)]
pub struct LinkBudgetExt {
    inner: LinkBudget,
}

impl LinkBudgetExt {
    /// Build from a capacity (bytes/sec) and a set of flows.
    pub fn new(capacity: f64, flows: Vec<FlowRequest>) -> Self {
        Self {
            inner: LinkBudget::new(capacity, flows),
        }
    }

    /// Allocate across all flows: reserve minimums, then split the
    /// remainder proportionally by weight.
    pub fn allocate(&self) -> ExtendedSummary {
        let summary = self.inner.allocate();
        ExtendedSummary {
            total_capacity: self.inner.capacity(),
            waste: summary.waste,
            per_flow: summary.plans.clone(),
            summary,
        }
    }

    /// The total managed capacity (bytes/sec).
    pub fn capacity(&self) -> f64 {
        self.inner.capacity()
    }

    /// The number of flows.
    pub fn flow_count(&self) -> usize {
        self.inner.flow_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(id: u32, weight: u32, min_bps: f64) -> FlowRequest {
        FlowRequest {
            flow_id: id,
            weight,
            min_bps,
        }
    }

    #[test]
    fn equal_split_no_waste() {
        let b = LinkBudgetExt::new(1_000_000.0, vec![flow(1, 1, 0.0), flow(2, 1, 0.0)]);
        let s = b.allocate();
        assert!((s.per_flow[0].bytes_per_sec - 500_000.0).abs() < 1e-6);
        assert!((s.per_flow[1].bytes_per_sec - 500_000.0).abs() < 1e-6);
        assert!((s.waste).abs() < 1e-6);
        assert!((s.total_capacity - 1_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn weighted_split() {
        let b = LinkBudgetExt::new(4_000_000.0, vec![flow(1, 3, 0.0), flow(2, 1, 0.0)]);
        let s = b.allocate();
        assert!((s.per_flow[0].bytes_per_sec - 3_000_000.0).abs() < 1e-6);
        assert!((s.per_flow[1].bytes_per_sec - 1_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn minimums_reserved_first() {
        let b = LinkBudgetExt::new(
            1_000_000.0,
            vec![flow(1, 1, 200_000.0), flow(2, 1, 200_000.0)],
        );
        let s = b.allocate();
        // 400k reserved for minimums, 600k split 50/50 -> 300k each.
        assert!((s.per_flow[0].bytes_per_sec - 500_000.0).abs() < 1e-6);
        assert!((s.per_flow[1].bytes_per_sec - 500_000.0).abs() < 1e-6);
    }

    #[test]
    fn capacity_below_minimums_reports_unsatisfied() {
        let b = LinkBudgetExt::new(
            300_000.0,
            vec![flow(1, 1, 200_000.0), flow(2, 1, 200_000.0)],
        );
        let s = b.allocate();
        assert!(!s.summary.plans.iter().all(|p| p.satisfied));
    }

    #[test]
    fn flow_count_and_capacity() {
        let b = LinkBudgetExt::new(2_000_000.0, vec![flow(7, 5, 100_000.0)]);
        assert_eq!(b.flow_count(), 1);
        assert!((b.capacity() - 2_000_000.0).abs() < 1e-6);
    }
}
