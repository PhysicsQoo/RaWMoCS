# Structural cache admission holdout study

This is a **research-only policy ablation**, not a production cache policy
or a measured real-world cost-aware admission algorithm. It follows the
feature-gated cache transaction attribution work in
[cache-cost-profiling.md](cache-cost-profiling.md).

## Scope and implementation boundary

The independent `cache-admission-research` Cargo feature is **disabled
by default**. With that feature the Rust solver offers
`solve_with_min_cache_clauses(problem, limits, min_clauses)`: a cache
entry is stored only if the already-normalized residual key contains at
least the fixed minimum number of clauses. All normal cache lookups
remain enabled; the new policy filters **insertions only**, not hash/key
construction. `min_clauses=0` must behave like
`solve_with_limits`. The feature exposes a separate rejection counter.
No existing public default, cached numeric semantics, or benchmark ID
changes.

This simple threshold is intentionally a **negative/positive control**
for admission-policy experiments. Number of clauses is not a sufficient
predictor of remaining DPLL work, cache reuse, physical RSS, or exact
Gaussian-rational arithmetic cost; any policy based solely on it is a
hypothesis, not a validated production heuristic. Do not use these
results to change QuPRS GPMC selection.

## Three predeclared connected holdout graph families

All clauses have the form `(x|y_a|y_b) & (!x|y_a|y_b)`.
Both decisions on the high-frequency selector `x` produce the
identical connected residual graph CNF, but branching and decomposition
vary with the graph topology.

- `cycle`: one n-vertex ring (pre-existing transfer-DP positive control)
- `rungs`: an n-vertex ring with n/2 opposite-vertex edges
- `spokes`: an n-vertex ring with additional odd-index edges to vertex 0

Only the predeclared `n = 6, 10, 14` are accepted. The graph's exact
Full Complex WMC is certified independently by enumerating all
`2^n` vertex subsets, counting the independent sets by cardinality and
summing complex weights `(2+i)^(n-k) (1-i)^k`, multiplied by the
free selector's fixed exact Gaussian-rational weight sum.
This oracle does not invoke the DPLL implementation, its memo table,
or any solver-generated WMC answer. The cycle family is further
cross-checked against the older independent transfer recurrence.

## Fixed policy matrix

For each family and n, compare four policies on the **same** CNF and
weights:

| Policy | Cache entry cap | Minimum retained key clauses |
| --- | ---: | ---: |
| `off` | 0 | not applicable |
| `legacy` | default 10,000 | 0 (unaltered defaults) |
| `min4` | default 10,000 | 4 |
| `min12` | default 10,000 | 12 |

Three independent Linux child processes per case with rotated policy
order produce **108 worker records** (3 graph families × 3 sizes × 4
policies × 3 trials). Every row is admitted only after SAT and the
independent exact WMC oracle agree. Output includes solve WallTime,
Linux VmHWM peak resident memory, search nodes, cache hits/entries,
threshold rejections and logical retained payload counters.

As with earlier work, these children have a five-second parent
deadline, 512 MiB Linux `RLIMIT_AS` **virtual address space** guard
(**not** an RSS cap), and separate forced allocation-denial and
timeout negative controls. Nonzero exits are not mislabeled as OOM.
The workflow rejects missing rows and duplicated/truncated records.
The prior Cache/Quota/Repeated/Connected/Cost study datasets remain
separate, and normal-build CodSpeed baseline identities remain intact.

To reproduce the feature-gated study on Linux:

```sh
cargo test --workspace --features cache-admission-research
cargo test --locked --manifest-path benchmarks/rust_core/Cargo.toml --features cache-admission-research
cargo run --locked --release --manifest-path benchmarks/rust_core/Cargo.toml \
  --features cache-admission-research --example cache_profile -- --admission-suite
```

Run `cache-admission.yml` for the pinned CI and CSV artifact. Do not
treat three samples from one runner as a statistically significant
speedup or physical memory advantage. The threshold parameters were
fixed **before** measurement; inspecting their performance is exploratory,
not out-of-sample validation of an automatically tuned threshold.
A subsequent phase must add unrelated actual/irregular CNF workloads,
repeat across runners and measure both production WallTime and RSS.
