//! Extended link budget v2: weighted allocation with per-flow minimums and
//! a fairness guard. (Hilariously over-engineered: a "bandwidth governor"
//! for a link whose total capacity is a few KB/s.)

/// A flow request in the budget.
#[derive(Debug, Clone)]
pub struct FlowRequestV2 {
    pub flow_id: u16,
    pub weight: f64,
    pub minimum: f64,
}

/// An allocation result for one flow.
#[derive(Debug, Clone)]
pub struct AllocationV2 {
    pub flow_id: u16,
    pub amount: f64,
    pub satisfied_minimum: bool,
}

/// The v2 budget allocator: reserves per-flow minimums first, then splits
/// the remainder proportionally to weights.
#[derive(Debug)]
pub struct LinkBudgetV2 {
    capacity: f64,
}

impl LinkBudgetV2 {
    pub fn new(capacity: f64) -> Self {
        Self { capacity: capacity }
    }

    /// The total link capacity (B/s).
    pub fn capacity(&self) -> f64 {
        self.capacity
    }

    /// Allocate the capacity across flows: minimums first, then the
    /// remainder is split by weight.
    pub fn allocate(&self, requests: &[FlowRequestV2]) -> Vec<AllocationV2> {
        let total_min: f64 = requests.iter().map(|r| r.minimum).sum();
        if total_min > self.capacity {
            // Not enough capacity for all minimums: give each flow its
            // minimum scaled down proportionally.
            let scale = self.capacity / total_min;
            return requests
                .iter()
                .map(|r| AllocationV2 {
                    flow_id: r.flow_id,
                    amount: r.minimum * scale,
                    satisfied_minimum: false,
                })
                .collect();
        }
        let remainder = self.capacity - total_min;
        let total_weight: f64 = requests.iter().map(|r| r.weight.max(0.0)).sum();
        let mut out: Vec<AllocationV2> = Vec::with_capacity(requests.len());
        for r in requests {
            let extra = if total_weight > 0.0 {
                remainder * (r.weight.max(0.0) / total_weight)
            } else {
                0.0
            };
            out.push(AllocationV2 {
                flow_id: r.flow_id,
                amount: r.minimum + extra,
                satisfied_minimum: true,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: u16, weight: f64, minimum: f64) -> FlowRequestV2 {
        FlowRequestV2 {
            flow_id: id,
            weight,
            minimum,
        }
    }

    #[test]
    fn equal_weights_split_equally() {
        let b = LinkBudgetV2::new(100.0);
        let allocs = b.allocate(&[req(1, 1.0, 0.0), req(2, 1.0, 0.0)]);
        assert!((allocs[0].amount - 50.0).abs() < 1e-9);
        assert!((allocs[1].amount - 50.0).abs() < 1e-9);
    }

    #[test]
    fn weighted_split() {
        let b = LinkBudgetV2::new(100.0);
        let allocs = b.allocate(&[req(1, 3.0, 0.0), req(2, 1.0, 0.0)]);
        assert!((allocs[0].amount - 75.0).abs() < 1e-9);
        assert!((allocs[1].amount - 25.0).abs() < 1e-9);
    }

    #[test]
    fn minimums_reserved_first() {
        let b = LinkBudgetV2::new(100.0);
        let allocs = b.allocate(&[req(1, 1.0, 20.0), req(2, 1.0, 30.0)]);
        // Minimums (20+30=50) reserved, remainder 50 split equally (25 each).
        assert!((allocs[0].amount - 45.0).abs() < 1e-9);
        assert!((allocs[1].amount - 55.0).abs() < 1e-9);
        assert!(allocs[0].satisfied_minimum && allocs[1].satisfied_minimum);
    }

    #[test]
    fn below_minimums_scales_down() {
        let b = LinkBudgetV2::new(50.0);
        let allocs = b.allocate(&[req(1, 1.0, 30.0), req(2, 1.0, 40.0)]);
        // total_min = 70 > 50, scale = 50/70.
        assert!((allocs[0].amount - (30.0 * 50.0 / 70.0)).abs() < 1e-9);
        assert!((allocs[1].amount - (40.0 * 50.0 / 70.0)).abs() < 1e-9);
        assert!(!allocs[0].satisfied_minimum);
    }

    #[test]
    fn zero_weight_gets_only_minimum() {
        let b = LinkBudgetV2::new(100.0);
        let allocs = b.allocate(&[req(1, 0.0, 10.0), req(2, 5.0, 0.0)]);
        assert!((allocs[0].amount - 10.0).abs() < 1e-9);
        assert!((allocs[1].amount - 90.0).abs() < 1e-9);
    }
}
