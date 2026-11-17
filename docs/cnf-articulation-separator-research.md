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
