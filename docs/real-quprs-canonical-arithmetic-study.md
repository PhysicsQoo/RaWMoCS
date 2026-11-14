# Original QuPRS weighted-CNF: exact arithmetic optimization A/B

**Status:** Research-only, exact rational finite-printed-decimal Full WMC.
Default `rawmocs-core`, QuPRS SolverAdapter, and pinned GPMC C++ remain
unchanged. This is a source-pinned measurement, not a claim of quantum
equivalence or a common numerical-precision contract.

## Inputs and measurement

The [original QuPRS Rust `PathSum / WmcManager` exporter](https://github.com/PhysicsQoo/QuPRS/blob/0b28736651b3740ed3b201edd2efdef3da541e0d/rust/quprs/examples/interval_cnf_reference.rs)
is pinned to `0b28736651b3740ed3b201edd2efdef3da541e0d`.
Both solver families receive the exact same GPMC-dialect weighted-CNF
bytes, and each source QASM, CNF, and source phase sidecar is hashed and
recorded. The unchanged native GPMC reference is MPFR30; **RaWMoCS
counts the finite printed decimal tokens as exact rational numbers**.

The matched Rust A/B/C cohort builds these arms on one Linux runner:

1. Baseline: unchanged `rawmocs-core` at
   `45d71e9f5e302a53732b6f678898404c9c8a20e9`.
2. Candidate: the current default exact arithmetic (no production-core
   modifications in this research PR).
3. Canonical: current source with the existing opt-in
   `canonical-mul-research` feature, calling
   `solve_with_canonical_mul_research`.

The canonical arm clones and validates every external rational weight and
performs exact denominator normalization **inside the timed solve span**.
It then uses cross-cancelled multiplication with `BigRational::new_raw`,
which is safe for the canonical private inputs. No floating-point
conversion or precision downgrade occurs. This feature is disabled
for normal production builds.

Each case runs three independent native-process executions of each arm,
rotating arm order, with an 8-second timeout and 2-GiB virtual-address
space bound. Source hashes, exact SAT / complex WMC, stdout SHA256,
WallTime (process launch, parse, solve and serialization), peak RSS,
and candidate internal phase timings are archived as JSON. **Exact
rational values and the complete native stdout must match byte for byte
across all nine runs**, or that case is marked non-admitted.

## Complete eleven-case same-runner cohort

[Eleven-case Actions qualification](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43583103276)
on 2026-11-14: **11/11 `EXACT_MATCH`** across three alternating
native-process trials for each of the three Rust arms. GPMC was also
built and run from its pinned unchanged C++ source in the same CI job,
separately using its MPFR30 contract.

| Original QuPRS CNF | GPMC MPFR30 wall ms | Rust default exact wall ms | Rust canonical exact wall ms | Canonical speedup vs pinned Rust baseline | Baseline→canonical Rust peak RSS KiB | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| GHZ2 | 4.832 | 3.157 | 4.057 | 1.005× | 2980→2972 | exhaustive oracle |
| GHZ3 | 4.789 | 4.034 | 4.025 | 0.988× | 3028→2968 | exhaustive oracle |
| GHZ4 | 4.823 | 3.981 | 3.984 | 0.992× | 2848→2948 | exhaustive oracle |
| Graph3 | 10.740 | 4.286 | 4.165 | 1.014× | 2972→2952 | exhaustive oracle |
| BV3 | 9.841 | 4.514 | 4.426 | 1.004× | 2936→2928 | exhaustive oracle |
| QFT2 | 9.916 | 4.095 | 3.987 | 1.001× | 3040→2972 | exhaustive oracle |
| QFT3 | 10.536 | 4.435 | 4.278 | 1.034× | 2988→2948 | exhaustive oracle |
| Grover2 | 11.080 | 7.020 | 6.301 | 1.134× | 3000→3104 | cross-solver only |
| QAOA3 | 9.484 | 8.426 | 7.275 | 1.139× | 3096→3084 | cross-solver only |
| Original Feynman mod5_4 | 11.349 | 34.325 | 25.826 | **1.345×** | 3656→3764 | cross-solver only |
| Original Feynman qft_4 | 16.822 | 2609.074 | 1621.448 | **1.612×** | 18324→18484 | cross-solver only |

**Different comparisons are intentionally separated:** GPMC MPFR30 versus
exact Rust is **not precision aligned**; the GPMC column supplies only
context on unchanged native-process WallTime. The canonical speedup
column uses a distinct same-source **exact-Rust pinned baseline** built
in the same job. The default Rust candidate's process median is also
shown, but on tiny cases OS startup jitter dominates and no reliable
small-case improvements can be inferred. The canonical path remains
~96× slower than GPMC on the unchanged Feynman QFT4 in this run,
despite its exact-Rust A/B improvement.

For Feynman qft_4, the unmodified default exact-Rust internal solve
median is **2597.475 ms**, versus **1610.093 ms** for canonical.
CNF parse is ~0.4 ms; stdout serialization is ~4.3 ms. The source
count uses 154 variables and 296 clauses. This establishes where
process time was spent in these trials, not a standalone GCD
instruction-cycle attribution.

The [earlier independent four-case CI run](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43582506332)
observed canonical improvements of **1.307×** (mod5_4) and **1.628×**
(qft_4) with exact output parity. Repeating the larger cohorts on the
new runner sample yielded **1.345× / 1.612×**. The two results support
a promising workload-specific trend, not a general proof of performance
or statistical confidence from three trials.

An earlier same-runner two-arm experiment adding sparse Gaussian
zero-component fastpaths gave essentially **no speedup** on source-pinned
Feynman cases. Those core optimizations were reverted; no production
core fastpath is retained by the PR.

## Extended cohort and scientific gates

The qualified CI covers **all eleven source-emitted circuits**:
GHZ2–4, graph3, BV3, QFT2–3, Grover2, QAOA3, plus original Feynman
mod5_4 and qft_4. Each candidate must match exact output and preserve
resource/admission status. The source-capture workflow archives every
trial, including regressions.

The seven small source-emitted CNFs at 17 variables or fewer additionally
have an **independent exhaustive printed-rational Full WMC oracle**.
The four larger 23–154 variable circuits are checked against cross-solver
agreement, not independent exhaustive certification. A finite printed
decimal oracle does not prove source-quantum phase precision.

Neither an isolated 1.63× same-runner result nor the small `n=3`
sample establishes a statistically general speedup. It does not bridge
GPMC MPFR30 and exact rational precision. The reported `WallTime`
and native RSS must not be presented as CodSpeed CPU Simulation.
Do not promote the canonical feature to default without independent
case diversity, repeated controlled native measurement, negative input
tests, large-number resource limits, and cross-platform parity.

## Reproduction

Use [the original encoder workflow](../.github/workflows/quprs-encoder-source-capture.yml)
via GitHub Actions. It checks out the pinned QuPRS/GPMC and Rust baseline,
builds the default and feature-gated native CLI, records the source SHA,
and writes `quprs-representative-native.json` and
`quprs-optimizer-ab.json` to the
`quprs-emitted-cnf-native-parity` Actions artifact.

Public source and findings intentionally contain no private work-tracker
identifiers.
