# Exact Full WMC scalability research

RaWMoCS currently has an exact Gaussian-rational Full WMC research core.
Native CodSpeed is a CPU Simulation instrument, not a solver-inclusive
wall-clock measurement. Do not use it to claim superiority over other tools.

## Study admission: closed-form medium CNF families

The first controlled study extends beyond the two-variable exhaustive oracle
without using unverified solver answers as expected results:

- **Equality chain (16, 32, 64 variables):** clauses encode
  \(x_1 \leftrightarrow x_2 \leftrightarrow \cdots \leftrightarrow x_n\).
  Exactly two Boolean assignments satisfy the formula, and the exact weighted
  count is \(\prod_i w_i^+ + \prod_i w_i^-\).
- **Independent forced pairs (16, 32, 64 variables):** each pair has
  \((x \lor y) \land (\neg x \lor y)\), forcing \(y\) true while \(x\) is free.
  The exact count is the product of \(w_y^+(w_x^+ + w_x^-)\) across pairs.

Both use deterministically generated rational-complex literal weights.
They test exact Full WMC/SAT, cache-off equality, bounded cache entry counts,
and fail-closed node/depth exhaustion. The CI tests print measured
solver-node, cache-hit, decomposition and cache-entry counts; those are
algorithmic work counters, not memory RSS measurements.

The original six CodSpeed benchmark identities are preserved. Additional
named and sized simulation benchmarks measure native 16/32/64 variable
instances. Fixture construction and independent closed-form computation
are outside timed spans.

## Subsequent optimization priorities

1. **Search policy:** compare variable-choice heuristics on fixed connected
   instances; require same exact counts and SAT, A0 source-pinned CI, and
   same-runner CodSpeed results, including regression cases.
2. **Component detection:** profile allocation and union-find overhead on
   connected chains versus independent pairs before changing the algorithm.
3. **Cache policy:** report useful hits, entries and memory overhead; introduce
   a separately tested byte-aware bound before claiming memory safety.
4. **Exact Gaussian-rational arithmetic:** separate multiplication cost,
   operand bit growth and denominator normalization; never round weights
   silently or equate a zero sum with UNSAT.
5. **Resources:** node, depth and cache-entry limits are not hard wall-time
   deadlines or RSS limits. Add process isolation and fail-closed cancellation
   with independent stress and negative-control tests in a separate change.

Every algorithmic change must preserve A0 Oracle checks and the medium
analytical counts, be cross-platform tested, and publish comparisons against
an immutable CodSpeed run. One synthetic workload or a faster Simulation
result does not establish general solver scalability. Projected WMC and
QuPRS production solver replacement remain explicitly outside this stage.
