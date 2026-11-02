//! Bandwidth allocator: proportional-fair allocation of the link's
//! estimated capacity across active transfer jobs. (Hilariously over-
//! engineered: a "scheduling layer" for a light link that physically
//! only moves a few KB/s.)

use crate::link_budget::LinkBudget;

/// One transfer job asking for a share of the link.
#[derive(Debug, Clone)]
pub struct AllocRequest {
    pub job_id: u64,
    /// Weight of this job (1 = normal priority).
    pub weight: u32,
}

/// A proportional-fair bandwidth allocator.
///
/// Given total link capacity (bytes/sec) and a set of weighted jobs,
/// allocates each job a share proportional to its weight.
#[derive(Debug)]
pub struct BandwidthAllocator {
    total_capacity: f64,
}

impl BandwidthAllocator {
    pub fn new(total_capacity: f64) -> Self {
        Self {
            total_capacity: total_capacity.max(0.0),
        }
    }

    /// Allocate bandwidth across all requests, proportional to weight.
    /// Returns (job_id, allocated_bps) pairs.
    pub fn allocate(&self, requests: &[AllocRequest]) -> Vec<(u64, f64)> {
        let total_weight: u64 = requests.iter().map(|r| r.weight as u64).sum();
        let mut out = Vec::with_capacity(requests.len());
        for r in requests {
            let share = if total_weight == 0 {
                0.0
            } else {
                self.total_capacity * ((r.weight as f64) / (total_weight as f64))
            };
            out.push((r.job_id, share));
        }
        out
    }

    /// The total capacity (bytes/sec) this allocator manages.
    pub fn capacity(&self) -> f64 {
        self.total_capacity
    }

    /// Total allocated bytes/sec across all requests.
    pub fn total_allocated(&self, requests: &[AllocRequest]) -> f64 {
        self.allocate(requests).iter().map(|&(_, b)| b).sum()
    }
}

/// Build an allocator from a `LinkBudget`, using its capacity as the total.
pub fn allocator_from_budget(budget: &LinkBudget) -> BandwidthAllocator {
    BandwidthAllocator::new(budget.capacity())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link_budget::{FlowRequest, LinkBudget};

    fn req(id: u64, w: u32) -> AllocRequest {
        AllocRequest {
            job_id: id,
            weight: w,
        }
    }

    #[test]
    fn proportional_allocation() {
        let a = BandwidthAllocator::new(10_000_000.0);
        let alloc = a.allocate(&[req(1, 1), req(2, 3)]);
        // 10MB/s total, weights 1:3 -> job1 = 2.5MB/s, job2 = 7.5MB/s
        assert!((alloc[0].1 - 2_500_000.0).abs() < 1e-6);
        assert!((alloc[1].1 - 7_500_000.0).abs() < 1e-6);
    }

    #[test]
    fn equal_weights_split_equally() {
        let a = BandwidthAllocator::new(8_000_000.0);
        let alloc = a.allocate(&[req(1, 1), req(2, 1)]);
        assert!((alloc[0].1 - 4_000_000.0).abs() < 1e-6);
        assert!((alloc[1].1 - 4_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn zero_weight_gets_zero() {
        let a = BandwidthAllocator::new(8_000_000.0);
        let alloc = a.allocate(&[req(1, 0), req(2, 0)]);
        assert_eq!(alloc[0].1, 0.0);
        assert_eq!(alloc[1].1, 0.0);
    }

    #[test]
    fn single_job_gets_all() {
        let a = BandwidthAllocator::new(6_000_000.0);
        let alloc = a.allocate(&[req(1, 5)]);
        assert!((alloc[0].1 - 6_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn total_allocated_equals_capacity() {
        let a = BandwidthAllocator::new(1_000_000.0);
        let reqs = [req(1, 1), req(2, 3)];
        assert!((a.total_allocated(&reqs) - 1_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn allocator_from_budget_uses_budget_capacity() {
        let flows = vec![
            FlowRequest {
                flow_id: 1,
                weight: 1,
                min_bps: 0.0,
            },
            FlowRequest {
                flow_id: 2,
                weight: 3,
                min_bps: 0.0,
            },
        ];
        let budget = LinkBudget::new(4_000_000.0, flows);
        let a = allocator_from_budget(&budget);
        assert!((a.capacity() - 4_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn negative_capacity_clamped() {
        let a = BandwidthAllocator::new(-5.0);
        assert_eq!(a.capacity(), 0.0);
    }
}
