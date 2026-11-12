# RaWMoCS

Rust-based Algebraic Weighted Model Counting Solver.

RaWMoCS is an experimental Rust library for **full complex WMC** over exact
Gaussian rational weights. The initial reference engine includes original-CNF
unit propagation, chronological weighted DPLL, component decomposition, and a
canonical residual cache.

The numerical count and Boolean satisfiability are separate; a satisfiable
formula can have zero weighted count. All variables have independently defined
positive and negative literal weights. A node-budget failure returns an error,
never a partial result.

## Development

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets
    cargo test --workspace

The tests include an independent exhaustive assignment oracle for small CNFs.

**Research status:** not a production solver. Projected WMC, high-precision
transcendental angle weights, certified intervals, process isolation, CLI,
QuPRS adapter integration, and validated performance improvements are outside
this initial scope.

## License

MIT. See LICENSE.
