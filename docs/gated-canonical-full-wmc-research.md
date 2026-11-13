# QOO-214 — Gated canonical rational multiplication in Full Complex WMC

Status: **source-isolated research feature**. Main parent SHA at branch creation: `dae5e6549e0c95db944dd3e3e6e88deaf5dfe52b`. No production default behavior or performance claim. Previous arithmetic proof and positive/negative controls: `docs/exact-rational-cross-cancellation-research.md` (QOO-213).

## Unsafe assumption rejected

`Gaussian::new(BigRational, BigRational)` preserves whatever numerator and denominator fields the caller provided. `Problem::new` validates variable counts and literals, **not weight canonicality**. A public `BigRational::new_raw(2,4)`, `new_raw(1,-2)` or `new_raw(0,7)` therefore crosses the current public boundary. A zero denominator can also be supplied and is invalid for rational arithmetic. Do not infer `gcd(|n|,d)=1` or `d>0` from the `BigRational` type alone.

QOO-213's cross-cancel multiplication lemma requires strictly positive canonical denominators and coprime components. An unchecked global replacement of `BigRational::mul` with `new_raw` could break algebraic or cache invariants.

## Explicitly gated experiment

The `canonical-mul-research` Cargo feature is **OFF by default** in `rawmocs-core`.

When explicitly requested through `solve_with_canonical_mul_research(p, limits)`, the research path:

1. Clones the externally supplied `Problem` and validates all four rational denominators per variable.
2. **Fails closed** with a distinct research error on a zero denominator; does not continue with undefined rational arithmetic.
3. Canonicalizes each external fraction **once**, via `BigRational::new` (cost included in end-to-end timings).
4. Creates an internal DPLL `Engine` with a private research flag; all internal weighted multiplications use the QOO-213 proven two-GCD `BigRational::new_raw` implementation. Other searches, numerical sums, cache keys, admission and limits are unchanged.
5. Preserves the public `Gaussian::mul` and the default `solve_with_limits` / `solve` algorithm. Direct public arithmetic on unchecked `Gaussian` values never calls the research path.

**Canonical propagation argument:** all entry weights are canonical after step (3). `Gaussian::one/zero` are canonical; the private cross-cancel multiplication preserves the invariant by QOO-213; `BigRational` addition and subtraction normalize results; cloning a canonical value preserves its representation. All internal DPLL branches, cache hits, residual returns and components are composed only through these operations. This is a research contract requiring tests and inspection, not an assertion that future representations automatically preserve it.

## Independent science gates

- `benchmarks/rust_core/tests/canonical_full_wmc_research.rs`: bounded independent Boolean enumeration for signed and complex weights, 64 deterministic four-variable connected controls, zero/one, SAT-but-exact-zero cancellation, UNSAT, 521/607-bit fractions, and noncanonical-but-valid external inputs. Every case compares exact rational counts, SAT and cache-on/cache-off search statistics with the original solver.
- Invalid denominator is rejected as an explicit research error before arithmetic. **The default solver's invalid-denominator behavior is neither changed nor claimed valid.**
- `benchmarks/rust_core/examples/canonical_mul_paired.rs`: 7 immutable case names (cycle10, repeat8, chain16, pairs16, SAT-but-zero, UNSAT, noncanonical input) and 2 cache caps (0, 10000). For every case/cap: 3 alternating-order rounds; each baseline/candidate run is a new isolated Linux process with 12 iterations. Every worker verifies exact expected count and independent Boolean SAT before reporting.
- Peak `VmHWM` is a **process high-water mark**, not a hard RSS limit, not live per-call allocation, and not a portable macOS/Windows metric. WallTime is inclusive of the research **input normalization**. These synthetic cases do not prove general speedups or real-world CNF performance.
- The workflow `canonical-mul-research.yml` runs the exact feature's fmt/Clippy/tests/native evidence and archives the raw TSV; standard PR science and cross-platform CI cover default behavior. Historical CodSpeed identities and production release flags remain unchanged.

## Adoption gate (not satisfied by this PR)

To contemplate a production default change, separately show successful proof and independent exact canonical-field parity at external input boundaries, no SAT/numeric-zero conflation, no cache or negative-weight regressions, meaningful source-locked **native** Full WMC WallTime improvement on held-out connected/irregular cases, realistic CPU and RSS across the target platforms, and explicit accounting for boundary normalization overhead. Otherwise archive the negative result and retain the existing normalizing multiply.
