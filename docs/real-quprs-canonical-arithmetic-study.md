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

## First qualified four-case A/B/C observation

[Source-pinned CI run](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43582506332)
on 2026-11-14, `EXACT_MATCH` for all four cases:

| QuPRS emitted CNF | Baseline wall ms | Default candidate wall ms | Canonical wall ms | Canonical speedup vs baseline | Baseline / canonical peak RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| QFT3 (12 variables) | 3.927 | 3.953 | 3.882 | 1.012× | 2908 / 2956 |
| Grover2 (23 variables) | 5.579 | 5.600 | 5.149 | 1.084× | 3068 / 3100 |
| Original Feynman mod5_4 (43 variables) | 21.647 | 21.658 | 16.559 | **1.307×** | 3696 / 3792 |
| Original Feynman qft_4 (154 variables) | 1306.839 | 1286.910 | 802.786 | **1.628×** | 18344 / 18420 |

For qft_4, internal solve median falls from 1279.100 ms to
794.719 ms. Output serialization (approximately 1.8 ms) and CNF parse
(approximately 0.3 ms) are relatively minor costs. Larger exact
numerator/denominator growth is a plausible contributing factor, not a
measured proof that GCD reduction dominates all search work.

An earlier same-runner two-arm experiment adding sparse Gaussian
zero-component fastpaths gave essentially **no speedup** on the
source-pinned Feynman cases. Those core optimizations were reverted; no
production core fastpath is retained by this PR.

## Extended cohort and scientific gates

The maintained CI expands the same paired exact arithmetic study to
**all eleven source-emitted circuits**: GHZ2–4, graph3, BV3, QFT2–3,
Grover2, QAOA3, plus original Feynman mod5_4 and qft_4. Each candidate
must match exact output and preserve resource/admission status. The
source-capture workflow archives every trial (including regressions).

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
