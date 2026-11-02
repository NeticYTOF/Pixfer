// pixfer web bandwidth allocator — proportional-fair capacity split.
// Mirrors `pixfer-core/src/bandwidth_allocator.rs`.

// AllocRequest-shaped input.
export function allocate(capacityBps, requests) {
  const totalWeight = requests.reduce((a, r) => a + (r.weight || 0), 0);
  return requests.map((r) => {
    const share =
      totalWeight === 0 ? 0 : capacityBps * ((r.weight || 0) / totalWeight);
    return { jobId: r.jobId, allocatedBps: share };
  });
}

// Build an allocation from a LinkBudget-shaped { capacity } object.
export function allocateFromBudget(budget, requests) {
  return allocate(budget.capacity || 0, requests);
}

// Total allocated across all requests.
export function totalAllocated(capacityBps, requests) {
  return allocate(capacityBps, requests).reduce((a, r) => a + r.allocatedBps, 0);
}