# RaWMoCS

Rust-based Algebraic Weighted Model Counting Solver.

RaWMoCS is an experimental Rust library for **full complex WMC** over exact
Gaussian rational weights. The initial reference engine includes original-CNF
unit propagation, chronological weighted DPLL, component decomposition, and a
canonical residual cache.

The numerical count and Boolean satisfiability are separate; a satisfiable
formula can have zero weighted count. All variables have independently defined
positive and negative literal weights.

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

This is a research prototype, **not** a production solver or a quantum-circuit
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

## License

MIT. See LICENSE.
