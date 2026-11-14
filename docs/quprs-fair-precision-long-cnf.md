# Fair-precision long-CNF benchmark protocol

**Study type:** Research only. No production solver or QuPRS SolverAdapter change.

## Why the earlier cross-solver speed ranking is not fair

The original native comparison uses GPMC `-mode=1 -prec=30`, where
`core/Main.cc` sets `mpfr::mpreal::set_default_prec(mpfr::digits2bits(30))`.
The original `mpfr/mpreal.h` defines `digits2bits(d)` as
`ceil(d*3.3219280948873624)`. Thus GPMC rounds every real MPFR operation
at **100 significant binary bits**. It constructs complex products from
four real multiplications and separate rounded addition/subtraction.
The RaWMoCS baseline instead uses fully reduced, unbounded BigRational
Gaussian arithmetic on **exact finite decimal source tokens**.

**Same source CNF and close final answers are not equal arithmetic precision.**
A speed ratio between these modes is a precision-contract comparison only,
not a solver-algorithm performance ranking. The earlier source-emitted
QFT4 exact-rational timing and the research canonical-rational speedup remain
valid within their own contracts, but they do not establish a same-precision
GPMC-vs-RaWMoCS ranking.

## Required matched-precision experiment

Run identical QuPRS PathSum/WmcManager-emitted weighted CNF bytes, preserved
literal-sign weights and SAT verdict, same runner, same walltime and memory
budgets, alternating fresh processes. Align both tools on:

| Decimal precision | GPMC binary MPFR | Required RaWMoCS binary MPFR |
| --- | ---: | ---: |
| 30 digits | 100 bits | 100 bits |
| 50 digits | 167 bits | 167 bits |
| 100 digits | 333 bits | 333 bits |

GPMC's complex arithmetic implements four independently rounded products
and subsequent separately rounded adds/subtracts. A Rust `rug::Float`
research mode must use the same rounding-to-nearest convention, explicitly
materialize four intermediate MPFR products, and avoid accidental fused
multiply-add. The source decimals must each be parsed and rounded directly
at the selected precision. Bit-matched arithmetic does not guarantee
bit-identical final counts because the two solvers can sum in different
orders; compare SAT separately and check both against a stronger exact or
independent reference with predeclared tolerances.

The **primary performance cohort** is the expensive, original unchanged
Feynman `qft_4` (154 CNF vars) and `mod5_4` (43 vars), followed by
QAOA3 (33 vars) and Grover2 (23 vars); QFT3 (12 vars) is a small oracle
control. Prioritize p50/p90 and timeout/unknown frequency as future
larger-corpus costs rise, not only median microbenchmark improvements.

## Implemented GPMC precision characterization (not the matched contest)

`quprs_precision_sweep.py` now runs the original **unchanged C++ GPMC**
at 30/50/100 digits, 3 isolated native trials, on the same QuPRS
source-emitted CNFs, recording input hashes, precision bits, SAT,
numerical acceptance, WallTime, and RSS. The reference for small cases
is an independent exhaustive printed-decimal Full WMC oracle; for large
cases it is the fixed exact-rational RaWMoCS cross-solver observation,
which is **not** an independent full truth-table proof. The runner
fails closed on missing sources, censored results or unacceptable
numerical drift; it never emits a direct cross-solver speed leaderboard.

The one-solver sweep remains labelled
`ONE_SOLVER_ONLY__NOT_GPMC_VS_RAWMOCS_FAIR_SPEEDUP`. Never fill a missing
MPFR Rust measurement with an exact-rational timing or use GPMC's
in-process reentrancy-unqualified Counter experiment.

## First source-pinned matched-MPFR qualification

The isolated Rust `benchmarks/mpfr_research` executable uses `rug::Float`
with the same binary MPFR mantissa precision as the unchanged GPMC C++ CLI
(100, 167, 333 bits). Rust materializes all four real MPFR products
before adding/subtracting, preventing fused operations. Its research-only
DPLL retains the original exact solver's cache-key, component and pivot
policies. It is **not** linked to production `rawmocs-core` or QuPRS.

[Original QuPRS native CI #43584714771](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43584714771)
at branch source `8a84947b68151a3a6d733ffcd0731c76540b9de7`
completed **15/15 matched 3-trial precision/case cohorts**:
SAT verdicts and printed complex results were checked against the
independent reference where available or the fixed printed-exact
cross-solver reference otherwise. Rust DPLL node count, cache hits and
component decompositions matched the pre-existing exact-rational
source-pinned solver for each case. Unqualified results are censored.

First measurements from `quprs-mpfr-matched-long.json` artifact:

| Source-emitted circuit | Precision | GPMC native wall median | Rust MPFR native wall median | Relative outcome |
| --- | ---: | ---: | ---: | --- |
| Original Feynman qft_4 (154 vars) | 100 bits | 15.817 ms | 66.219 ms | **GPMC 4.19× faster** |
| Original Feynman qft_4 (154 vars) | 167 bits | 15.842 ms | 66.511 ms | GPMC 4.20× faster |
| Original Feynman qft_4 (154 vars) | 333 bits | 15.867 ms | 66.993 ms | GPMC 4.22× faster |
| Original Feynman mod5_4 (43 vars) | 100 bits | 11.234 ms | 6.861 ms | **Rust 1.64× faster** |
| Original Feynman mod5_4 (43 vars) | 167 bits | 11.914 ms | 6.959 ms | Rust 1.71× faster |
| Original Feynman mod5_4 (43 vars) | 333 bits | 11.629 ms | 7.046 ms | Rust 1.65× faster |
| QAOA3 (33 vars) | 100 bits | 10.235 ms | 4.516 ms | Rust 2.27× faster |
| Grover2 (23 vars) | 100 bits | 9.959 ms | 4.546 ms | Rust 2.19× faster |
| QFT3 (12 vars) | 100 bits | 9.518 ms | 4.220 ms | Rust 2.26× faster |

These are **three fresh processes per tool per precision**, not a robust
cross-machine statistically significant ranking. Same bit width and same
source input do not imply identical floating-point accumulation orders.
A reference-relative/absolute acceptance guard and independent SAT
checking are necessary; original CNF and source-phase SHA-256 values and
all raw samples are in the archive. Larger cases have **no independent
full enumeration certificate**. No quantum phase/fidelity guarantee is
inferred from the emitted finite source-decimal values.

At 100 bits the qft_4 Rust study records **5,119 DPLL nodes, 2,317
cache hits and 1,187 component splits**. Median internal Rust solve
time is **58.497 ms** (native wall 66.219 ms); input parse and
serialization are much smaller. It also records peak process RSS
**15,300 KiB Rust vs 14,156 KiB GPMC**. This is the main long-case
target for the next optimization round. The relative performance at
167/333 bits is comparable, indicating that arithmetic precision alone
is not the remaining dominant differential on this source-pinned case.

## Engineering priorities after numeric parity

1. On QFT4/mod5_4, attribute search-node counts, component partitioning,
   cache-key work and cache hit rate; optimize total WallTime and RSS.
2. Preserve native numerical accuracy over canceling complex amplitudes
   and SAT-but-exactly-zero cases.
3. Run 30/50/100-digit precision-vs-cost curves on held-out long cases.
4. Keep precise problem source SHA, executable revisions, non-completion
   statuses and raw measurements in GitHub Actions artifacts.

This document contains no private work-tracker identifiers.
