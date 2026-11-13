# QOO-217 — Exact input boundary normalization cost versus solver multiplication

Research only. Base RaWMoCS main: `d4011de85ba20637c4c209ad8eb25de0358546ff`. The experimental `canonical-mul-research` feature remains **disabled by default**, and the standard `solve`, `solve_with_limits`, public `Gaussian::mul`, cache strategies, QuPRS and GPMC are unchanged.

## Three-arm controlled experiment

Both research arms call the **same** `normalize_research_problem(&Problem)` helper, which clones the whole `Problem`, rejects zero denominators and canonicalizes all four complex rational components per variable with `BigRational::new`:
1. `baseline`: `solve_with_limits(p, limits)`, normal multiplication and no input canonicalization.
2. `boundary`: `solve_with_normalized_input_research(p, limits)`, identical clone/canonicalize/validation to the candidate, then *ordinary* normalization-preserving multiplication.
3. `gated`: `solve_with_canonical_mul_research(p, limits)`, same clone/canonicalize/validation, then QOO-213 cross-cancelled multiplication without redundant final gcd.

All three measure the *whole* solve. In particular, the normalizer is **not** accidentally hidden outside the gated arm. The two research arms are admissible only for nonzero denominators; the default solver's handling is unchanged.

**Interpretation warning:** subtracting medians or ratios is **not** pure GCD, cloning, allocation or normalization cost. Differences include representation/cache/CPU effects and are not necessarily additive, especially on sub-millisecond cases. `baseline` vs `boundary` approximates the incremental impact of validation, full problem clone and weight canonicalization **together**. `boundary` vs `gated` compares multiplication algorithms under the same pre-normalized problem representation.

## Predeclared cases and controls

Seven *replayed*, previously observed QOO-214 cases: `cycle10`, `repeat8`, `chain16`, `pairs16`, `cancel`, `unsat`, `noncanonical`.

Four *new*, fixed and source-pinned holdouts that were **not** used to design the QOO-216 threshold:
- `chain24_holdout`: exact, complex EqualityChain on 24 variables; independent closed-form oracle.
- `pairs24_holdout`: exact, complex IndependentPairs 24 variables; independent factorized oracle.
- `forced20_holdout`: twenty forced positive unit literals, weights 1/0 and expected exact WMC = 1.
- `contradict20_holdout`: two contradictory unit clauses with 20 signed/complex rational weights; SAT=false, WMC=0 independently.

All expected results are independent of the DPLL algorithm. Existing feature-gated `canonical_full_wmc_research.rs` checks exact numerator and denominator fields, SAT/numeric-zero distinction, invalid denominators, large signed complex rationals, and cache statistics across all three arms.

## Measurement contract

For each of **11** cases × **2** cache caps (0, 10000) × **2** call counts (1 and 12) × **3** fixed rotating execution orders × **3** arms:
- Spawn fresh native Linux processes per arm. Worker times 1 or 12 complete solves, including normalizer and book-keeping. Importantly the 12-call batch still canonicalizes before **every** solve, so it is a repeated-call control, *not* a prepared-input/amortized comparison.
- Independently verify Boolean satisfiability and exact complex weighted count against the predeclared witness and compare SAT/count/core solver statistics across arms.
- Retain full WallTime, `/proc/self/status` VmHWM (peak process resident memory), **396 PAIRED rows**, and **44 SUMMARY rows** in a raw CI artifact.
- Do not infer per-solve allocation, a hard memory cap, or cross-platform RSS from VmHWM; multiple short repeated process launches can be noisy. Timing is one Linux host, not all supported CPUs.
- Existing 62 historical CodSpeed identities are not changed. Cross-run hardware comparisons are not considered proof of performance.

## Next decision

This is *measurement and controlled attribution*, not a production API adoption. A separate future research iteration may prepare a canonicalized problem once and amortize multiple solves, with explicit setup/memory cost and source-locked new holdouts. Reject default adoption unless genuine end-to-end Full Complex WMC improvements persist, independent SAT and canonical field parity holds, and realistic CPU/RSS evidence exists.
