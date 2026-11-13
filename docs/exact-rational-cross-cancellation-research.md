# QOO-213 — Exact rational cross-cancellation canonicality

Status: **mathematical lemma and research-only implementation candidate**; not a production solver change. Source baseline: RaWMoCS main `738e997fa4c3da9e42ed62b58af2e1a1dc91f96a`, `num-rational` **0.4.2** and `num-integer` **0.1.47** pinned by the separate benchmark workspace lockfile. Do not transpose these conclusions to approximate complex arithmetic or alternative representations.

Upstream source: https://docs.rs/num-rational/0.4.2/src/num_rational/lib.rs.html (look at `Mul<Ratio<T>>`, the two gcds, and its final `Ratio::new` call). This research explores whether that last normalization can be omitted **if and only if the input fractions are canonical**.

## Lemma: two cross-GCDs produce a canonical product

Assume
`x=a/b`, `y=c/d`, `a,c∈ℤ`, `b,d∈ℤ_{>0}`,
`gcd(|a|,b)=gcd(|c|,d)=1`.
Set
`g₁=gcd(|a|,d)`, `g₂=gcd(b,|c|)`, and
`A=a/g₁`, `D=d/g₁`, `C=c/g₂`, `B=b/g₂`.
All divisions are exact. Let `N=A·C`, `Q=B·D`.

1. `B,D>0`, so `Q>0`.
2. `gcd(|A|,D)=1`: dividing `a,d` by their complete gcd removes all common prime factors.
3. `gcd(|C|,B)=1`: the symmetric claim for `c,b`.
4. `gcd(|A|,B)=1` because `A|a`, `B|b`, and `a,b` were coprime.
5. `gcd(|C|,D)=1` because `C|c`, `D|d`, and `c,d` were coprime.

Any prime dividing both `|AC|` and `BD` would divide one of the four coprime pairs above, a contradiction. Thus `gcd(|N|,Q)=1`. Furthermore `N/Q=ac/bd`, including signed values.

**Zero edge:** for canonical `0/b`, necessarily `b=1`. Cross-cancellation yields `N=0`, `Q=1`; the product is canonical `0/1`. The same holds for zero in the other input. Signed numerators are permitted; denominators remain strictly positive.

**Counterexample to unguarded `new_raw`:** `2/4` and `1/3` are not both canonical. The cross-cancelled numerator and denominator can retain a common factor, even though their mathematical product equals a reduced ratio. A negative denominator and `0/2` are likewise disallowed as inputs. An unchecked `Ratio::new_raw` does not establish the hypothesis.

## Implementation and validation boundaries

- The candidate lives in `benchmarks/rust_core/src/cross_cancel_research.rs`, **not** `crates/rawmocs-core`. It uses `BigRational::new_raw` only after two cross-GCDs, with debug-mode canonical-input assertions.
- The Rust test corpus compares *numerator and denominator fields*, not only rational value, against an independently constructed schoolbook product `BigRational::new(a.numer()*b.numer(),a.denom()*b.denom())`. It covers zero, one, negative denominators normalized on input, reciprocal/divisible pairs, deterministic signed rationals, 521/607-bit values, and malformed raw negative controls.
- The benchmark binary explicitly verifies all fixed scalar and dense Gaussian candidate outputs against the current upstream implementation **before timing**. The scalar pair `n/(n+1) · (n+1)/(n+2)` creates an intentionally large cross-cancel. Dense Gaussian uses the existing `arithmetic_operands()` fixture and computes the established four-product formula with the research scalar multiplication as each subproduct.
- The benchmark comparison uses **same-run** Simulation values for `rational_cross_cancel_reference[128/512]` versus `rational_cross_cancel_without_final_reduce[128/512]`, and `complex_rational_multiply` versus `gaussian_dense_cross_cancel_without_final_reduce`. Existing names and production algorithms are unchanged.
- The benchmark includes GCDs, four numerator/denominator divisions, multiplications and BigInt clones. It is **not** pure GCD timing, allocation profiling, full-WMC WallTime, or a promise that `new_raw` is cheaper at the WMC level. Required production adoption evidence: exact Boolean SAT independently from numeric-zero results, signed/complex WMC, cache ON/OFF, real RSS, representative workload holdouts and same-host native A/B.
- The proof depends on the `BigRational` representation invariant. It does not justify carrying noncanonical raw results between solver stages or constructing/cache-keying invalid denominators.

## Decision criteria

Accept this PR as research infrastructure only when Rust Core, A0, all four cross-platform jobs, independent oracle tests, formatting/Clippy and CodSpeed all pass. Any production optimization must be a separate reviewable PR with an explicit canonicality contract and full-WMC science gates. If this candidate loses or the dense complex benefit is negligible, retain it as negative evidence and do not change the solver.
