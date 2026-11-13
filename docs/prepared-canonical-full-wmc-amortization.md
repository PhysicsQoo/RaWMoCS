# QOO-218 — Prepared canonical weights: exact Full WMC amortization study

Status: research-only. Base source RaWMoCS main `c7b696f85124011c395c1da25d59ea2d65b20866`; QOO-217 showed that per-call cloning, validation and normalization can dwarf multiplication in trivial SAT/UNSAT formulas. The `canonical-mul-research` Cargo feature is disabled by default. The public `solve`, `solve_with_limits`, and `Gaussian::mul` defaults and QuPRS/GPMC integrations do not change.

## Private immutable canonical representation

`PreparedCanonicalProblem` has **private fields**, can only be created through `PreparedCanonicalProblem::prepare(&Problem)`, and exposes no mutation or unchecked construction. Preparation uses the exact same `normalize_research_problem` helper used by the existing QOO-217 opt-in canonical arithmetic path:
- clone the caller's entire problem;
- check every one of four denominator fields per variable and fail closed on zero denominators;
- canonicalize all four real/imag positive/negative rational fields per variable with `BigRational::new`.

The accessor `solve_with_limits` creates a **new DPLL Engine and cache** on every solve (no hidden solver results or state reuse). It uses the previously proved cross-cancelled rational multiplication on the privately canonicalized fields. The construction and solver methods are explicitly gated and independent of the default Rust API.

## Predeclared native evidence

Three arms on one Linux runner, each in a **fresh independent process**, order rotated across three rounds:
1. `baseline`: repeatedly call existing `solve_with_limits` on the original problem.
2. `gated`: repeatedly call `solve_with_canonical_mul_research` (clone/check/normalize every single solve).
3. `prepared`: prepare **once per worker** and then repeatedly call `PreparedCanonicalProblem::solve_with_limits`. Preparation cost remains inside primary end-to-end WallTime and its own `setup_ns` is additionally reported as a breakdown, **not subtracted from main results**.

Each process uses the fixed number of solves `N∈{1,2,12,64}`, both cache caps (0, 10000) and exact canonical signed/complex WMC witness plus independent Boolean SAT and structural stats parity. Count is **13 fixtures × 2 caps × 4 N values × 3 rounds × 3 arms = 936 paired rows**, **104 summary rows**, and **936 setup rows**. `VmHWM` measures peak process RSS but does not isolate per-object allocations or impose a hard memory limit.

Replayed QOO-217 seven cases: cycle10, repeat8, chain16, pairs16, cancel, unsat, noncanonical. Replayed QOO-217 four preregistered holdouts: chain24, pairs24, forced20, contradict20. **New unseen** holdouts (not used in QOO-214/216/217): `chain32_holdout` with closed-form arithmetic WMC and `contradict32_holdout` with UNSAT and 32 signed/complex rational weights. These were named in source before timing; do not tune selection thresholds on this cohort and then call it independent.

## Scientific caveats and gates

- `PreparedCanonicalProblem` is **not** a production recommendation or a new default persistent cache. Its solver caches are recreated each call.
- A `PreparedCanonicalProblem` contains a copied canonicalized source problem; memory lifetime may be larger than one-shot calls. Compare VmHWM and retain independent exact WMC/Boolean SAT evidence.
- The `setup_ns` measurement occurs within the same worker's total timer and is only meaningful for the `prepared` arm. Do not subtract timings to invent pure GCD or allocation costs.
- Report both N=1 (where setup is unlikely to amortize) and repeated N; ignore apparent sub-microsecond gains unless consistent across multiple rotations and an independent source-locked holdout.
- Require Rust Core/A0 Windows/Linux ARM/macOS ARM+Intel, fmt/Clippy, independent signed/complex WMC and canonical numerator/denominator exact parity, SAT-but-zero and malformed weight negative controls, native WallTime/VmHWM, and all historical 62 CodSpeed identities unchanged.
- Further work must consider truly realistic CNFs and memory, and may reject production adoption if either setup cost or persistent prepared storage is unjustified.
