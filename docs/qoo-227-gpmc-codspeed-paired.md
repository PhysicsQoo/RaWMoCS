# QOO-227 — Paired QuPRS test-defined complex WMC, GPMC C++ vs RaWMoCS Rust

Status: **opt-in source-locked benchmark research**. No GPMC algorithm,
QuPRS SolverAdapter/default, RaWMoCS production library, or existing CodSpeed
benchmark name is changed by this branch.

## Shared input proof

- Original source: QuPRS `main@0b28736651b3740ed3b201edd2efdef3da541e0d`,
  original `test/test_wmc_direct_cnf_golden.py` Git blob
  `d5d9b8ab6e513afd332e34c642a8017b7c415caa`.
- **Single generator across languages:** GPMC
  `benchmarks/quprs/prepare_direct_cnf.py` (blob
  `0a6859c583f34725e59983380cedf1030d7da15b`) from the source-pinned
  [GPMC PR #22](https://github.com/PhysicsQoo/GPMC/pull/22).
  Both CI jobs use the same script and QuPRS checkout to produce *identical
  GPMC-format raw CNF bytes* and a per-file SHA-256 manifest.
- Six originals: unconstrained, forced SAT, Boolean contradictory UNSAT,
  disjunction, XOR-like relation, and **SAT with exact complex cancellation**.
  Original printed decimal weights are binary64 exact for these six cases.
- Source exact SAT/Full WMC is checked against an independent bounded
  truth-table Gaussian-rational oracle; the Rust Divan benchmark also checks
  the default solver on every iteration.

**These are test-defined direct weighted CNFs, not actual QuPRS
PathSum/WmcManager-emitted circuits.** QOO-226 owns that later gate.

## Pairing of timing boundaries

| Solver | In-process timed work | Numeric semantics |
| --- | --- | --- |
| GPMC C++ Google Benchmark | new `Counter<Complex>`, raw weighted-DIMACS parse, default preprocessing, extra scores (if needed), model counting (if needed) | MPFR 30 digits |
| RaWMoCS Rust Divan (this PR) | same raw weighted-DIMACS parse into `Problem` then new default `solve_with_limits` | exact Gaussian rationals of finite printed decimal tokens |

Both exclude original QASM generation, fixture source check, filesystem read,
tool startup and independent mathematical oracle. Neither is an exclusively
counting-kernel-only measurement; trivial CNFs can be solved during GPMC
preprocessing. The two integrations share **input and broad work boundaries**,
but do **not** share identical algorithms, numerical precision semantics or
setup allocations. Do not interpret sub-millisecond CPU Simulation numbers
as QuPRS end-to-end speed or as equivalent correctness guarantees.

The normal RaWMoCS `solver` Divan bench remains unchanged. The separate
`quprs_direct_cnf` benchmark is guarded by Cargo feature
`quprs-gpmc-paired-research` and executes only in its dedicated
[workflow](../.github/workflows/quprs-direct-cnf-paired.yml).

The GPMC C++ workflow runs in the GPMC repository, and the Rust workflow runs
in RaWMoCS. Even with CodSpeed CPU Simulation on each, **cross-repository
absolute number comparison is an exploratory observation**, not an official
CodSpeed paired diff or one-run guarantee. Use same toolchain/instrument
version and controlled runner details; separately record native fresh-process
WallTime, VmHWM, input hash, successful/censored verdict and precision for
head-to-head. Censor UNKNOWN/timeouts: never turn them into numeric speedups.

## Reproduce

Generate the six CNFs using the pinned upstream original QuPRS repository
and [GPMC generator](https://github.com/PhysicsQoo/GPMC/blob/research/quprs-codspeed-cpp/benchmarks/quprs/prepare_direct_cnf.py).
Then:

```sh
export GPMC_QU_PRS_CNF_DIR=/path/to/generated/cnf
cargo fmt --manifest-path benchmarks/rust_core/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --all-targets --features quprs-gpmc-paired-research -- -D warnings
cargo bench --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --features quprs-gpmc-paired-research --bench quprs_direct_cnf
```

The CodSpeed simulation job is additionally gated by the existing
`RAWMOCS_CODSPEED_ENABLED=true` variable. GPMC requires connecting its
repository to CodSpeed and setting `GPMC_CODSPEED_ENABLED=true`.
Until both actual CodSpeed **measurement runs** complete successfully,
`CI smoke success` is only a correctness/compilation observation.
Do not claim either solver is faster without published paired measurements.

## Verified final-head Rust CodSpeed integration

- [CodSpeed run 6af74c57fb3193ed60289436](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6af74c57fb3193ed60289436),
  source `2e3098f1baa6f13e7dc3e6ce67b8dd5aaf342a92`,
  is **completed, 68 benchmarks**: original 62 plus six new
  source-pinned QuPRS direct-CNF identities. All are Simulation, not
  native WallTime; new values range 83.3–160 µs **inside Rust**, not
  comparisons with GPMC.
- RaWMoCS CodSpeed's first attempt erroneously uploaded 62 legacy and
  six new benchmarks via different workflows on the same commit,
  which CodSpeed flagged as unsupported. The **single**
  `.github/workflows/codspeed.yml` now instruments and uploads both
  targets with `cargo codspeed build -m simulation --locked
  --features quprs-gpmc-paired-research --bench solver --bench
  quprs_direct_cnf`, then one `cargo codspeed run --bench solver
  --bench quprs_direct_cnf`. The extra standalone Rust workflow runs
  **correctness-only**; no second CodSpeed run.
- [Same original-source Rust CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43474647343)
  proves source input SHA and independent Full Complex WMC oracle. The
  final merged PR decision must still confirm current-head CI.
- GPMC's C++ in-process repeated counting adapter **is NOT admitted**:
  four of six test-defined cases produced denormal garbage in measured
  iterations. The GPMC research branch therefore uses the native
  fresh-process CLI `codspeed.yml` exec benchmark as its better
  actual integration comparison path. Until GPMC repo itself is connected
  to CodSpeed and the correct native executor is measured, **no
  cross-solver CPU speedup statement can be made**.
- The CLI `exec` profile and Rust `Divan` profile do **not** have an
  identical timed boundary. A fair native-CLI-vs-native-CLI E2E
  WallTime/VmHWM bridge, under equivalent precision/verdict constraints,
  remains necessary (as does QOO-226 actual QuPRS encoder output).

## Same-runner native CLI validation and accepted GPMC source

The source-locked experimental GPMC [native CLI PR #23](https://github.com/PhysicsQoo/GPMC/pull/23)
was **squash merged** to `PhysicsQoo/GPMC master@e3faa9c022bf93bc8ccebef7376e69d96367dd8e`
after its six original QuPRS direct-CNF cases passed 18 fresh-process
GPMC CLI exact source/MPFR30 SAT/count checks. The Rust research
workflows now pin **this merged GPMC main SHA**, not the earlier Draft
in-process Google Benchmark commit. The original shared generator
Git blob remains `0a6859c583f34725e59983380cedf1030d7da15b`.

A new [source-pinned same-Linux-runner native CLI paired workflow](../.github/workflows/quprs-direct-cnf-paired.yml)
checks out that GPMC source, builds its unchanged C++ GPMC
`gpmc-bin` plus independent Rust `quprs_native_cli`, reuses the **same
raw six CNF bytes**, and runs each with an identical GNU
`/usr/bin/time` per-child wrapper. Each case has three order-rotated
paired trials, i.e. **36 fresh native subprocess samples**. The
independent expected SAT and complex counts are required on every
sample (GPMC MPFR30 observation versus exact Rust rational printed
weight count); process elapsed and peak RSS are recorded and all
sample records archived.

[First passing paired source SHA a5bc8722 / CI #43476624430](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43476624430)
produced 36/36 valid samples. Observed median WallTime in **milliseconds**
(the single GH-hosted runner, three samples per solver per case):

| Test-defined 2-variable CNF | GPMC native CLI | RaWMoCS native CLI |
| --- | ---: | ---: |
| unconstrained | 2.000 | 1.271 |
| forced-sat | 1.849 | 1.214 |
| contradictory-unsat | 1.712 | 1.199 |
| disjunction | 5.782 | 1.295 |
| xor | 1.848 | 1.255 |
| complex-cancellation | 1.805 | 1.190 |

These are **not** QuPRS circuit miter encoder instances. Timings
include executable startup and file I/O, often dominating two-variable
search. They are a micro-cohort process-integrity and pipeline
smoke, **NOT evidence of solver dominance in actual quantum circuits**
or a production default-change gate. GPMC's in-process repeated-count
research is still invalid and separate [Draft PR #22](https://github.com/PhysicsQoo/GPMC/pull/22).
