# Shared bitset primal graph for bounded pair cutset (research only)

## Original-CNF evidence and hypothesis

The final-head original-Feynman source-locked [two-variable cutset study](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44227916721) passed 94 unit tests and 6/6 source×precision native cohorts. It did not solve original HWB6:
- 100-bit-equivalent: pinned unchanged GPMC p50 WallTime 175.786 ms, Rust Inline 501.995 ms, pair-cutset 701.494 ms.
- 333-bit-equivalent: GPMC 194.556 ms, Inline 527.624 ms, pair-cutset 728.548 ms.
- Pair admission exhausted its 180 ms structural-planning budget after approximately 14 candidate pairs; all proposed HWB6 counts fell back unchanged to Trail. This is a solver-performance No-Go.

The next hypothesis changes **only the cost of graph-only pair planning**. Neither the source CNF clauses, signed literal weights, dense width-11 factor arithmetic, fixed pair budget, nor fallback is rewritten. Planning cost is included in measured full process WallTime.

## A/B implementation

- \`factor_pair_fastgraph.rs\`: construct one undirected primal adjacency bitset per same original CNF; at most 320 vertices and five 64-bit words per row. Preserve original variable IDs and ordering.
- Rank candidates by the same original degree key \`(Reverse(degree), variable_id)\`.
- For every candidate pair, remove its two vertices in an active bitmask; simulate the **identical feasible-width-first min-fill** priority \`(bucket > 11, missing_edges, bucket, original_variable_id)\` using bitset row intersections. Add the same fill clique and stop upon the same over-width condition as the previous graph planner. Do not allocate a new CNF for each pair.
- Preserve **all** policy gates: 12 top-ranked vertices, at most 24 pair candidates, maximum root bucket 14, 180 million ns planning budget, 320 variable/3000 clause source envelope; exactly four bounded Dense Factor-VE cofactors on an admitted pair, complete unchanged Trail fallback on any cap.
- The fast path is only enabled with the explicit research options \`--factor-pair-fastgraph --factor-pair-cutset-ve\`. The existing \`--factor-pair-cutset-ve\` alone remains the original dense graph-planner control.

## Science gates and numerical semantics

1. **Planner identity:** enumerate every undirected graph with five vertices and compare old/new min-fill \`order\`, \`max_bucket\`, \`fill_edges\`, \`candidate_evaluations\`, and \`capped\` under caps 1, 2, 3, 11; compare again for representative sparse-to-denser graphs with 16, 64, 128, 285 and 320 variables and all six pairs of the first four degree-ranked candidates.
2. **Independent WMC oracle:** for a 13-variable, 78-original-clause clique requiring a two-variable cutset to reach width 11, require old/new routes both to match independent exhaustive 8192 assignments, SAT plus signed Complex real and imaginary WMC, at 100/333 bits. The old source Trail solver remains the additional cross-check. A graph already fitting dense width 11 must reject both research modes without changing WMC.
3. **Native original-Feynman same-CNF A/B:** same runner and machine, fixed original QuPRS source revision and unchanged pinned original GPMC C++, original CNF bytes and original signed literal weights, 7 rotating fresh processes per regular source×precision, equivalent 30/100 GPMC decimal digits vs Rust 100/333-bit MPFR, 30s/2GiB per child. Preserve all SAT/real/imag comparisons, measured RSS, full WallTime p50/p90, node counts, pair probes, source SHA and Grover5 censors.
4. **Go/No-Go:** A smaller planning budget is not a solver speedup by itself. If HWB6 still caps or falls back, report a negative result, including measured extra planning cost. Do not enable a default router, increase pair limits, modify QuPRS or claim GPMC parity based on synthetics.

This is an opt-in research microarchitecture hypothesis, not a new arithmetic kernel, an independent weighted solver, or a production change.
