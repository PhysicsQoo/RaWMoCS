# QOO-224 — Strict versus relaxed XOR native admission (source-scoped research)

Tracks [Linear QOO-224](https://linear.app/qoogohome/issue/QOO-224), following [QOO-223](qoo-223-relaxed-xor-mixed-cnf.md) / [PR #49](https://github.com/PhysicsQoo/RaWMoCS/pull/49), source base `e87fd0ee8ff79a2f33e45c11cb876a297fb96e5a`. All work is in unpublished `benchmarks/rust_core` behind the opt-in `branching-research` feature. **No public solver, cache, pivot, arithmetic, QuPRS or GPMC production changes.**

## Predeclared cohort and independent evidence

The dedicated [GitHub workflow](../.github/workflows/xor-native-admission-research.yml) pins original SATLIB source bytes (Dubois20 and uf20-01/02), reuses the independent exact Full Complex WMC oracle for SAT controls, and independently source-checks Dubois20's forty-equation Boolean UNSAT proof. Synthetic cases are deliberately small enough for a **separate truth-table enumeration and schoolbook four-real-product exact complex oracle**; synthetic rational weights are **not** from SATLIB and intentionally permit negative/complex contributions.

Eight fixed cases: SATLIB uf20-01 and uf20-02 (SAT), original Dubois20 (UNSAT), original Dubois20 plus one duplicated original clause (UNSAT), eight ternary forbidden assignments on one triple (UNSAT), the same plus one duplicate (UNSAT), a consistent parity half-space with a duplicate (SAT), and that same parity half-space with an independent contradictory unit pair (UNSAT but *not* necessarily provable from extracted XOR equations). All input construction happens **before** the recorded timer. The Dubois duplicate is checked to equal the original source plus exactly one clause. In both SATLIB sources, expected Full WMC is computed by the independent source-pinned mask oracle; the Boolean original SAT files are not confused with synthetic weights.

Three modes:
1. **baseline** — unchanged `solve_with_limits` bounded DPLL.
2. **strict** — QOO-222 `refute` / `verify`, then original DPLL fallback when no proof.
3. **relaxed** — QOO-223 `refute_relaxed` / `verify`, then original DPLL fallback when no proof.

For each case, compare cache caps `0` and `10000` and three rotated mode orders (same source/weights/limits): **8 × 2 × 3 × 3 = 144 PAIRED** workers and **16 SUMMARY** rows. Every mode executes in an independent Linux child with `timeout -k 1s 6s` plus `prlimit --as=536870912`; a parent 240s deadline and 15min workflow timeout bound CI. `RLIMIT_AS` is a cap on *virtual address space*, **not physical RSS/cgroup memory.max**. `VmHWM` is process-observed peak RSS; no OOM guarantee is inferred, and QOO-208 remains independently open.

Primary `wall_ns` spans full proof extraction + replay + fallback DPLL. It excludes process creation, CNF fixture parsing, and independent oracle setup. `PAIRED` rows include case, cache cap, trial order, mode, exact/UNKNOWN verdict, time, VmHWM KiB, proof equation count, and whether default DPLL was run. `SUMMARY` shows three-sample medians and baseline ratios **only when complete verdicts are identical**, otherwise `NA_DIFFERENT_OR_INCOMPLETE_VERDICT`. This avoids falsely treating Dubois baseline UNKNOWN as proving UNSAT faster.

## Science/negative controls and interpretation

The output checker requires exact Full WMC and separate SAT verdict whenever DPLL succeeds, and refuses a purported XOR UNSAT certificate that fails source replay or contradicts an independent SAT oracle. The strict/relaxed tests also cover mutated original clauses and resource dimension caps. All synthetic UNSAT cases must return a completed UNSAT verdict, even if a guard cannot extract a contradiction and needs fallback. A consistent XOR system with independent conflicting unit clauses is a **negative completeness control**: full CNF UNSAT need not be found by GF(2) alone.

These eight source-scoped cases **do not** establish general CNF admission, fair solver superiority, significant cross-hardware gain, hard RSS, QuPRS-generated CNF performance, projected WMC, or exact transcendental phase capability. Even if native timings are favorable on planted opposite-parity clauses, the practical default remains unchanged. The next step is **external, independent, mixed-XOR irregular/QuPRS prepared CNFs** from QOO-205/207 with source-locked Exact WMC/SAT proof and fresh-runner evidence; only then consider guarded default or adapter admission.

### Reproduce Linux

```sh
cargo test --locked --manifest-path benchmarks/rust_core/Cargo.toml --features branching-research xor_research
cargo run --quiet --locked --release --manifest-path benchmarks/rust_core/Cargo.toml \
  --features branching-research --example xor_native_admission_paired
```

At creation, this document defines the **required** gate. Do not report PASS or speeds before the final PR/source-pinned workflow has actually completed, and preserve negative cases.

## Accepted source-scoped qualification (2026-11-13)

- Final PR [#50](https://github.com/PhysicsQoo/RaWMoCS/pull/50): research HEAD `afa8b0657a970b2f5ba7993ad5ccb5d48173e7e6`, **14/14** final-head research/Science/portability/CodSpeed workflows `completed/success`. Squash-merged as [main `3fda2571b1a6274f32f35fe1e8a9861cd2feb4f5`](https://github.com/PhysicsQoo/RaWMoCS/commit/3fda2571b1a6274f32f35fe1e8a9861cd2feb4f5).
- [Native source-scoped workflow #43397275491](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43397275491): 144/144 `PAIRED`, 16/16 `SUMMARY` rows; [artifact ID 14023786264](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43397275491) contains `xor-native-admission-evidence.tsv` (GitHub retention 30 days). No missing verdict or invalid negative control. A setup-inclusive *solve path* WallTime excludes process startup, source fixture generation, and independent oracle calculation; it includes both XOR proof generation/replay and fallback DPLL.
- [Rust Science/A0/five-OS #43397275540](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43397275540), [CodSpeed #43397275559](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43397275559) successful. No existing `rawmocs-core` source or existing CodSpeed benchmark identity was changed by this PR. CodSpeed success is not a measured speedup in this 3-arm study.
- The **initial** PR commit `4e8ef13f8e0df6fe280f9b10fca53adda02492f0` failed Rustfmt and did **not** complete the native experiment; after style repair, `afa8b0657a970b2f5ba7993ad5ccb5d48173e7e6` is the only qualified experiment source. Do not mix the two runs.

### Exact original native SUMMARY medians

Values are **milliseconds** (ns in raw TSV); each median uses only *three* independent processes per exact case/mode/cache-cap, on one CI Linux machine. Two modes returning different verdict categories are **not comparable as speedups**.

| CNF | Cache | DPLL median | Strict median | Relaxed median | Comparison note |
| --- | ---: | ---: | ---: | ---: | --- |
| SATLIB uf20-01 (SAT) | OFF | 0.5448 | 0.5665 | 0.5828 | Same SAT+exact WMC; both preprocessing modes slower |
| SATLIB uf20-01 (SAT) | 10000 | 0.6625 | 0.6953 | 0.6913 | Same SAT+exact WMC; both slower |
| SATLIB uf20-02 (SAT) | OFF | 0.8340 | 0.8526 | 0.8588 | Same SAT+exact WMC; both slower |
| SATLIB uf20-02 (SAT) | 10000 | 0.9785 | 0.9999 | 0.9902 | Same SAT+exact WMC; both slower |
| Dubois20 original UNSAT | OFF | 34.6772 (UNKNOWN) | 0.0661 (UNSAT) | 0.0675 (UNSAT) | `NA_DIFFERENT_OR_INCOMPLETE_VERDICT` |
| Dubois20 original UNSAT | 10000 | 37.2112 (UNKNOWN) | 0.0657 (UNSAT) | 0.0653 (UNSAT) | `NA_DIFFERENT_OR_INCOMPLETE_VERDICT` |
| Dubois20 + duplicate | OFF | 34.6004 (UNKNOWN) | 34.8982 (UNKNOWN) | 0.0622 (UNSAT) | `NA_DIFFERENT_OR_INCOMPLETE_VERDICT` |
| Dubois20 + duplicate | 10000 | 37.8150 (UNKNOWN) | 37.4496 (UNKNOWN) | 0.0636 (UNSAT) | `NA_DIFFERENT_OR_INCOMPLETE_VERDICT` |
| Opposite parity (UNSAT) | OFF | 0.00925 | 0.01349 | 0.00579 | Same UNSAT, relaxed proof 2 equations |
| Opposite parity (UNSAT) | 10000 | 0.01099 | 0.01523 | 0.00650 | Same UNSAT, relaxed proof 2 equations |
| Opposite parity + duplicate (UNSAT) | OFF | 0.00884 | 0.01178 | 0.00632 | Same UNSAT, relaxed proof 2 equations |
| Opposite parity + duplicate (UNSAT) | 10000 | 0.01061 | 0.01429 | 0.00622 | Same UNSAT, relaxed proof 2 equations |
| Consistent parity + duplicate (SAT) | OFF | 0.02011 | 0.02440 | 0.02338 | Same SAT+exact WMC, overhead |
| Consistent parity + duplicate (SAT) | 10000 | 0.02412 | 0.02814 | 0.02734 | Same SAT+exact WMC, overhead |
| Parity plus non-XOR unit conflict (UNSAT) | OFF | 0.00338 | 0.00616 | 0.00872 | DPLL resolves faster; both XOR guards fall back |
| Parity plus non-XOR unit conflict (UNSAT) | 10000 | 0.00600 | 0.00795 | 0.00986 | DPLL resolves faster; both guards fall back |

**Memory:** `VmHWM` for SATLIB uf20 examples was of order 2.6–2.7 MiB, with small mixed differences; the Dubois baseline uses more than the GF(2) source proof in this run, but the verdicts are different. Not a measured hard-RSS bound, not a cross-hardware gain, and not a robust median memory claim.

**Science interpretation:** Existing strict QOO-222 proof already handles original Dubois20; relaxed QOO-223 uniquely recovers a source-replayable 40-equation GF(2) UNSAT witness for the *derived* duplicate-clause Dubois20. The synthetic opposite-parity UNSAT cases have same complete verdict in all three modes and show a small on-runner advantage of relaxed proof over DPLL (observed 1.4–1.7×), but solve times are only 6–15 microseconds and three samples are too few to assert portable or significant speedup. The external SATLIB SAT examples reveal measurable preprocessing cost; quick non-XOR conflict is the counterexample where relaxed is ~2.6× slower (Cache OFF).

**Decision:** retain opt-in research implementation and proof replay; **DO NOT enable unconditional relaxed XOR source extraction in production**. The next admission-relevant milestone is independent and externally sourced mixed-XOR/QuPRS-prepared CNF holdouts with exact source-faithful Gaussian WMC witnesses, repeatable multi-runner inclusive timings and an explicit no-regression policy. A future implementation must prove decision overhead under non-XOR workloads and avoid returning SAT on missing proof. QOO-205/207 own corpus and search heuristics, QOO-208 owns genuine kernel memory safety.
