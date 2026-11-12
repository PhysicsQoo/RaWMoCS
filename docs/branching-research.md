# DPLL branch-order policy ablation (research only)

This experiment does **not** change the normal Full Complex WMC solver
or QuPRS's solver selection. The independent `branching-research`
Cargo feature is disabled by default. It offers
`solve_with_pivot_policy(problem, limits, policy)` solely to compare
integer, deterministic DPLL variable-order choices with all cache
admission rules fixed.

## Predeclared pivots

| Pivot | Exact ranking |
| --- | --- |
| `frequency` | Original literal-occurrence maximum and last-variable tie break (baseline) |
| `short` | Sum of `2^(8-min(clause_len,8))` across all occurrences, with frequency as secondary key and the same last-variable tie break |
| `minfill` | Minimize the number of missing edges among current primal-graph neighbors, then maximize frequency, preserving the baseline last tie break |

The min-fill implementation allocates a bounded adjacency matrix only
if there are at most 64 active variables and each residual clause has
at most 16 literals; otherwise it **falls back** to frequency and
records `pivot_fallbacks`. This prevents a research heuristic from
silently allocating a matrix quadratic in a large CNF variable count.
Its bound and the short-clause integer formula are fixed research
design choices, not tuned results.

The original `solve_with_limits` (including any other default entry
point) always receives Frequency when the feature is enabled, and
normal builds contain no alternative pivot dispatch. No changes
are made to cache-key identity, unit propagation, connected-component
splitting, Gaussian-rational cancellation, or budget error semantics.
`pivot_decisions` and `pivot_fallbacks` exist only with the feature.

## Two independent controls

The benchmark workspace enables the already existing
`cache-admission-research` feature **only to reuse its source-pinned
CNF fixtures and independent oracles**. The branching runner never
calls a threshold or work-proxy admission API. Cache policy is fixed
to exactly two states on every identical CNF:

- `off`: `cache_entries = 0`, no Key construction or cache hits.
- `legacy`: the existing default `Limits` Cache (entry cap 10,000,
  no payload, minimum-clause or observed-work filtering).

Four connected CNF graph families without an artificial duplicate
selector are tested: `ring`, `chorded`, `mixed` (2/3-CNF with
mixed literal polarity), and a separately declared `skewed` family
with a high-degree vertex, nonuniform binary/ternary clauses and
mixed literal polarity. Sizes `n=8,10,12` are fixed.

Expected SAT and exact Complex Full WMC come from
**independent exhaustive Boolean assignment evaluation** and Gaussian
integer weighted cardinality sums, not the solver's DPLL, cache,
component splitter or numerical output. The three earlier fixture
families reuse their already independently certified oracles.
The new skewed family has a separate literal-level exhaustive oracle.
Every policy is checked against the same exact count and SAT verdict.
A zero-node-budget negative test must keep the same error. The
Frequency research entry point is additionally tested for *identical
node, hit, decomposition and count behavior* to the normal solver.

## Source-locked measurement

Run on Linux:

```sh
cargo test --workspace --features branching-research
cargo test --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --features branching-research
cargo run --locked --release --manifest-path benchmarks/rust_core/Cargo.toml \
  --features branching-research --example cache_profile -- --branch-suite
```

The new `.github/workflows/branching-research.yml` executes the
matrix 4 families × 3 sizes × 2 cache settings × 3 pivot strategies ×
3 fresh trial workers = **216 isolated records**, rotating policy
order across trials. It writes source-locked
`branching-profile.csv` and `branching-guards.log`, with 30-day
artifact retention. Each worker is constrained by an independent
parent five-second deadline and Linux `RLIMIT_AS=512 MiB`
**virtual address space** (not a physical RSS limit). Memory-denial
and timeout negative controls must pass. All successful rows
are semantically certified against the independent exact Full WMC
oracle before emitting a performance record.

CSV fields distinguish nodes, Cache hits and entries, component
decompositions, pivot decisions/fallbacks, Linux process peak
`VmHWM` and in-child WallTime. MinFill computes a primal graph
separately at each pivot and may cost more than its search-tree
savings; these extra instructions are part of the measured end-to-end
solve time, not normalized away.

## Interpretive limits

These are reproducible **synthetic** CNFs, not benchmark evidence for
real SAT-competition instances, arbitrary QuPRS-generated CNFs or
large-scale weighted-model-counting workloads. Three isolated child
workers per case, on one CI runner, are insufficient to establish
statistically robust performance improvements or hardware portability.
The 37 existing CodSpeed Simulation benchmark identities must remain
unchanged. Simulated CPU cycles are distinct from these physical
WallTime and VmHWM observations; cross-CPU CodSpeed comparisons must
explicitly acknowledge runner model differences.

A candidate research pivot is not a production recommendation until
held-out irregular CNFs with external provenance and repeatable exact
oracle correctness are demonstrated, together with controlled wall,
memory and timeout results. For now, normal **Frequency** pivot and
Legacy Cache remain unchanged.


## First source-scoped 216-case observations

The first complete cohort was measured on source SHA
`4c28137b4034df6eaeee267ef0e6964df8987753`:
[Linux branching qualification #43192742484](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43192742484).
**All 216 distinct source-locked worker records passed** the
independent exact Complex Full WMC/SAT oracle and the timeout and
memory-denial negative controls. This is one Linux CI runner, with
three fresh children per case and policy. Each median describes that
runner only; it is not a cross-platform or statistical claim.

Median actual solve WallTime in milliseconds (all 12-variable cases):

| Family | Cache | Frequency | ShortClause | MinFill |
| --- | --- | ---: | ---: | ---: |
| Ring | OFF | 0.320 | 0.297 | 1.189 |
| Ring | Legacy | 0.274 | 0.278 | 0.325 |
| Chorded | OFF | 0.248 | 0.252 | 0.760 |
| Chorded | Legacy | 0.290 | 0.294 | 0.635 |
| Mixed | OFF | 0.289 | 0.274 | 0.489 |
| Mixed | Legacy | 0.336 | 0.325 | 0.485 |
| Skewed | OFF | 0.063 | 0.062 | 0.142 |
| Skewed | Legacy | 0.095 | 0.075 | 0.170 |

Deterministic search nodes for the same cases:

| Family | Cache | Frequency | ShortClause | MinFill |
| --- | --- | ---: | ---: | ---: |
| Ring | OFF | 65 | 65 | 397 |
| Ring | Legacy | 51 | 51 | 67 |
| Chorded | OFF | 49 | 49 | 223 |
| Chorded | Legacy | 49 | 49 | 141 |
| Mixed | OFF | 56 | 51 | 131 |
| Mixed | Legacy | 56 | 51 | 97 |
| Skewed | OFF | 9 | 9 | 19 |
| Skewed | Legacy | 9 | 9 | 19 |

MinFill frequently increases both graph-building overhead and
search size. Ring-12 OFF visits **397 rather than 65 nodes**;
Chorded-12 OFF visits **223 rather than 49 nodes**. Although it
can produce more cache hits in some cases (Chorded-12 Legacy:
49 vs Frequency's 6), this does not imply fewer search nodes.
ShortClause has a modest positive structural effect in Mixed-12
(56 → 51 nodes) but does not consistently improve all tested
families or Cache modes; the 0.01 ms scale differences are not
statistically resolved on three same-runner observations.
There are no demonstrated significant RSS savings: all workers have
small, mixed `VmHWM` variations relative to the process baseline.

All cases used unchanged `Limits`, the original unit propagation,
exact rational arithmetic, component splitting, and either Cache OFF
or unmodified Legacy admission. No case used structural thresholds or
observed-work admission. Values are *in-child Linux WallTime*, not
CodSpeed Simulation or QuPRS solver end-to-end time. Complete per-trial
records are in the run's `branching-profile.csv` artifact; this table
does not conceal the poor cases.

**Decision:** retain Frequency as the default. ShortClause and
bounded MinFill remain strictly opt-in research controls, pending an
independent real-world irregular CNF corpus and further scientific
investigation. This run does not support promoting MinFill even on
these small synthetic graph families.
