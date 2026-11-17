# Exact Factor-VE admission triage for expensive original Feynman CNFs

Research-only, source-faithful follow-on to independent [Factor-VE
MQT PR #110](https://github.com/PhysicsQoo/RaWMoCS/pull/110)
and [Feynman holdout PR #111](https://github.com/PhysicsQoo/RaWMoCS/pull/111).
No production Rust core/default, QuPRS GPMC default or original
Weighted CNF changes.

## Why precise refusal evidence is needed

The same original source/same precision full native WallTime tests
established MQT QNN4 and VQE4 as Factor-VE wins over unchanged
C++ GPMC, then Feynman Barenco5 and ModMult55 as independent
wins. Yet Feynman HWB6/100-bit stayed 633.889ms original Rust
Inline versus 642.785ms factor VE, while GPMC was 235.555ms;
333-bit Rust Inline681.741 vs FactorVE680.731 vs GPMC257.788.
Factor-VE's previous diagnostics reported completed=false,
width=0, products=0, meaning **an early structural bound prevented
any elimination**. They did not prove which cap was hit:
source variables, surviving scope, clause count, original clause
arity, initial factor table entries, or later bucket width.
It would be scientifically incorrect to claim HWB6 is
intrinsically too high-treewidth without measuring this.

Additionally Feynman Grover5/100-bit original GPMC completed,
while both Rust Inline and FactorVE were CENSORED_EXIT; the
current harness did not attempt 333 bits after that censor.
Losing a process at node cap before end-of-solver diagnostics
must not erase the original FactorVE rejection reason.

## Exact admission observability, no algorithm changes

Extend the VE research statistics to retain:
- source variable count, surviving weighted-equivalence scope size,
  original source clause count and largest source clause width;
- the **first precise fail-closed reason**:
  source_variables, source_scope, source_clauses,
  missing_weights, invalid_scope, clause_outside_scope,
  source_clause_width, source_factor_cells,
  source_unary_cells, source_factor_invalid,
  bucket_width, bucket_cells, bucket_products or
  non_scalar_final_factor;
- original factor product/bag/cell stats and completed/capped state.

Emit this source-CNF admission diagnostic **before** launching
the unchanged Trail fallback, so a hard-case node-budget
censor still retains the input envelope and rejection reason
in stderr and the archived source-relative report.
Do not alter the algebra, cap values, numeric precision,
factor ordering, input clauses, signed complex weights,
component cache or fallback logic.

Run the same original Feynman 6-cohort native GPMC/Inline/
FactorVE triple-armed test and Grover5 expensive/censored
control, preserving every raw trial and only the predeclared
source precision combinations. The prerequisite small exact
dyadic Gaussian WMC + SAT tests must continue to pass.

## Decision

If the factor cap is due to an *arbitrary admission envelope*,
study a targeted, count-safe bounded streaming factor builder
with memory evidence and a frozen test/holdout split; do not
blindly raise all resource caps, because those bounds prevent
memory and super-exponential table blowup.

If an actual large bucket width is responsible, research
exact balanced separator / join-tree DP with factor ownership
and guarded width or cutset conditioning. Preserve all
WMC addends and signed complex literals. Zero Complex WMC
is never Boolean UNSAT. If a source only succeeds by
switching to ANF/BDD or different CNF, it cannot be used as
a same-CNF solver win.

Passing diagnostics is NOT a performance improvement. Keep
this Draft and retain production GPMC/QuPRS defaults unchanged.
