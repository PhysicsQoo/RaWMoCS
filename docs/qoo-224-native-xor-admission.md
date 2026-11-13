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
