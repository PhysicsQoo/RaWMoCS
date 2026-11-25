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

## 2026-11-19 final-head evidence: bounded admission no-go and cache/key bottleneck

Source-locked [Feynman five-arm CI #44454383757](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44454383757),
[MQT five-arm CI #44454383856](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44454383856)
and [Rust Science #44454383763](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44454383763)
all succeeded: **55/55 workflow runs on research commit `1692f902`**.
Every selected source/precision cohort preserved source-locked original QuPRS
CNF, unchanged GPMC, exact SAT and signed Complex real/imag WMC and exact
Trail fallback. Existing independent four-arm reports remain separate from the
new five-arm reports. The new cheap 256-variable scope refusal is a strictly
opt-in negative control, not the production Router.

### Paired native cost of structural refusal

All entries below are same-batch seven fresh processes, complete-process
**p50 WallTime in ms**. Precision is equivalent across original GPMC
30/100 decimal digits and Rust 100/333 MPFR bits.

| Source / bits | GPMC | Rust Inline | bag14 | cost preflight | quick scope256 |
|---|---:|---:|---:|---:|---:|
| HWB6 / 100 | 173.686 | 516.585 | 609.549 | 538.461 | 523.847 |
| HWB6 / 333 | 192.099 | 544.547 | 681.277 | 572.834 | 555.145 |
| QNN4 / 100 | 15.588 | 32.757 | 7.624 | 7.398 | 7.276 |
| QNN4 / 333 | 16.078 | 34.496 | 8.234 | 8.093 | 7.889 |
| QPEexact8 / 100 | 4.396 | 3.968 | 6.046 | 6.077 | 6.077 |
| QPEexact8 / 333 | 4.449 | 4.103 | 6.649 | 6.761 | 6.733 |

HWB6 quick-scope control gains 14.1% / 18.5% against the previously
rejected bag14 arm, but wins **zero of seven** same-trial comparisons against
original Inline at either precision. The new arm remains about **3x slower
than unchanged GPMC** and is **NO-GO** as a general optimization. QNN4 and
Barenco5/ModMult55 retain selected exact Factor-VE benefits, while QPEexact8
shows additional non-beneficial planning overhead. A selective fast path
needs a separately demonstrated, source-structural admission rule, not a
global switch to Factor-VE or merely a cheaper negative admission.

### Independent source-locked native stage and CodSpeed callgraph

[Native Feynman stage attribution #44454383848](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44454383848):
for HWB6 100-bit the *profiled* cumulative key stage was **254.237 ms**,
lookup **28.581 ms**, unit **36.942 ms**, frequency **31.570 ms**, components
**63.311 ms**. Same-run *unprofiled* Rust Inline complete-process p50 was
**518.209 ms**, GPMC **175.060 ms**. At 333-bit key stage **257.004 ms**,
lookup 28.731 ms, components 63.635 ms, unprofiled Inline 545.721 ms
and GPMC 192.676 ms. These are instrumented stage totals, not an
exhaustive decomposition of or causal percentage of an uninstrumented run.

The [CodSpeed simulated flamegraph of original HWB6 Inline](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6afe78be6b5fd92a3624a2e9?uri=exec_harness%3A%3Aoriginal_cnf%2Fhwb6%2Frust_equivalence_trail_inline_full_mpfr100&runnerMode=Simulation)
on the same source-locked research revision reports **4.698 s simulated
CPU time** including **2.584 s modeled cache-miss cost (55.0%)** and
**1.508 s instruction cost (32.1%)**; the
`TrailEngine::canonical_compact::{closure#0}` self-time is **1.227 s
(26.1%)** and `count_relevant` self-time about **677 ms (14.4%)**,
whereas `mpfr_mul` self-time is about **93 ms (2.0%)**.
The [unchanged GPMC simulated counterpart](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6afe78be6b5fd92a3624a2e9?uri=exec_harness%3A%3Aoriginal_cnf%2Fhwb6%2Fgpmc_mpfr100&runnerMode=Simulation)
is **1.731 s simulated**, with approximately **1.087 s modeled cache cost**.
**CodSpeed Simulation is not native WallTime**, and its call counts /
recursive aggregation must not be interpreted as direct per-node counts.
This is corroborating hotspot evidence, not a new quantitative speed claim.

### Next falsifiable algorithmic experiment

Prior exact ordered-key shortcut and bounded arena reuse are independently
benchmarked already; do not silently rebrand or re-run these as new algorithms.
Target **original exact residual-key materialization and cache-locality
cost** while retaining component identity, deterministic source clause order,
signed literal polarity, weight-class ownership, collision-safe equality,
separate SAT and Complex WMC channels, reversible Trail and original
fallback. Prototype **one opt-in change at a time**, report the read-only
canonical-key materialization/ordering/packing subphase evidence first, then
the same original CNF seven-trial full-process p50/p90/RSS against unchanged
GPMC and Inline with cross-family positive/negative holdouts and capped
Grover5 controls. Exact cache hits or lower simulated CPU time alone are
not promotion criteria. No source changes to GPMC or QuPRS, no extra PR
stack and no production default/Router change.
