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



## Beyond the original five: source-exact holdouts and width-admission limits

Source-pinned [Gaussian scientific oracle and structural CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43794305755)
and [seven-trial paired 16-cohort MPFR comparison](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43794305712)
qualify three additional original Feynman families. All three
post-RR residuals admit source-level exhaustive enumeration independent
of the exact Gaussian Rational Project-Join research code.

| Original source | RR original vars | RR phase terms | SAT | Exact source WMC | 100-bit common-prep+GPMC / native DP p50 (ms) |
| --- | ---: | ---: | --- | --- | --- |
| Feynman Grover5 | 15 | 6 sign phases | TRUE | 0+0i | 5.455 / 4.807 |
| Feynman Barenco5 | 9 | 0 | TRUE | 496+0i | 7.388 / 3.705 |
| Feynman ModMult55 | 9 | 0 | TRUE | 32+0i | 8.574 / 3.963 |

Grover5 is **SAT-but-zero by independent exact enumeration**, not
UNSAT. Keeping SAT support separate from the complex count is mandatory.
Each row also matched at 333 MPFR bits. These are small **after RR**,
so they are not evidence of generic high-cost solver scalability.

A second [19-24q source-only structural scout](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43794760879)
returned all six original raw/RR profiles successfully under a
30-second/2GiB-virtual-address source cap:

| Original source | qubits | RR original vars | RR phase terms | max native output-factor scope | primal min-degree width upper |
| --- | ---: | ---: | ---: | ---: | ---: |
| Barenco10 | 19 | 19 | 0 | 11 | 10 |
| Toffoli10 | 19 | 19 | 0 | 19 | 18 |
| Adder8 | 24 | 24 | 0 | 24 | 23 |

The scope value is an observable *initial factor materialization*
limitation for the research dense-table solver (max scope 16).
The graph width estimate is only an upper bound, NOT a certified
minimum width. Barenco10 was admitted into [seven-trial native
GPMC/Factor DP comparison](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43795267900):
all 18/18 precision/case cohorts numerically matched. Source-level
independent [exact oracle](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43795268071)
proves Barenco10 SAT=true, Full WMC=523776+0i (19 variables).
At 100-bit: full native two-process common-prep+GPMC p50 7.722ms
vs Native Factor DP 5.912ms (~1.31x). At 333-bit: 7.854ms
vs 6.376ms (~1.23x). Still one Linux CI runner, seven trials/tool,
and an experimental dual-format common-source-prep boundary.

## Source-structural algorithm PoCs beyond dense factor width

### Bounded monotone trace inclusion-exclusion — restricted

After cancelling the same output variable on both sides of a trace
equality over GF(2), one *single* surviving positive AND monomial
M_i(x) required to be false is a forbidden conjunction. For a system
of such independent **conjunction constraints** (not XORed together),
the exact, unweighted model count is

    SUM_{S subset [k]} (-1)^|S| * 2^(n - |union_{j in S} vars(M_j)|).

Research implementation:
PathsumFactors::count_monotone_trace_bounded(). Admission requires
phase-free, parity-free, unit-weight original variables, <=50 variables
and <=20 distinct forbidden conjunction constraints. This is exact
Gaussian integer computation with separate SAT support, not a CDCL
port, and may fail closed on nonmonotone trace equations.

**Negative result matters**: the original Toffoli10 source is NOT a
system of independently forbidden AND constraints. It has **one
nontrivial output equality containing nine XOR-connected AND
monomials**. Misinterpreting these as nine separate requirements
would give the wrong count; the monotone route correctly refuses it.

### Bounded single-ANF Walsh counting — admitted on Toffoli10

Research implementation:
PathsumFactors::count_single_anf_trace_bounded(). Its admission is
strictly: phase-free, parity-free, all literal weights unit,
<=50 variables, <=20 AND monomials, and at most one nontrivial
output equation after GF(2) self-cancellation. Other outputs must be
tautological. The global Gaussian scalar is retained.

For a Boolean polynomial g(x) = c XOR m_1(x) XOR ... XOR m_r(x),
the parity-even trace indicator has the exact representation

    [g(x)=0] = [1 + (-1)^c * PRODUCT_j(1 - 2*m_j(x))] / 2
    N = {2^n + (-1)^c *
         SUM_{S subset [r]}(-2)^|S| * 2^(n-|union_{j in S} vars(m_j)|)} / 2.

This uses **2^r subset terms** rather than a dense table over all
variables touched by the original output equality. If multiple
nontrivial equations, arbitrary phases, nonunit literal weights or
an excessive term count are present, it returns an explicit rejection.

The pinned original [Toffoli10 source and independent exact oracle](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43797131554)
produce 19 original variables, zero phases, one output ANF with
nine nontrivial AND monomials. The old dense Factor DP would reject
initial factor scope=19; **ANF Walsh exactly matches exhaustive
2^19 original source assignments: SAT=true, Full Complex WMC=393216+0i**,
with 512 subset terms. Observed **single kernel** solve time at that
CI run: 33,861ns (about 34 microseconds). This is NOT a source-to-result
WallTime, not a matched-precision or process-matched GPMC comparison,
and does not imply 34us production QuPRS verification.

The [final scientific source and cross-platform validation CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43797229560)
passed Linux, macOS, Windows research science / Rustfmt / Clippy
and original source oracle at the code HEAD before this document
update. Production Rust core, QuPRS Adapter and GPMC stay unchanged.

**Adder8 remains unsupported** by both bounded dense Factor DP
(scope24 >16) and single-ANF Walsh (14,541 linked output XOR terms
and 14,482 nontrivial AND monomials). It is a strong negative
scaling/control case for later shared ANF DAG / ADD / source-aware
compilation work. Do not infer a result or quietly increase
dense-table limits.

## Matched original Toffoli10: independent ANF Walsh versus GPMC (2026-11-16)

The original pinned QuPRS/Feynman Toffoli10 QASM exceeds dense native
factor width admission (scope19 >16). An independent exact source-level
enumeration previously verified **SAT=true; Full Complex WMC=393216+0i**,
with 19 original Boolean variables and one nontrivial nine-monomial
output ANF trace constraint.

An independent research-only MPFR CLI option, --anf-walsh, now accepts
only phase-free input, at most one nontrivial output ANF after GF(2)
self cancellation, <=20 nontrivial monomials and <=50 variables. It uses
512 exact Walsh subset terms, then converts the integer to 100/333-bit
MPFR printed output. The ordinary bounded dense Factor DP remains
unchanged; this is not an automatic production router.

[Seven-trial same-source MPFR paired CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43798429810)
**SUCCESS**. All 20/20 original-source circuit/precision cohorts matched
SAT and both complex WMC components, including Toffoli10 at 100/333 bits.
Each original QuPRS RR residual is source-pinned and GPMC is original C++.
Both arms run seven fresh child processes in alternating order, using the
same QuPRS printed MPFR weights but different factor/CNF representations.

| Toffoli10 | GPMC solver p50 ms | ANF Walsh p50 ms | Solver ratio | Shared source+GPMC p50 ms | Shared source+Walsh p50 ms | Combined ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 bits | 6.808 | 1.694 | 4.02x | 8.990 | 3.869 | 2.32x |
| 333 bits | 7.055 | 1.720 | 4.10x | 9.251 | 3.916 | 2.36x |

Observed peak RSS KiB (p50): 100-bit GPMC 13,812 vs Walsh 3,200;
333-bit GPMC 13,968 vs Walsh 3,144. RSS is observed GNU time data
and the study only limits *virtual address space* to 2 GiB. The
controlled source exporter builds **both** representations each trial:
shared-preparation+solver is conservative research two-process wall,
not optimized QuPRS production E2E or same-CNF-byte solver timing.

**Negative results**: original Adder8 has 24 source variables, dense
factor scope24 and over 14,000 nontrivial ANF monomials, exceeding
both the width16 dense and Walsh term20 science gates. Phase-bearing,
multiple-equation or nonunit-weight inputs are likewise not accepted
by this restricted integer-counting Walsh method. They must stay
explicitly ineligible or use separately verified complex DP/GPMC paths.

Next research gate: quantify Adder8 shared ANF subexpressions and
factorization before investing in bounded source-aware DAG, ADD/BDD or
rank-width compilation. Avoid increasing the dense table limit blindly.

## Original Adder8: source-pinned canonical GF(2) ANF DAG structural evidence (2026-11-16)

An independent, **research-only structural profiler** now examines the
*original unchanged QASM* from QuPRS at
0b28736651b3740ed3b201edd2efdef3da541e0d. The temporary QuPRS
source example builds the original post-RR PathSum, then exports output
trace ANF equations using a stable source-variable map. A separate
RaWMoCS example compiles each equation under GF(2) into a canonical
hash-consed ZDD-like polynomial DAG:

    f(x) = low(x) XOR (x_v AND high(x))

with identical (variable, low, high) triples interned across all
outputs; duplicate monomials cancel modulo two and variables within
AND monomials are idempotent. This representation preserves each
Boolean *polynomial*, **not** the weighted model count of a conjunction
of the output equalities. The tool does not change or approximate
original gates, phase coefficients, QuPRS adapters, or GPMC.

Research programs:
- benchmarks/rust_core/tools/quprs_pathsum_anf_dag_export.rs
  (copied only into the isolated immutable QuPRS checkout in CI);
- benchmarks/rust_core/examples/anf_dag_profile.rs
  (research-only Cargo feature pathsum-factor-research);
- .github/workflows/quprs-anf-dag-research.yml
  (Linux/macOS/Windows science and strict lint, original QASM source
  profile, 30s / 2GiB virtual-AS per source experiment).

Source structural CI: https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43800553654

| Post-RR original source | variables | original ANF terms | canonical trace terms after GF(2) | unique DAG nodes | multiply referenced DAG nodes | cross-output shared nodes | intern reuse hits |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Feynman Adder8 | 24 | **14,565** | **14,541** | **206** | 36 | 8 | 14,371 |
| Feynman Toffoli10 | 19 | 28 | 9 | 54 | 0 | 0 | 0 |
| Feynman Barenco10 | 19 | 20 | 1 | 10 | 0 | 0 | 0 |
| Feynman HWB6 | 7 | 353 | 358 | 168 | 28 | 23 | 300 |

Adder8 has 24 output equations of which **nine are nontrivial**;
all tested four post-RR traces have zero remaining phase terms. The
Adder8 canonical polynomial support contains **77,740 variable
occurrences**. A deliberately defined naive non-shared branch upper
bound is 92,281 entries, compared with 206 hash-consed DAG nodes.
These quantities use *different representations*, not byte-identical
memory accounting, and they do NOT imply a 447x solve acceleration
or physical-RSS reduction. Interning 14,371 times reflects reused
triples during construction, not an experimentally measured dynamic
programming cache-hit rate for WMC.

The initial pinned-source single Linux run observed Adder8 compiler
time approximately **3.592 ms** and 256 deterministic original-ANF
versus DAG assignment comparisons approximately **27.116 ms**
(before the follow-up improvement sharing memoized evaluations across
outputs per assignment). These are source-specific one-runner
diagnostic costs, not controlled GPMC or QuPRS full pipeline numbers.

Small independent science tests exhaust all assignments for multiple
synthetic 7-variable multi-output ANF systems, including duplicate
cancellation, constant terms, zero outputs and cross-output subgraph
sharing. All first qualified Linux, macOS and Windows science / Clippy
jobs passed. The source Adder8 probe compares 256 deterministic
assignments with direct original source ANF evaluation, **not**
exhaustive 2^24 enumeration; a dedicated mathematically independent
source counting oracle remains to be built. Fail-closed input/work/
node budgets are 8 MiB, 56 variables, 200k source monomials,
4 million summed recursive monomial visits, 800k unique DAG nodes,
and 30 seconds / 2 GiB virtual-AS for CI source operations.

### Interpretation and admission gate

The key result is **very strong polynomial representation sharing**
for Adder8, while Toffoli10 and Barenco10 do not benefit from DAG
hash-consing in the same way. This strongly motivates a *bounded
joint trace-constraint decision-diagram counting experiment* rather
than lifting Dense Factor DP width16 to width24 or applying the
single-output <=20-monomial Walsh shortcut to 14k terms.

However a small GF(2) polynomial DAG does not imply that a BDD/ADD
for the **conjunction of nine nontrivial trace equalities** is also
small. That second representation may blow up. Required next experiment:
factor-aware Shannon cofactors or ADD/BDD over all output constraints,
explicit variable order and a strict node/time/RSS cap, preserving the
separate Boolean SAT bit and Full Complex WMC. Compare on the SAME
post-RR source and independently verify the 24-variable phase-free
result against a separately emitted exact GPMC CNF. Keep CENSORED
and fallback statuses as scientific evidence. Production routing
remains unchanged; this DAG profiler **does not solve Adder8 WMC**.

## Bounded multi-output ROBDD counting from source Adder8 (2026-11-16)

Status: Adder8 phase-free, unit-weight exact integer count has
now **matched independent original GPMC** on the same QuPRS post-RR
source. This is a restricted correctness result, not production or
general complex-phase performance admission. A third, entirely
independent, full 2^24 bit-parallel source enumeration is under test.

The previously validated 206-node ANF polynomial DAG is ZDD-like,
representing XOR sums of square-free monomials, *not* the satisfying
assignment BDD. The new opt-in "--joint-bdd" route converts it using
the exact Shannon cofactors:

    f = low XOR (x_v AND high)
    BDD(f) = mk(v, BDD(low), XOR(BDD(low), BDD(high)))

Each trace equality is f=0, so the solution ROBDD is the AND of the
negated output polynomials. The solver hash-conses each (var,lo,hi)
BDD node, drops redundant lo==hi nodes, memoizes bounded XOR/AND
Apply, and sums 0/1 cofactors over all declared variables,
including skipped/free variables.

This standalone method is limited to **phase-free, unit-weight**
Boolean WMC, in which the Full Complex count is the exact integer
plus zero imaginary part. It does **not** support arbitrary-angle
phase factors, complex source weights, MPFR rounding or a
production router. Resource admission is 56 source variables;
250,000 nonterminal BDD nodes; 3,000,000 Apply calls; 1,500,000
Apply-cache entries; 30-second source CI process timeout and 2-GiB
virtual address limit. All over-budget paths fail closed.

First source-pinned CI at original QuPRS revision
0b28736651b3740ed3b201edd2efdef3da541e0d:
https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43802083276

The pinned source executable built and returned these results. The
independent platform science jobs in that particular run failed
Rustfmt before testing and have not been marked qualified.
Canonical source formatting has since been applied.

| Original source | ANF DAG nodes | Joint BDD nodes | Apply calls | SAT | Exact integer WMC |
| --- | ---: | ---: | ---: | --- | --- |
| Adder8 | 206 | **624** | **2,238** | true | **40960+0i (independent GPMC matched)** |
| Toffoli10 | 54 | 152 | 268 | true | 393216+0i |
| Barenco10 | 10 | 20 | 32 | true | 523776+0i |
| HWB6 | 168 | 479 | 1480 | true | 4+0i |

Toffoli10, Barenco10 and HWB6 reproduce previously obtained independent
original-source exact WMC oracles. Adder8 at 40960+0i has **now
been independently confirmed by unchanged source-pinned GPMC C++**
running on QuPRS-emitted original weighted CNF. That is a separate
CNF algorithm and does not rely on the new ROBDD Apply/count engine.
The source-level full-domain bitset exhaustive oracle is a distinct
additional verification gate, not yet reported as qualified. Count/validation stage in one Linux source run took
approximately 0.407ms, excluding QASM parse, RR, source export,
ANF DAG compilation and process startup. Never report this as
a 0.4ms QuPRS E2E verified solver.

The **independent GPMC oracle source CI succeeded**:
https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43802727982

The same immutable original QuPRS post-RR PathSum emitted its
original phase-free GPMC CNF; the exporter also emitted ANF bytes
that matched the first source export byte-for-byte. Original unchanged
GPMC C++ (SHA 89bbc89d0de918e7a5385740355df55765039230) at 30
decimal digits under 90s and 2-GiB virtual-AS independently returned
SAT=true, **Full WMC 40960+0i**. The CI compared SAT and both exact
integer complex components against the bounded ROBDD: 
ADDER_GPMC_ROBDD_SOURCE_EXACT_MATCH True 40960 0.
All three platform science tests and native-source jobs were PASS.
The next proof gate is whole-domain original-ANF 64-way bit-parallel
enumeration (not the ROBDD implementation), then a properly controlled
paired native WallTime/RSS experiment.

## Full original Adder8 2^24 exhaustive source oracle — independent GF(2) Mobius

A third research-only verification route now computes the original
phase-free QuPRS trace count by **direct GF(2) subset/Mobius transform
of the source ANF coefficients**, without using polynomial DAG
hash-consing, ROBDD Apply, BDD model counting, or GPMC's CNF search.
It parses the independently serialized original post-RR ANF equation
monomials and builds the complete 2^n truth table in 64-bit packs.

Per original equation, it initializes an ANF coefficient bitset,
XORs the output variable and every original source monomial (duplicate
terms cancel), then applies 6 within-word and up to 18 across-word
GF(2) butterfly passes. The resulting bitset is exactly the truth
table of that source equation. The complement of all output equations
is intersected bitwise; the final POPCOUNT includes every original
free variable. This enumerates the **entire 2^24 = 16,777,216**
original Adder8 assignment universe through packed operations.

Source: benchmarks/rust_core/examples/anf_bitset_oracle.rs.
Feature-gated, standalone benchmark crate only, not production.
Independent scalar truth-table tests cover 1..9 variables with many
nonlinear outputs; there are explicit duplicate cancellation,
constant, free-variable, phase, dimension and work-limit tests.
Limits: <=24 original variables; <=200,000 raw monomials; <=8MB
source input; <=250 million packed word operations; 75-second
CI process timeout and 2-GiB virtual-address-space guard. The
initial per-monomial bitset expansion implementation hit its
conservative work budget on Adder8 and was **rejected**, not
misreported as success; switching to the mathematically equivalent
Moebius butterfly removed that cost without increasing memory scope.

The source-pinned [exact original-QASM exhaustive oracle run](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43804272292)
passed the original-source job. Adder8 complete universe count:
**SAT=true, Full Complex WMC=40960+0i**, using
**100,663,296 packed butterfly/intersection word operations**.
Single CI solve-stage wall observed **55,790,766 ns**; this
excludes the full QuPRS RR, source export, and fresh-process startup.
Three other original-source controls independently agreed:
Toffoli10=393216+0i, Barenco10=523776+0i, HWB6=4+0i, all SAT.

Caveat: the initial source CI used source code whose platform
science checks were blocked by formatting/strict Clippy; its pinned
original-source computations DID complete. The code has since
been canonical-formatted and its indexed-mask lint corrected.
The final SHA must still pass Linux/macOS/Windows science and
source jobs before final research admission.

Together with the independently compiled original GPMC C++ and
the 624-node ROBDD on the **same pinned RR residual**, three different
methods now agree on the Adder8 **exact integer**. This does not
prove general arbitrary-angle complex WMC or a source-to-result
end-to-end speedup. Do not compare the 0.4ms BDD counting stage
against a 90s GPMC budget or use a 206-node ANF polynomial DAG
as a claim of production speedup without measured full pipeline
and memory-fallback evidence.

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
