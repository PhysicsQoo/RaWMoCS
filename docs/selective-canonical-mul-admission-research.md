# QOO-216 — Selective canonical Full WMC arithmetic: falsifiable admission study

Research status: **predeclared, benchmark-only static policy**. Base `main` SHA `a78ed5cc075012cce6fdb9ba079cf1e3d6dbf27a` (QOO-214 merged, `canonical-mul-research` feature disabled by default). This is neither a production heuristic nor a Full WMC speedup guarantee.

## Decision rule and invariants

The experimental `admit(problem)` is evaluated *before solving* and is timed as part of the selective strategy. It selects the gated, canonicalizing exact multiplication only if all four conditions hold:

1. `variables >= 8`;
2. `clauses.len() >= variables / 2`;
3. every source clause has length at least 2;
4. at least one variable has both positive and negative weights with nonzero imaginary numerators.

Otherwise it uses the existing `solve_with_limits`. The check is a cheap static scan of source CNF/weights, **not** an estimate of GCD costs, number of actual multiplication calls or SAT. No benchmark outcome can affect selection. This threshold is knowingly naive and is tested against *adversarial holdouts*, not tuned after seeing their results.

The gated arm uses `solve_with_canonical_mul_research` from QOO-214, which clones, validates and normalizes external inputs. Both normalization and admission overhead are **included** in worker WallTime. Direct normal default solvers are unchanged.

## Predeclared 15 cases

QOO-214 calibration cohort: `cycle10`, `repeat8`, `chain16`, `pairs16` (expected select); `cancel1`, `unsat1`, `noncanonical1` (expected fallback).

Independent held-out positives: `cycle14_holdout`, `repeat12_holdout` (expected select; larger than calibration).

Explicit **false-positive**/arithmetic-light controls:
- `units16_holdout` forces all variables through unit clauses; expected fallback.
- `identity16_holdout` has 16 variables, many clauses, but all positive and negative weights equal one; expected fallback.
- `sparse16_holdout` has 16 variables and mostly zero-valued branches; expected fallback.
- `contradict16_holdout` is an unsatisfiable source CNF with no unit clause, 16 variables, >8 binary clauses and dense complex rational weights. It **passes the naive static gate despite no useful weighted multiplication**, intentionally exposing a potential false-positive and forbidding broad adoption claims.

Explicit **false-negative** tests:
- `cycle6_holdout` and `repeat2_holdout` have nontrivial complex weights and search but fewer than 8 variables; expected fallback, though gated arithmetic may be faster.

All expected exact Full WMC values come from existing source-pinned independent transfer-DP/algebraic oracles or trivial provable SAT cases; none are copied from the production solver.

## Three-arm source-locked evidence

For every case × two cache caps (0, 10000) × three fixed rotation orders, independently run `baseline`, `gated` and `selective` in **fresh Linux processes**, each performing 12 solves. Each worker verifies the independent exact complex WMC and Boolean SAT before outputting a native WallTime and `/proc/self/status` peak `VmHWM`. The parent verifies deterministic selection, SAT/count/core solver statistics parity, and emits all 270 raw trial rows, 30 median summaries and 15 admission decisions.

`VmHWM` is the **entire process high-water memory**; it does not isolate allocations or impose a hard quota. Very short sub-millisecond timings remain noisy; medians of 3 rounds and alternating order help but cannot eliminate noise. The experimental admission scans source clauses rather than normalized residuals, so tautologies/duplicates may produce misleading scores; this is a deliberate limitation.

## Gates and conclusion policy

- CI: Rust benchmark formatting, Clippy and independent full-WMC oracle under the opt-in feature; 15 × 2 × 3 × 3 isolated-process native cohort and saved TSV. The default test/portability suite remains unchanged; the research example is compiled only with the explicit feature.
- Outcome categories: false positive, false negative, fallback saved overhead, and heavy-workload retention. **Do not automatically tune the same threshold on holdout results and then report improved holdout accuracy.** A future new cohort is required for any revised policy.
- Only propose a separate default-implementation PR if a richer, independent real-world CNF corpus shows material native Full WMC benefits without SAT-but-zero, cache/precision or RSS regressions. No QuPRS/GPMC changes.
