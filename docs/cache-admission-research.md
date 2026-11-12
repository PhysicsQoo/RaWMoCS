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


## First source-scoped structural admission observation

PR source SHA `1e0d1be5ef873b001fb3f63292023b62993276e3` passed
[Linux structural-admission run 43186678573](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43186678573):
**108/108** worker rows, source-independent exact Gaussian-rational
Full WMC/SAT oracle, default-policy parity and zero-node-budget
fail-closed tests, plus both forced memory-denial and timeout controls.
The `admission-profile.csv` and `admission-guards.log` are GitHub
Actions artifacts with limited retention.

Median solve WallTime (ms), **three fresh processes per policy on the
same GitHub-hosted Linux runner**, not CodSpeed Simulation. These
synthetic graph families were predeclared; this is not a statistically
independent real-world holdout study.

| Graph | Vertices | Cache OFF | Legacy | Min4 | Min12 |
| --- | ---: | ---: | ---: | ---: | ---: |
| cycle | 6 | 0.118 | 0.087 | 0.079 | 0.128 |
| cycle | 10 | 0.400 | 0.234 | 0.245 | 0.422 |
| cycle | 14 | 1.272 | 0.506 | 0.555 | 0.720 |
| rungs | 6 | 0.107 | 0.081 | 0.079 | 0.116 |
| rungs | 10 | 0.369 | 0.247 | 0.234 | 0.219 |
| rungs | 14 | 1.034 | 0.642 | 0.603 | 0.575 |
| spokes | 6 | 0.110 | 0.078 | 0.074 | 0.114 |
| spokes | 10 | 0.317 | 0.182 | 0.201 | 0.187 |
| spokes | 14 | 0.919 | 0.328 | 0.392 | 0.496 |

At 14 vertices, the deterministic search-node, hit and retained-entry
counts are:

| Graph | Policy | Nodes | Hits | Entries |
| --- | --- | ---: | ---: | ---: |
| cycle | off | 231 | 0 | 0 |
| cycle | legacy | 69 | 25 | 44 |
| cycle | min4 | 85 | 9 | 28 |
| cycle | min12 | 117 | 1 | 5 |
| rungs | off | 179 | 0 | 0 |
| rungs | legacy | 89 | 14 | 75 |
| rungs | min4 | 91 | 1 | 28 |
| rungs | min12 | 91 | 1 | 10 |
| spokes | off | 151 | 0 | 0 |
| spokes | legacy | 41 | 15 | 26 |
| spokes | min4 | 49 | 7 | 18 |
| spokes | min12 | 77 | 1 | 5 |

**Interpretation:** Min12 appears faster than Legacy on rungs-10 and
rungs-14 in these three processes, despite retaining fewer entries.
But it is materially slower on cycle-6, cycle-10 and spokes-14, where
rejecting relatively cheap/small keys can also eliminate valuable
residual reuse. Min4 has smaller, mixed effects. A one-dimensional
clause-count threshold therefore **does not generalize even across
these three related synthetic graph shapes**. Do not switch the
default cache admission strategy or extrapolate this to QuPRS.

The physical Linux `VmHWM` differences were small and mixed, with no
certified consistent memory improvement. The Linux `RLIMIT_AS`
restriction limits virtual address space, not resident memory.
The medians above cannot establish significance or cross-machine
portability. They also do not measure key-build/lookup/insert self-cost:
those are in the separate feature-gated cache-cost experiment.

The PR's existing **37 native CodSpeed Simulation identities were all
preserved**; [CodSpeed comparison to the preceding main
run](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/compare/6af57d381256b96ba6f6af9a..6af57f065487f3a70f815216)
reports 37 unchanged, but explicitly records a **different CI CPU**
(EPYC 7763 versus EPYC 9V74). This comparison must not be represented
as a precise same-hardware performance bound.

**Decision:** Keep the research feature opt-in and retain the default
unmodified. A future genuinely cost-aware policy should quantify
expected avoided search work and key/lookup cost, use separate
discovery and held-out irregular workload sets, and compare real
WallTime/RSS with repeated fresh workers and exact semantic oracles.
