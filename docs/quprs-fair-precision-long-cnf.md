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

The Rust MPFR research solver is a **blocking future dependency** for a
formal fair-performance result. Until it is implemented and independently
validated on the same source, output tables must remain labelled
`ONE_SOLVER_ONLY__NOT_GPMC_VS_RAWMOCS_FAIR_SPEEDUP`. Never fill a missing
MPFR Rust measurement with an exact-rational timing or use GPMC's
in-process reentrancy-unqualified Counter experiment.

## Engineering priorities after numeric parity

1. On QFT4/mod5_4, attribute search-node counts, component partitioning,
   cache-key work and cache hit rate; optimize total WallTime and RSS.
2. Preserve native numerical accuracy over canceling complex amplitudes
   and SAT-but-exactly-zero cases.
3. Run 30/50/100-digit precision-vs-cost curves on held-out long cases.
4. Keep precise problem source SHA, executable revisions, non-completion
   statuses and raw measurements in GitHub Actions artifacts.

This document contains no private work-tracker identifiers.
