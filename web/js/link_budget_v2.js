// pixfer web extended link budget v2 — weighted allocation with per-flow
// minimums and a fairness guard.
// Mirrors `pixfer-core/src/link_budget_v2.rs`.

// A flow request in the budget.
export class FlowRequestV2 {
  constructor({ flowId = 0, weight = 1, minimum = 0 } = {}) {
    this.flowId = flowId;
    this.weight = weight;
    this.minimum = minimum;
  }
}

// An allocation result for one flow.
export class AllocationV2 {
  constructor({ flowId = 0, amount = 0, satisfiedMinimum = false } = {}) {
    this.flowId = flowId;
    this.amount = amount;
    this.satisfiedMinimum = satisfiedMinimum;
  }
}

// The v2 budget allocator: reserves per-flow minimums first, then splits
// the remainder proportionally to weights.
export class LinkBudgetV2 {
  constructor(capacity) {
    this.capacity = capacity;
  }

  // The total link capacity (B/s).
  capacityValue() {
    return this.capacity;
  }

  // Allocate the capacity across flows: minimums first, then the remainder
  // is split by weight.
  allocate(requests) {
    const totalMin = requests.reduce((a, r) => a + r.minimum, 0);
    if (totalMin > this.capacity) {
      const scale = this.capacity / totalMin;
      return requests.map(
        (r) =>
          new AllocationV2({
            flowId: r.flowId,
            amount: r.minimum * scale,
            satisfiedMinimum: false,
          }),
      );
    }
    const remainder = this.capacity - totalMin;
    const totalWeight = requests.reduce((a, r) => a + Math.max(0, r.weight), 0);
    return requests.map((r) => {
      const extra = totalWeight > 0 ? (remainder * Math.max(0, r.weight)) / totalWeight : 0;
      return new AllocationV2({
        flowId: r.flowId,
        amount: r.minimum + extra,
        satisfiedMinimum: true,
      });
    });
  }
}