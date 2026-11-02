// pixfer web extended link budget — multi-flow allocation with minimums,
// weights, and a waste report. Mirrors `pixfer-core/src/link_budget_ext.rs`.

// An extended allocation summary with a waste breakdown.
class ExtendedSummary {
  constructor(totalCapacity, waste, perFlow, summary) {
    this.totalCapacity = totalCapacity;
    this.waste = waste;
    this.perFlow = perFlow;
    this.summary = summary;
  }
}

// A per-flow allocation plan.
class BudgetPlan {
  constructor(flowId, bytesPerSec, satisfied) {
    this.flowId = flowId;
    this.bytesPerSec = bytesPerSec;
    this.satisfied = satisfied;
  }
}

// An extended budget allocator: reserves minimums first, splits the
// remainder by weight, and reports waste.
export class LinkBudgetExt {
  constructor(capacity, flows) {
    this.capacity = capacity;
    this.flows = flows; // [{ flowId, weight, minBps }]
  }

  // Allocate across all flows: reserve minimums, then split the remainder
  // proportionally by weight.
  allocate() {
    const reserved = this.flows.reduce((a, f) => a + (f.minBps || 0), 0);
    const remaining = this.capacity - reserved;
    const plans = [];
    let totalAllocated = 0;
    if (remaining < 0) {
      // Can't cover all minimums: greedily grant minimums while capacity allows.
      let left = this.capacity;
      for (const f of this.flows) {
        const canGrant = left >= (f.minBps || 0);
        const granted = canGrant ? (f.minBps || 0) : 0;
        if (canGrant) left -= (f.minBps || 0);
        const plan = new BudgetPlan(f.flowId, granted, canGrant);
        plans.push(plan);
        totalAllocated += granted;
      }
      return new ExtendedSummary(this.capacity, 0, plans, { totalAllocated, waste: 0, plans });
    }
    // Phase 2: split the remainder by weight.
    const totalWeight = this.flows.reduce((a, f) => a + (f.weight || 0), 0);
    for (const f of this.flows) {
      const share = totalWeight === 0 ? 0 : remaining * ((f.weight || 0) / totalWeight);
      const total = (f.minBps || 0) + share;
      const plan = new BudgetPlan(f.flowId, total, true);
      plans.push(plan);
      totalAllocated += total;
    }
    const waste = Math.max(0, this.capacity - totalAllocated);
    return new ExtendedSummary(this.capacity, waste, plans, { totalAllocated, waste, plans });
  }

  capacity() {
    return this.capacity;
  }

  flowCount() {
    return this.flows.length;
  }
}