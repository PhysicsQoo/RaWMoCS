# Exact original Weighted CNF canonical key subphase attribution

Research-only, stacked on the independently source-locked original Feynman
and MQT ordered-residual shortcut studies. This changes **only opt-in
instrumentation**, its per-trial parser, and fail-closed scientific checks.
There is no new search heuristic, CNF transformation, WMC approximation,
QASM re-encoding, production default, or GPMC modification.

## Motivating source-exact observation

In [original five-arm native Feynman workflow #44250824600](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44250824600),
all six ordinary original CNF×matched precision cohorts passed, with 102
Rust tests and both signed Complex WMC fields matching unchanged GPMC.
HWB6/100-bit original unprofiled full WallTime p50: GPMC 177.418ms,
Rust Inline 513.266ms, Scoped DSU 511.194ms. The opt-in instrumented
canonical-key stage p50 was 239.303ms, versus components 62.558ms.
That aggregate *includes* source literal materialization, exact
comparison/sorting/duplicate removal, key packing, localized weighted
scope/class handling and other residual operations.

The exact ordered-key shortcut on [Draft PR #125](https://github.com/PhysicsQoo/RaWMoCS/pull/125)
skips sorting and dedup *only* when the sequence of reduced signed-literal
views is already strictly increasing. It must be proven using full
unprofiled process time; its mere admission count or signed WMC parity is
not a speedup.

## New three nested timers

Within `canonical_compact`, record the following exclusive sibling
**nested** intervals only when existing `--trail-stage-profile` is set:

- `key_materialize_ns`: allocate packed flat arena and visit the exact
  original source clauses; scope membership, live/satisfied filtering,
  optional local ordinal relabel and `View` creation are included
- `key_order_ns`: check exact strict adjacent slice order (when the
  research fastpath is enabled); either bypass or run the entire original
  unstable sort and duplicate elimination
- `key_pack_ns`: allocate and serialize the retained source clause IDs
  and all signed literal vectors, retaining exact zero separators

These are nested within existing `stage_key_ns`, not independent of it.
The residual `stage_key_ns - sum(three subtimers)` represents other
key operations such as weighted scope class preparation, relabel and
associated book-keeping. **Neither this residual nor the three separate
stage p50 values is a causal CPU attribution for uninstrumented solver.**
Timer insertion itself may change cache locality, branch prediction,
process scheduling, allocator behavior and the native WallTime.

Each emitted original Rust sample records the three nested clocks, the
existing key and total stage clocks, complete process WallTime/RSS,
the source QASM and original weighted CNF SHA, precision mapping, exact
positive/negative complex weight handling, SAT, real/imag WMC and
DPLL/cache/decomposition topology.

## Fail-closed requirements

The existing [Feynman source-pinned five-arm CI](https://github.com/PhysicsQoo/RaWMoCS/actions/workflows/cnf-ordered-residual-key-paired.yml)
and independent [original MQT seven-circuit, fourteen-cohort holdout](https://github.com/PhysicsQoo/RaWMoCS/actions/workflows/mqt-ordered-residual-key-holdout.yml)
must both pass **on this final head**. Both original pinned QuPRS and
unchanged GPMC C++ remain frozen. GPMC decimal digits 30/100 correspond
exactly to Rust MPFR 100/333 bits; complete process 30s/2GiB, p50/p90
WallTime and observed RSS remain primary metrics.

- Profiled samples: all nested timers nonnegative, total >0 and no
  sum greater than inclusive `stage_key_ns`. The parent key time must
  fit within `solver_ns`, as must the sum of all five original
  disjoint stage intervals. Preserve per-sample
  `key_subphase_coverage_fraction` and `key_unattributed_ns`.
- Non-profiled samples: all stage and nested key clocks must be zero.
- Independent Python negative-oracle preflight rejects disabled
  nonzero clocks, corrupted or zero solver time, nested double count,
  altered SAT/WMC, wrong bit precision, and search-topology drift.
- Independent exhaustive small-CNF Rust signed-complex WMC oracles and
  identical reversible Trail search/rollback for profiled/unprofiled
  at both precision levels and cache capacities 0/1/128.
- Every raw censored result is archived without selective omission.
  No blanket speedup or router-default claim unless both source
  families pass exact numeric tests and **unprofiled** full process
  measurements improve reliably.

## Decision tree

If `key_order_ns` is small while `key_materialize_ns` dominates,
classify the simple strict-order bypass as computationally insufficient
and prioritize safe reuse of exact residual source-clause materialization
with bounded allocation lifetime. If ordering is dominant but the
shortcut admission is low or its strict checking cost is significant,
do not auto-enable; investigate a bounded exact ordinal order index or
an amortized source-clause mapping, each separately source-locked.
If weighted key scope and unexplained residual dominate, isolate
`prepare_weighted_scope` and MPFR weighted-class relabel before proposing
a new optimization.

Preserve the observed slower MQT cases and censored Grover5 cases, and
avoid extrapolating from HWB6 or any one GPMC trial. No production
promotion is authorized by instrumentation alone.
