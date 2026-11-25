# RaWMoCS

Rust-based Algebraic Weighted Model Counting Solver.

The original RaWMoCS reference library implements **full complex WMC** over exact
Gaussian rational weights. The initial reference engine includes original-CNF
unit propagation, chronological weighted DPLL, component decomposition, and a
canonical residual cache.

The numerical count and Boolean satisfiability are separate; a satisfiable
formula can have zero weighted count. All variables have independently defined
positive and negative literal weights.

## First release candidate: QuPRS weighted CNF executable

The v0.1.0 candidate is an opt-in signed-complex full WMC executable
for weighted CNF emitted by QuPRS. It selects a factor-prejoin route from
the CNF structure, with general MPFR DPLL, component decomposition and
residual caching for ordinary admission failures. It never selects by circuit name or source
benchmark ID. The reference crate and its API remain unchanged.

Build on Linux x86-64 with Rust, GMP/MPFR build prerequisites and a resolved
dependency lock:

```sh
cargo build --locked --release --manifest-path crates/rawmocs-cli/Cargo.toml
crates/rawmocs-cli/target/release/rawmocs --digits 60 input.cnf
```

The release candidate lock must be committed before publication; the initial
CI may bootstrap it. The four supported digit selectors are 30, 60, 100, 130
(working precisions 100, 200, 333, 432 bits). These selectors are working
precision choices, not a certified number of correct decimal digits.
QuPRS should use guarded precision where required and handle execution
failure independently from SAT, UNSAT, and numeric zero.

The source format is strict, with a `p cnf N M` header, `c t wmc`,
optional `c p weight L REAL IMAG 0` records, then one terminated clause
per line. Omitted literal weights are independently one. The finite
decimal grammar and source size limits are enforced. Example:

```
p cnf 1 0
c t wmc
c p weight 1 -1 0 0
```

A complete solve emits exactly a Boolean status and a rounded complex count
(`c r mpfr REAL IMAG`); SAT with zero is valid. Nonzero exit means no
count and empty stdout. Stderr reports INVALID_INPUT (2), UNSUPPORTED_INPUT
(3), TIMEOUT (4), RESOURCE_LIMIT (5), explicit ROUTE_REJECTED (6), or
INTERNAL_ERROR (7). The supervisor must classify external timeout,
cancellation and OOM/kill from process evidence; partial stdout is invalid.
Auto routing uses one solver deadline (45 seconds by default). Set
`--timeout-ms N` (0 to 86400000; zero expires immediately) after `--digits`.
Planning uses at most two thirds of the remaining time and 30 seconds.
The reference diagnostic uses the external process supervisor instead. The general route has no 32-variable admission cutoff;
it retains the parser's input bounds and optional bounded cache. General
search uses heap-resident continuation frames rather than a fixed recursive
depth cap. Frame allocation failure is a resource failure, not a count; The exact-rational reference is an
explicit small-case diagnostic, not the auto fallback. An external process
resource supervisor is still required for hard RSS and whole-process time.
The heap frame guard is not a bound on all allocations.

**Candidate only:** see [release admission](docs/first-release-admission.md).
No claim of competition-wide performance, projected WMC, exact
transcendental phase weights, certified rounding or equivalence checking.

## Scientific baseline

The PR science job checks out the existing **source-pinned QuPRS A0 research
corpus** at commit `abda06877571a32f644451801dccbbb33e8802ac` rather
than copying fixture data into this repository.

- Original independent oracle: 48 complex fixtures (39 Full WMC; 9 projected).
- Native Rust differential tests: all 39 Full WMC fixtures.
- Original 620 mathematical properties include 120 projected checks. These run
  as reference-only tests, **not** native projected solver certification.
- Adversarial cache context: 20 original pairs / 40 states; 32 Full WMC states
  are independently checked against the Rust core.
- Original 65,536-state two-variable rewrite-safety audit and native Rust
  counting comparison for that same source-defined universe.
- Zero and negative weights, SAT-but-zero, rational precision, and bounded
  node/depth/cache behavior are explicitly tested.

## Development

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

The external A0 integration tests are ignored by default and **fail closed**
when invoked without their source-pinned fixture directory. CI checks out the
existing original files and then runs those tests with `QUPRS_A0_DIR`.

The exact reference engine remains a research prototype, **not** a production solver or a quantum-circuit
equivalence checker. The finite node budget and depth limit return an error
instead of a partial count; the cache uses an entry-count limit, not a memory
byte limit. OS-level hard RSS caps, wall-clock timeout/cancellation,
projected WMC, source-exact transcendental phase weights, certified rounding
and QuPRS adapter integration remain outside this scope.


## Native Rust performance benchmarks

The separate unpublished workspace in `benchmarks/rust_core/` defines six
stable Divan-compatible benchmark identities for exact rational arithmetic,
forced/free factors, connected search, disconnected components and an
identical cache-on/cache-off formula. The benchmark fixtures are generated
once, before measurement. Solver calls include internal allocations and search,
but **not** QuPRS parsing, external solver startup or end-to-end verification.

Validate the benchmark source without claiming a speedup:

```sh
cargo test --locked --manifest-path benchmarks/rust_core/Cargo.toml
cargo bench --locked --manifest-path benchmarks/rust_core/Cargo.toml --bench solver --no-run
```

Until the repository is enabled in CodSpeed, actual measurement uploads are
opt-in. After the CodSpeed GitHub connection is authorized, set the repository
Actions variable `RAWMOCS_CODSPEED_ENABLED` to `true` to allow uploads on
performance-bearing PRs or main pushes; `workflow_dispatch` is also available.
The no-upload benchmark smoke job still runs independently. CodSpeed output is
not a required PR check and is not a scientific correctness gate.

The initial CodSpeed Simulation numbers measure **only** native CPU work, not
wall-clock solver-inclusive or QuPRS end-to-end time. Compare only matched
benchmark IDs, compatible instrumentation, and controlled SHA/runner inputs.
Do not infer performance superiority over GPMC or Ganak from this suite.

## Scalability study

See [Exact Full WMC scalability research](docs/scalability-study.md) for
analytical medium-CNF contracts, controlled benchmark series, and resource
safety boundaries.

## License

MIT. See LICENSE.
