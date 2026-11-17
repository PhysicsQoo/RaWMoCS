# Primal bag-centroid score for exact signed Complex WMC

Status: research-only independent Draft, NOT the GPMC FlowCutter
implementation, no Rust production/main or QuPRS solver-default changes.

## Source-based motivation

[Original GPMC mechanism ablation #108](https://github.com/PhysicsQoo/RaWMoCS/pull/108)
on all 14 original MQT circuit×precision cohorts showed that turning
off GPMC tree decomposition slowed **QNN4** within the same native
source/precision runner by about +27% (30 decimal digits / 100-bit MPFR
effective precision) and +22% (100 decimal / 333-bit effective).
VQE4 also slowed; QAOA7 and short one-node controls barely moved.

A source-code inspection of pinned unchanged GPMC at
89bbc89d0de918e7a5385740355df55765039230 finds:
- core/Counter.cc computeTDScore builds the CNF primal graph, uses
  FlowCutter to construct a tree decomposition and derives normalized
  variable priority from distance to a centroid bag.
- core/ComponentManager.cc pickBranchVar prioritizes the TD
  exscore, then frequency and dynamic activity for ties.
- FlowCutter/tree decomposition and model-count-safe
  component/cache/weighted-branch semantics interact; a flag
  ablation cannot identify independent additive causes.

Previous Rust naive min-fill pivot and unconditional articulation pivot
experiments regressed. Root-only articulation and bounded Boolean
failed-literal root consequences improved ModMult55 locally but
did not reduce search nodes in seven independent MQT sources.
Therefore this separate PoC does not attempt another boolean cut
heuristic on every recursive residual; it measures the value of a
source-CNF-derived, static structural **bag-centroid score**.

## Exact research transformation: ONLY branch order

Take the original QuPRS source-emitted weighted CNF after the existing
weight-compensated equivalent-literal SCC reduction, preserving the
surviving variable scope and all original signed Complex weights.

1. Construct the **primal** variable graph from original CNF clauses:
   every clause connects all participating source variables.
   No representation change, ANF, XOR synthesis or QASM-specific oracle.
2. Run bounded minimum-current-degree triangulation, recording
   elimination bags and fill edges. Create an elimination bag forest:
   each eliminated bag points to its earliest later-neighbor bag.
3. Choose a centroid of each bag tree by minimizing the largest
   remaining subtree; compute bag distances to that centroid.
   Map each original source variable to a static normalized
   inverse-distance priority, with residual clause frequency as
   a deterministic second-order tie breaker.
4. Compare two independent opt-in treatments: root-only priority
   (only DPLL depth 0, all deeper nodes Frequency) and full-depth
   priority (static score at each surviving component). A score
   that does not select a different eligible variable is a No-Op.
   The control keeps exactly the existing Frequency pivot.
5. Both Boolean Shannon branches, all MPFR literal factors,
   all component multiplications and exact weighted-cache keys
   remain fully unchanged. The variable order alone changes:
   WMC and Boolean SAT must be invariant.

This surrogate uses **min-degree bags rather than FlowCutter**;
the aim is to test whether structural centroid priority has utility
in Rust before investing in a real weighted join-tree/FlowCutter
implementation, not to claim it is the same TD algorithm as GPMC.

## Explicit bounds and fallback

Maximum 256 surviving source variables; 3,000 original clauses;
maximum clause width 32; at most 20,000 original primal edges and
20,000 fill edges. Reject any elimination bag whose size exceeds
half the surviving scope, to fail closed on dense structures.
If any bound fails, the profile is zero and the unchanged Frequency
search is preserved. The preprocessing/build time is included in
complete-process native WallTime, never hidden as setup outside timing.

Research CLI: both arms need existing --weighted-equivalence and
--trail-count, cannot be combined with the previous root failed-literal,
articulation or post-unit cache experiments. Flags:
- --trail-primal-bag-centroid
- optionally --trail-primal-centroid-root-only.

Each process logs eligibility, cap, original graph vertices/edges,
min-degree bag width, fill count, max centroid distance, profile
building time, scored search visits and actual pivot overrides.

## Native comparator, precision, safety

Four arms on one frozen original Weighted CNF input each:
original pinned unchanged GPMC C++, original Rust Inline Full,
Rust Inline + root-only bag-centroid, and Rust Inline + full-depth
bag-centroid.

Predeclared original MQT QASM sources from pinned QuPRS
0b28736651b3740ed3b201edd2efdef3da541e0d: W-state4, VQE4,
QNN4, Grover4, QWalk3, QAOA7 and QPE-exact8. These are original
MQT QASM **encoded by QuPRS**, NOT MQT-published CNFs.
Pin GPMC source 89bbc89d0de918e7a5385740355df55765039230.
Frozen input source QASM/CNF SHA, dual-dialect/phase-source checks,
equal source bytes, GPMC 30/100 decimal digits vs Rust MPFR
100/333-bit precision, 7 rotated independent native process trials
per source×precision (14/14 cohorts expected), per-child 30-second
timeout/2 GiB address-space cap. Preserve censored inputs and all
full-process WallTime p50/p90, actual RSS, DPLL nodes, cache hits,
component splits, structural build cost, profile eligibility,
pivot overrides and all numeric samples. Boolean SAT is compared
separately from **both signed Complex WMC real and imaginary values**.
Large original-CNF GPMC/Rust numeric agreement is not a mathematical
independent Full WMC certificate; added Rust tests compare same
complex dyadic input at 100/333 bits across branch orders.

The p50 improvement must come from a real pivot-order change AND
both reduced DPLL nodes and native full WallTime to support a
structure-algorithm claim. Zero actual overrides or capped
profiles are recorded as negative structure outcomes, not wins
from noise. A regression in QNN4, VQE4 or previously successful
MQT cases blocks default promotion. A censored case remains in the
denominator and fails the strict 14-cohort qualification gate.

## Next Go/No-Go

A source-specific structural performance win is only an
algorithm-selection signal, not release readiness. If this
approximate bag tree does not improve QNN4, **do not copy its
static score into production**. Distinguish failure of the
min-degree surrogate from failure of true FlowCutter TD, then
research more accurate balanced separator/join-tree dynamic
programming with explicit weighted factor ownership and exact
complex proof obligations. All related PRs remain Draft,
production Rust main and QuPRS GPMC default unchanged.
