# Bounded Boolean unit-closure cofactor projections for Weighted CNF search

**Status: isolated research-only; no production or QuPRS solver selection change.**

## Why this is a new, independently controlled experiment

The earlier [direct-cofactor separator experiment](https://github.com/PhysicsQoo/RaWMoCS/pull/88)
passed MPFR science but *regressed* all six completed source/precision
cohorts against weighted-equivalence-only Rust. On original HWB6 at 100
bits, the full-process p50 increased from 455.451 to 550.137 ms and DPLL
nodes from 81,594 to 93,404. Barenco5's 36% node reduction did not
improve total process WallTime. Those results are **negative evidence**,
not a launchpad for default changes.

This separate branch starts from the [signed equivalent-literal solver
research](https://github.com/PhysicsQoo/RaWMoCS/pull/87), not from the
unqualified separator PR. The only new solver behavior is opt-in
`--unit-closure-pivot`; the official Rust core, default Frequency,
Cache policy and QuPRS adapter remain unchanged.

## Exact WMC semantics

For a candidate literal x, temporarily assume x and repeatedly find
Boolean unit literals of the **same source-CNF residual** to a fixed
point. Detect Boolean contradiction only when a clause becomes empty.
After that, estimate independent variable-incidence components of the
remaining, unsatisfied clauses using DSU. These *temporary* assignments,
simplified clauses and components are **never used as a WMC result**.

The real solver still implements both branches of:

    WMC(F) = w(x=true) * WMC(F given x=true)
           + w(x=false) * WMC(F given x=false).

All positive/negative literal weights, complex MPFR operations, actual
unit propagation, component multiplication, Cache keys and SAT status
remain in the original DPLL engine. Complex numerical zero **never
implies Boolean UNSAT**. No native QASM, Path-sum, RR, ANF, BDD or
alternate CNF is supplied to the solver. The optional weighted
equivalence SCC preprocessing is separately identified and measured
inside process WallTime.

Selecting a different pivot can change the final MPFR rounding order
for non-dyadic weights. Large circuits require matched-precision
complex numerical agreement, not byte-for-byte stdout equality; small
dyadic complex science tests require exact equality.

## Safe admission and work budgets

Only residuals with 6–640 active variables, at most 1,300 clauses and
7,000 literals enter the probe. At most 64 residual nodes, six
high-frequency candidate pivots per node (including Frequency
baseline), and 500,000 inspected literals globally per solver process.
An exhausted probe returns unmodified Frequency and disables all
further probes. No partially computed projection may affect a WMC
answer.

The algorithm compares against *the actual Frequency pivot's own
unit-propagated True/False cofactors*. It changes the pivot only when
both candidate branches split into at least two components (or one
branch is Boolean contradictory and the other splits), and the worst
retained component is reduced by at least 20%, with a two-variable
minimum margin and a 12-variable minimum baseline. This gate is a
research hypothesis, **not** a proof of future speedup. Unchanged
Frequency is a valid, expected no-benefit outcome.

Diagnostics: `RESEARCH_MPFR_UNIT_CLOSURE_PIVOT` prints
`enabled, scans, candidates, chosen, rejected, work, forced, conflicts,
censored`. A forced implication is Boolean only; these counters are
observability, not an independent complete-model WMC certificate.

## Scientific checks and paired comparison

- Unit-closure witness with two groups hidden until an implied literal
  is propagated, and a separate Boolean contradiction control.
- Dyadic signed/complex 100/333-bit MPFR Full WMC against unchanged
  Frequency, including mixed polarity, SAT-but-zero and UNSAT.
- Small-active negative controls and strict exhausted-work fallback.
- Strict Clippy and module Rustfmt in dedicated science workflow.
- Unmodified original QuPRS source SHA
  `0b28736651b3740ed3b201edd2efdef3da541e0d`, original
  GPMC SHA `89bbc89d0de918e7a5385740355df55765039230`.
- Identical source-generated Weighted CNF bytes, positive/negative
  Complex weights, precision 100/333 MPFR bits, same Ubuntu runner,
  30 s and 2 GiB virtual-address-space guard; never confuse this with
  physical RSS enforcement.
- Seven interleaved independent full-process trials for every completed
  original Feynman HWB6, Barenco5 and ModMult55 cohort; single bounded
  all-arm Grover5 pilot. GPMC original C++, Rust unchanged Frequency,
  Rust + signed equivalent-literal elimination, Rust equivalence +
  unit-closure pivot. Record CNF SHA, complete SAT/Full Complex WMC
  components, p50/p90 process WallTime, solve time, max observed RSS,
  DPLL nodes/cache/components, probe work, all failed/UNKNOWN/censored.
  QuPRS source fixture preparation is **outside** native solver time
  and cannot be called a Rust solver optimization.

The sole performance-admission gate is repeated, source-identical,
matched-precision *full process* WallTime benefit on **multiple
long-running sources without severe regressions**. A single tiny
example, solve-only microbenchmark, reduced DPLL nodes without
end-to-end gains, or a censored Grover5 result cannot qualify it.

### Reproduction

- `benchmarks/mpfr_research/src/unit_closure_pivot.rs`
- `benchmarks/rust_core/tools/quprs_cnf_unit_closure_ab.py`
- `.github/workflows/cnf-unit-closure-science.yml`
- `.github/workflows/cnf-unit-closure-paired.yml`

The branch is intentionally Draft until the exact-head CI result is
qualified. A failed test, missing CNF or an over-budget candidate
never licenses production admission.
