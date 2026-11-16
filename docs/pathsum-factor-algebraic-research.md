# Native Path-sum factor elimination and quadratic sign-phase research

Status: OPT-IN ALGORITHM PROTOTYPE — 7-trial same-RR MPFR comparison qualified on one Linux CI runner; not production or general-performance qualified.

This study does not reproduce GPMC's CDCL search architecture. It uses
QuPRS's original ANF/parity, AND monomials, phase factors, and trace/output
constraints before they are expanded into Tseitin CNF.

## Fixed QuPRS source analysis

- Source: https://github.com/PhysicsQoo/QuPRS/blob/0b28736651b3740ed3b201edd2efdef3da541e0d/rust/quprs/src/wmc.rs
- QuPRS encode_anf creates chained four-clause Tseitin encodings for XOR.
- QuPRS encode_phase_polynomial turns each phase monomial into a weighted
  auxiliary with a definitional AND equivalence.
- Output trace equalities and otherwise unused free Path-sum variables
  must remain in the full count.
- Thus, native factor inference is a plausible independent alternative
  to repeated generic CNF search. A limited same-RR QuPRS source cohort has now shown measured benefits; this is not yet a general QuPRS E2E admission.

## A. Opt-in bounded mixed-factor DP

Source: benchmarks/rust_core/src/pathsum_factor_research.rs
Feature: pathsum-factor-research (not in default/release builds).

PathsumFactors owns a declared Boolean variable set, independent positive and
negative exact Gaussian rational literal weights, XOR parity equations,
nonlinear ANF output equalities, conditional phase monomials, and global scalar.
No added Tseitin auxiliary is needed.

The algorithm builds explicit bounded factor tables, repeatedly chooses a
variable minimizing the predicted joined scope, performs factor multiplication
on shared variables, then sums over the eliminated variable. A factor table
cell carries both the Complex WMC and independent SAT support. Therefore a
satisfiable instance whose weighted sum cancels to zero remains SAT.

Research limits default to joined scope <=12 and total work <=1,000,000
visited table cells. Overflow returns explicit failure, never partial WMC.
Phase weights are exact for the *given printed Gaussian rational inputs*,
not arbitrary source-exact trigonometric phases.

The included structural profile reports original variable count, native XOR
and output-equality counts, phase/ANF degrees, max scope, and a primal min-fill
upper bound (not exact treewidth or cut-rank). The current solver also has
no general CNF clause factor. Separate fixed-source post-RR Gaussian and MPFR exporters are now available in the research tools.

## B. Independent restricted quadratic sign-phase evaluator

QuadraticSign accepts only a quadratic Boolean exponent with phase (-1)^Q,
affine GF(2) equations, unit variable weights, and an exact Gaussian scalar.
This is **not** general Clifford+T, modulo-four Clifford, arbitrary-angle
or general weighted quadratic evaluation.

First eliminate affine equations over GF(2) to derive a particular solution
and basis. Substitute into the quadratic polynomial, then eliminate isolated
variables or interacting pairs. The exact identity

    sum_{x,y in {0,1}} (-1)^(x*y + a*x + b*y) = 2*(-1)^(a*b)

allows pair elimination over the remaining variables, while retaining
a separate SAT flag when a sum vanishes by cancellation.
The restricted algorithm is polynomial-time in its encoded input size.

## Tests, reproducibility and stop conditions

Tests compare bounded factor DP with independent exhaustive enumeration:
signed/complex literal weights, conditional i and -1 phases, ANF of AND terms,
free variables, zero literal weights, SAT-but-zero, UNSAT, and cap overflow.
A second independent oracle checks 100 deterministic quadratic cases with
affine constraints. A cross-route test compares the sign-phase calculation
to bounded exact factor DP.

Commands:
    cargo test --locked --manifest-path benchmarks/rust_core/Cargo.toml --features pathsum-factor-research pathsum_factor_research
    cargo clippy --locked --manifest-path benchmarks/rust_core/Cargo.toml --features pathsum-factor-research --lib -- -D warnings

GitHub workflow: .github/workflows/pathsum-factor-research.yml
Linux, macOS and Windows CI must pass on final PR HEAD before TEST-qualified.

**Still not completed**: proof of arbitrary-angle source-exactness (versus finite-source-MPFR observation), production-optimized QuPRS E2E integration, expanded long-case holdouts, cross-runner comparisons, rank-width decomposition, ADD, and the production backend router. A separate research-only 100/333-bit MPFR native factor engine and same-RR source-pinned paired experiment have passed; the exact-Gaussian bridge remains as an independent small oracle. No QuPRS solver default or RaWMoCS core changes.

Next step: expand source-pinned post-RR holdouts beyond the five already measured circuit families and introduce a cost-bounded routing experiment only for eligible low-width residuals. The source emitter, planner, evaluation and fallback costs belong in WallTime; preserve peak RSS and UNKNOWN outcomes.

## First real original-QuPRS structural evidence (source-locked)

Pinned QuPRS: https://github.com/PhysicsQoo/QuPRS/tree/0b28736651b3740ed3b201edd2efdef3da541e0d

Research CI: https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43787896672

The new source probe copies a temporary example into an isolated
immutable QuPRS checkout, then parses original Feynman/MQTBench QASM
without transpilation or editing. It compares the same source under
auto-reduction OFF (original WMC research baseline) and ON (fully
reduced PathSum). Both are *research profiles*, not equivalent stage
timing to an external solver.

| Original QASM | Path variables raw -> RR | RR phase terms | RR phase degree | RR primal min-degree width upper |
| --- | ---: | ---: | ---: | ---: |
| Feynman HWB6 | 67 -> 7 | 0 | 0 | 6 |
| Feynman QFT4 | 51 -> 35 | 78 | 4 | 5 |
| MQT QNN4 | 52 -> 23 | 82 | 5 | 7 |
| MQT Grover4 | 50 -> 11 | 36 | 6 | 7 |
| MQT VQE4 | 52 -> 18 | 90 | 5 | 6 |

All **10/10** raw/RR source profiles completed. RR output
constraints MUST be included: for example HWB6 has 346 XOR-linked
output ANF terms of maximum degree 6, despite zero remaining phase
terms. Thus quadratic-sign Gauss sum is **ineligible** for these
five full residuals; a phase-only classifier would be wrong.
The width numbers are **heuristic graph elimination upper bounds**,
not exact treewidth or rank-width measurements.

## Exact-Gaussian source bridge and independent oracle

A second temporary QuPRS example exports only cases whose **entire
source phase** is exactly one of {1, i, -1, -i}, without radians or
symbolic coefficients, and whose post-RR original variable count fits
the explicit bound. Source data are serialized as native output-ANF
and phase factors; NO Tseitin auxiliaries are introduced. A separate
source-level exhaustive assignment oracle evaluates the exact value.

- Source exporter: benchmarks/rust_core/tools/quprs_pathsum_factor_export.rs
- Rust native DP consumer: benchmarks/rust_core/examples/pathsum_factor_source_cli.rs
- Latest source bridge CI: https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43788470394
- **Feynman HWB6 RR**: 7 original variables, 7 output constraints,
  no remaining phase terms, independent source witness **SAT=true,
  Full WMC=4+0i**. Native factor DP reproduced the exact pair.
  Under this CI run, research-kernel solve time observed **448,664 ns**,
  1,165 visited factor cells. This is a single native CI observation,
  **not** an end-to-end speedup or a GPMC same-CNF timing.
- QFT4, QNN4, Grover4 and VQE4 do not all fit the fourth-root-only
  source-exact gate; bridge records INELIGIBLE/CENSORED, not an invented
  WMC value or acceleration. These need a separate precision-aware
  generic Complex DP study.

Raw HWB6 GPMC's ~189ms in an earlier same-precision study is **not a
valid speedup denominator** for the ~0.45ms post-RR factor DP kernel:
the input representation, preprocessing, encoder and process boundary
are different. A fair next experiment must include parse + RR + factor
planner + native execution or compare both solvers on the **same post-RR
residual** with identical boundary definitions. Time and RSS gates
must be collected for every arm before algorithm admission.

### Follow-up gate: whole-pipeline costs

A seven-trial, same-RR, 100/333-bit native-process experiment has now passed. Results are reported in the following section. The experimental source exporter creates BOTH representations, so its shared preparation cost is deliberately charged to either arm; this is an auditable, conservative two-process comparison, not optimized production QuPRS E2E.



## Seven-trial matched-source MPFR algorithm experiment (2026-11-16)

Source-pinned [full trial CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43792555247);
raw artifact: "rr-pathsum-mpfr-native-factor-vs-gpmc" (JSON and all 10
source-emitted CNF/factor pairs). Original source revisions are fixed at
QuPRS 0b28736651b3740ed3b201edd2efdef3da541e0d and
GPMC 89bbc89d0de918e7a5385740355df55765039230.

**Scientific admission**: all 10 of 10 case/precision rows returned
MATCHED_SOURCE_MPFR_OBSERVATION. Each row uses **seven alternating
fresh independent native processes per tool** on the same Linux CI runner.
Both consume the **same RR PathSum** and **same QuPRS source-printed
MPFR complex phase weights**. GPMC reads Tseitin CNF; the new Rust
algorithm reads native output ANF and phase factors. Both use
30 decimal -> 100 MPFR bits and 100 decimal -> 333 MPFR bits.
SAT status and BOTH complex WMC components agreed under the
predeclared finite-precision observation guard. HWB6 also matches
the independently exhaustive original-RR SAT=true and WMC=4+0i.
The MQT Grover4 RR case is **UNSAT** for both algorithms, not
SAT-but-zero and not a proof of generic source-angle soundness.

**Solver-only native full subprocess wall** includes process startup,
input parse, algorithmic planning, count, output and process cleanup.
Shared QuPRS source export is **separately timed**; the table's
"source+solver" totals are paired sums across corresponding trials
from this two-process research pipeline.

| Original source | MPFR bits | GPMC solver p50 ms | Factor DP solver p50 ms | solver ratio GPMC/DP | Source+GPMC p50 ms | Source+DP p50 ms | combined ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Feynman HWB6 | 100 | 137.229 | 3.190 | 43.01x | 144.000 | 9.564 | 15.06x |
| Feynman HWB6 | 333 | 138.721 | 3.228 | 42.97x | 144.920 | 9.064 | 15.99x |
| Feynman QFT4 | 100 | 11.478 | 3.022 | 3.80x | 15.360 | 7.078 | 2.17x |
| Feynman QFT4 | 333 | 11.694 | 3.164 | 3.70x | 15.778 | 7.339 | 2.15x |
| MQT QNN4 | 100 | 16.838 | 4.513 | 3.73x | 21.109 | 8.823 | 2.39x |
| MQT QNN4 | 333 | 17.629 | 5.552 | 3.18x | 22.389 | 10.096 | 2.22x |
| MQT Grover4 (UNSAT) | 100 | 3.464 | 2.953 | 1.17x | 7.218 | 6.711 | 1.08x |
| MQT Grover4 (UNSAT) | 333 | 3.419 | 3.096 | 1.10x | 7.180 | 6.818 | 1.05x |
| MQT VQE4 | 100 | 14.078 | 3.628 | 3.88x | 18.348 | 7.772 | 2.36x |
| MQT VQE4 | 333 | 14.131 | 3.874 | 3.65x | 18.326 | 8.166 | 2.24x |

Peak RSS observed through Linux GNU time: for example HWB6
100-bit GPMC 21,516 KiB versus Native DP 3,364 KiB (p50);
QFT4 100-bit 14,060 KiB versus 3,196 KiB. RSS is observational
and the experiment uses a 2-GiB **virtual-address** prlimit, not
a cgroup hard RSS/memory limit.

Interpretation:
- The observed HWB6 factor benefit is **structural**: RR leaves
  7 original variables and zero phase terms but nonlinear output
  ANF up to degree 6; a generic Tseitin expansion has substantial
  preprocessing/solver overhead. This does not establish a 43x
  speedup over the *original un-reduced* GPMC workload, nor 43x
  QuPRS production E2E.
- QFT4, QNN4 and VQE4 consistently show 2.1–2.4x benefit in the
  conservative two-process comparison at both numeric widths.
  Grover4 is a valuable near-parity negative control.
- The common exporter deliberately builds both representations,
  creating an identical and **non-optimized** preparation charge
  for each algorithm. Hardware, source SHAs and input equivalence
  are controlled, but this remains a limited five-family,
  one-runner seven-trial cohort with no large-case independent
  Full Complex WMC proof beyond the exact HWB6 witness.
- No GPMC code, CDCL implementation or heuristic has been copied.
  The alternative is direct QuPRS-native bounded sum-product
  elimination; GPMC and Rust search the same mathematical residual
  through different representations.

Next admission step: source-locked independent Feynman hard holdouts
(Grover5, Barenco and ModMult when supported), paired full QuPRS
RR/encoder/solver process wall, better planner overhead attribution,
a fixed resource policy, and production fallback negative controls.
Keep the research feature disabled by default until these gates pass.


## Literature

- Compilation and Fast Model Counting beyond CNF (IJCAI 2024):
  https://doi.org/10.24963/ijcai.2024/367
- DPMC: Weighted Model Counting by Dynamic Programming on Project-Join Trees:
  https://arxiv.org/abs/2008.08748
- ADDMC: https://doi.org/10.1609/AAAI.V34I02.5505
- Algebraic Model Counting: https://doi.org/10.1016/j.jal.2016.11.031
- Quadratic Sums-of-Powers (2026 preprint):
  https://arxiv.org/abs/2605.29944
- Unifying and Extending Strong Simulation (2026 preprint):
  https://arxiv.org/abs/2610.00547

SRC = pinned source observation; TEST = CI-proven small scientific validation;
HYP = anticipated general/production benefit, NOT proved. EXP = matched source-scoped single-runner measurement; no cross-hardware or independent exact proof for large generic-angle cases.
