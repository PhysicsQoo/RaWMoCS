# QOO-223 — Relaxed source-backed XOR extraction on mixed CNFs

Research-only successor to [QOO-222](qoo-222-xor-gf2-research.md). Tracking: https://linear.app/qoogohome/issue/QOO-223 . Base: `main@eac54f13847ffef5e0b2468f77e6abfa2c82f0eb`.

## New tested hypothesis

The QOO-222 strict policy extracts a triple only when **exactly four** clauses forbid all four assignments of one parity. A stronger, still sound claim is: **four distinct original three-literal masks of the same forbidden parity are sufficient to imply the opposite XOR**, even with duplicate clauses, unrelated clauses, or extra opposite-parity clauses on the same variables. For every extracted equation, `clauses` stores **four distinct original clause indices**. `verify(original CNF, proof)` checks their exact literals and parity independently before accepting the GF(2) zero-LHS/one-RHS contradiction.

`refute()` remains the original strict baseline; `refute_relaxed()` is exclusively in the benchmark workspace's opt-in `branching-research` module. Neither mode changes `rawmocs-core`, its public API, cache, pivot, Gaussian arithmetic or QuPRS/GPMC. No contradiction means **no proof**, not SAT or a nonzero WMC. A verified Boolean contradiction entails WMC zero regardless of valid signed/complex weights, while SAT with numerically zero WMC is not UNSAT.

## Source oracle and controls

- Eight original ternary clauses on variables 1/2/3 contain both complete parity halves: **brute-force UNSAT**, strict extractor refuses the eight-clause triple, relaxed extractor derives and replays the two contradictory equations. A referenced original clause mutation invalidates the exact witness.
- Adding an identical duplicate and unrelated mixed unit/binary clauses to the same CNF must preserve the relaxed witness while strict still conservatively refuses.
- A single complete parity group is SAT by independent brute force. Adding contradictory non-XOR unit clauses makes the **full CNF UNSAT** but leaves its XOR subsystem consistent; the extractor must still refuse a false proof. This proves incompleteness is intentionally distinct from unsoundness.
- Source-pinned SATLIB Dubois20 stays refutable if one valid clause is duplicated; original QOO-222 strict policy refuses it. Existing `uf20-01/02` controls and 200 SAT randomized subsets remain active. Clauses/variables above existing 10,000/256 guard must fail closed.
- No source clause is removed or rewritten for the normal DPLL fallback. The source indices in `XorRefutation` are absolute positions in the supplied original CNF, not normalized/resorted positions.

## Gate and reproducibility

```sh
cargo fmt --manifest-path benchmarks/rust_core/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path benchmarks/rust_core/Cargo.toml --all-targets --features branching-research -- -D warnings
cargo test --locked --manifest-path benchmarks/rust_core/Cargo.toml --features branching-research xor_research
```

The pre-existing [XOR source-check CI](../.github/workflows/xor-refutation-research.yml) is path-triggered by the modified source; it also retains the original 36 paired Linux worker / six-summary strict XOR-vs-bounded-DPLL study and SATLIB provenance checks. **The new relaxed tests establish soundness/completeness examples only; they do not themselves provide a new relaxed-vs-strict native timing claim.** CI success must be checked before reporting these as passed.

## Next experiment / no-default-admission

Measure three paths—strict source guard, relaxed source guard, unmodified bounded DPLL—on the same source-locked CNF, cache OFF/Legacy, with identical complex exact semantics, separately verified SAT/UNSAT and independent fresh child WallTime/VmHWM. Include original SATLIB (20-variable SAT truth tables, 60-variable Dubois UNSAT) and an **independent external mixed-XOR holdout**, not only synthetic duplicate-clause cases. Retain `NA_DIFFERENT_VERDICT` for UNKNOWN vs PROVEN_UNSAT and avoid precision or performance claims from it. Require bounded parent deadline and explicitly label RLIMIT_AS (virtual address space) as not a hard RSS limit. Keep research-only until QOO-205/207 holdouts and QOO-208 guards qualify production admission.
