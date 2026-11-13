# QOO-215 — Irregular Full Complex WMC and prepared-input amortization extension

Status: research-only follow-up to QOO-218. Source baseline: RaWMoCS `main@4d7bc22814f75247653b0a5f5c1f2d312654b018`, with `PreparedCanonicalProblem` already available **only** under the disabled-by-default `canonical-mul-research` feature. This experiment does not modify `rawmocs-core`, default solving, cache admission, or QuPRS/GPMC.

## Source-declared new holdouts

`benchmarks/rust_core/src/irregular_holdout.rs` introduces **two unseen, deterministic, synthetic irregular CNFs** with full signed/complex rational weights.

- `irregular8`: 8 variables, 11 mixed-sign and mixed-width clauses (2 or 3 literals).
- `irregular10`: 10 variables, 14 mixed-sign and mixed-width clauses (2 or 3 literals).

Both have heterogeneous variable incidence rather than equal-degree chain/cycle shapes. The clauses and formulas are constant in source; there is no random seed search or manual adjustment after examining timings. This cohort is **not** an industry CNF, QuPRS fixture, or paper benchmark. It is merely an additional irregular synthetic holdout. A subsequent study must gather independently source-pinned real instances before making production claims.

For both cases, the witness is constructed by **independently enumerating every Boolean assignment** (2^8 or 2^10), evaluating the CNF literally and multiplying signed/complex rational weights using four schoolbook products. The oracle intentionally uses neither the Rust DPLL solver, solver cache, `Gaussian::mul`, nor the candidate cross-cancellation implementation. The scalar/rational arithmetic itself uses the library's exact `BigRational` primitives.

The standalone unit test checks the oracle against the default Full WMC result with cache OFF/ON. During native evidence collection, baseline, gated-per-call and prepared-once all check SAT and exact WMC against the independently calculated witness; candidate outputs additionally check canonical numerator and denominator **fields**, not only mathematical value.

## Full setup-inclusive reuse matrix

Re-run the preceding 13 source-declared fixtures without changes, append the two new cases, and test:

- `N ∈ {1, 2, 4, 12, 16, 64}` — includes the QOO-215 missing 4/16 repeats while retaining QOO-218's historical 12.
- `cache_entries ∈ {0, 10000}`.
- `baseline`, `gated` and `prepared`; three rotating execution orders on the same Linux runner and separate fresh processes for each arm.

The expected raw evidence consists of **15 × 2 × 6 × 3 × 3 = 1620 PAIRED rows**, **180 SUMMARY rows**, **1620 SETUP rows**; CI verifies each count and archives the unprocessed TSV. `prepared`'s primary elapsed time includes `PreparedCanonicalProblem::prepare` exactly once per worker; setup time is separately reported but never subtracted to inflate end-to-end speedups. A new DPLL Engine/cache is constructed for each repeat; no result caching or persistent search-state reuse is introduced.

VmHWM is the Linux process high-water RSS (not per-call allocations or an enforced resource budget). For very fast UNSAT or SAT-but-zero fixtures, sub-millisecond clock noise and process setup can overwhelm arithmetic and invalidate broad comparative claims. Small-input regressions remain essential negative controls.

## Decision gate

A successful research-only PR requires exact tests, all target-platform Core/A0 and selected research science, fmt/Clippy, unchanged CodSpeed benchmark identity, and a same-host native process-isolated three-arm comparison. Do **not** treat these new two synthetic irregular shapes as realistic independent production workloads. QOO-215 remains incomplete until verified paper-derived / industrial irregular CNFs with provenance, broader resource profiles and a defensible preparation break-even rule are available.

No part of this research authorizes enabling `canonical-mul-research` by default or replacing public `Gaussian::mul`.
