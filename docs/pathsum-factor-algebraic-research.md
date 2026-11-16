# Native Path-sum factor elimination and quadratic sign-phase research

Status: OPT-IN ALGORITHM PROTOTYPE — not production, not performance-qualified.

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
  to repeated generic CNF search, but no real QuPRS speedup is established.

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
no general CNF clause factor: a real QuPRS post-RR exporter is still required.

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

**Not completed**: real post-reduction QuPRS PathSum export, source-exact
phase handling, Feynman/MQT real E2E benchmarks, 100/333-bit MPFR comparison,
Rank-width decomposition, ADD, arbitrary-angle soundness, and production
backend router. No QuPRS solver default or RaWMoCS core is changed.

Next step: source-pinned post-RR exporter + empirical feature histogram on
Feynman HWB6/QFT4 and MQT QNN4/Grover4/VQE4, then identical residuals
through new algorithms and existing WMC oracles. Entire exporter, planner,
evaluation and fallback time belongs in WallTime, with peak RSS and UNKNOWN.

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
HYP = proposed effect for real QuPRS workloads, NOT a proven speedup.
