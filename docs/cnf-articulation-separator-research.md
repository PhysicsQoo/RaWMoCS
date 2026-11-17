# Research: exact articulation-point pivot on original weighted CNF

Status: independent, opt-in Draft; **no production or QuPRS solver-default changes**.

## Motivation and source isolation

[Same-source original GPMC controlled ablation](https://github.com/PhysicsQoo/RaWMoCS/pull/103)
established structural sensitivity. At HWB6 100-bit, unchanged GPMC p50
228.213 ms, GPMC `-no-td` 163.875 ms, and `-no-pp_ee` 140.765 ms.
For Grover5/100, default GPMC completes but `-no-td` is time-censored
at 30s. This is evidence of *opposite workload-dependent behavior*,
not evidence that GPMC's FlowCutter algorithm should be copied blindly.

PR #102's source-identical 18-arm Rust full-process native study showed
HWB6/100 Inline 500.597 ms, projected 518.155 ms and fused post-unit
504.839 ms vs GPMC 176.198 ms. Fusing traversals improved one variant
but never beat Inline on six qualified source×precision cohorts.
Prior naive min-fill pivot experiments regressed search nodes. Thus a
bounded, condition-triggered **structural separator pivot** is a better
isolated hypothesis than more micro-optimizations.

## Exact method

On the current residual **bipartite variable–clause incidence graph**,
run Tarjan lowlink to find an *articulation variable*. Assigning such a
variable can split the graph into multiple independent component pieces.
All remaining CNF clauses and weights stay exactly as before. Crucially,
this is only a **branch ordering heuristic**: the original chronological
Shannon expansion still sums *both* branches with their original signed
complex literal weights. The existing component decomposition multiplies
independent component counts only after branching. No SAT-only
equisatisfiable rewrite, no unproved Gaussian-elimination counting, no
assumption that XOR structure was present.

Bounds: <=256 active variables, <=4096 candidate source clause IDs,
<=8192 active variable–clause incidences. If exceeded, the graph has
unexpected out-of-scope variables, or unit closure was incomplete, skip
the graph search and use unchanged Frequency pivot. Otherwise rank
articulation variables by descending number of resulting pieces, then
frequency and deterministic variable ID. Record probes, qualifying
candidates, chosen separator pivots and skipped checks separately.

The opt-in flag `--trail-articulation-pivot` requires `--trail-count`,
and is isolated from post-unit cache experiments. The unchanged
production Core Rust solver, GPMC, QuPRS adapter and frequency default
are not modified.

## Exact weighted-CNF native evidence contract

- Original source-pinned QuPRS commit
  `0b28736651b3740ed3b201edd2efdef3da541e0d` and unchanged
  GPMC C++ `89bbc89d0de918e7a5385740355df55765039230`.
- Source QASM and original emitted Weighted CNF SHA-256; each frozen
  CNF reused *byte-identically* by unchanged GPMC, Rust Inline control,
  and Rust Inline plus articulation, at GPMC 30/100 decimal digits vs
  Rust 100/333-bit MPFR.
- Same native Linux runner, per-process 30s wall timeout and 2 GiB
  address-space limit; seven interleaved fresh processes per qualified
  source×precision cohort, costly Grover5 one trial flagged as censored
  where applicable.
- Compare SAT as a Boolean **separately** from both signed Complex
  WMC components; preserve exact source result, per-arm stdout hash,
  DPLL nodes/cache hits/decomposition, separator choices, p50/p90
  complete-process WallTime, observed RSS and failures. Small dyadic
  SAT/UNSAT/cancelling-zero cases have independent WMC reference
  parity tests in the Rust research science suite.
- If the actual QuPRS CNF has **no useful articulation points**, record
  that as an explicit negative structural outcome; do not claim
  an unobserved Tree Decomposition benefit or a Rust performance win.

## Go/No-Go

Promote **neither** the heuristic nor any default from synthetic
tests. Require six positive original-CNF source×precision same-run
numerical cohorts, no new resource regressions, reduction of both
full native WallTime and DPLL nodes on meaningful source workloads,
and a credible improvement on censored hard instances. If no consistent
benefit, retain the benchmark evidence and move to a true multi-variable
separator / tree decomposition or exact weighted-safe equivalence
preprocessor PoC on the *same* source CNF. All code remains research-only.


## 2026-11-17 follow-up: root-only counterexample after recursive No-Go

[Initial all-depth native CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44005152394)
did **not** meet its six-cohort acceptance gate: HWB6 100/333-bit
Articulation arm was censored in each of seven fresh-process trials,
even though unchanged GPMC and Rust Inline completed; Grover5 remained
censored for both Rust arms. Four short source×precision cohorts
qualified, but all four regressed versus Inline. For ModMult55,
repeated cut-vertex selection made 1,683 separator pivots; nodes rose
from 3,879 to 6,985, and p50 native WallTime from 17.089 ms to
33.533 ms at 100 bits (333 bits: 18.126 to 35.296 ms).
This is negative algorithmic evidence, not merely a test-script bug.
Never weaken the original six-cohort qualification threshold.

The independent root-only arm keeps exactly the same original CNF
and signed complex literal weights; the only change is that factor-graph
articulation probing is allowed **at DPLL depth zero only**, then
Frequency is used recursively. The guarded opt-in flag
`--trail-articulation-root-only` requires
`--trail-articulation-pivot` and remains incompatible
with post-unit caching. The runner compares unchanged GPMC, Inline,
and Root-Only in seven fresh order-rotated paired trials for all
complete original source×100/333-bit cohorts, preserving all failures,
Boolean SAT and both signed Complex WMC components, nodes,
p50/p90 full WallTime and peak RSS, and separator counts.

A root-only cap avoids repeated graph analysis but can still choose a
harmful root and expand the total search. **This is not claimed as a win.**
Retain the original six-cohort qualification gate; a production default
also requires material native WallTime and DPLL-node improvements
without hard-case regression against Inline. If root-only is negative,
do not keep tuning arbitrary separator cut ordering; instead research
comparative bounded cofactor costs and exact signed-weight-preserving
preprocessing with independent Gaussian WMC controls.

No changes to the production Rust core or QuPRS GPMC default.
