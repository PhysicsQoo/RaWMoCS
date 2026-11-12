# Observed-work cache admission: preregistered candidate and selector-free holdout

This stage is an **opt-in research control** under the existing
`cache-admission-research` feature. It adds
`solve_with_observed_work_ratio(problem, limits, ratio)` without changing
normal `solve`, `solve_with_limits`, default Cache policy, public stable
crate behavior, or CodSpeed benchmark identities. Do not enable this
policy by default on the basis of these synthetic fixtures.

## Candidate: actual first-solve effort, not predicted reuse

The existing `Engine::count` records its cumulative node counter on
entry and, when a residual has first been solved, observes the
subtree's visited nodes. In `Engine::save`, before retained cache
allocation, a research-only admission filter estimates **logical**
retained key-and-value payload items:

```text
logical_units = scope_variables + normalized_clauses + key_literals
                + ceil(exact_gaussian_result_bits / 64)
work_units = first_solve_visited_nodes * 8
admit iff first_solve_visited_nodes > 1
          and work_units >= logical_units * ratio
```

The factor `8` and experimental `ratio` values `1` and `4` are
fixed hypotheses, **not calibrated nanoseconds, memory bytes or a
statistical prediction model**. Multiplications use saturating
arithmetic; negative/zero ratios are invalid. Value bits measure exact
rational numerator/denominator lengths, not memory allocation. The
observed node count may include descendants and cached-result lookups;
it does **not** infer future reuse probability. No eviction or global
shared cache is added. Rejecting insertion leaves normal key
construction and HashMap lookup costs in place, so savings are not
guaranteed.

`cache_work_proxy_rejections` counts rejected insertion candidates
separately from `cache_min_clause_rejections` and logical-payload
quota rejections. A disabled cache must report zero for all. Cache
key identity (normalization, scope, current invocation's fixed exact
weights) is unchanged. The CLI and Rust library are scoped to Full
Complex WMC, not projected WMC or a QuPRS end-to-end circuit workload.

## Dataset separation

Previous guarded `cycle/rungs/spokes` CNFs deliberately used a
duplicated-selector root. They remain a **discovery/positive-control**
family to validate cache reuse and the work proxy.

The new **selector-free** holdout is separate, generated deterministically
without a duplicated selector:

| Family | Structure |
| --- | --- |
| `ring` | all-positive binary ring clauses |
| `chorded` | ring plus opposite-vertex binary chords |
| `mixed` | chorded ring plus mixed-sign 3-literal clauses |

Each has 8, 10 or 12 variables with equal exact Gaussian weights
`positive=2+i`, `negative=1-i`. An independent, exhaustive
Boolean-assignment oracle checks all clauses directly and counts
satisfying assignments by Hamming weight, then sums the weighted
Gaussian-integer polynomial. **It does not reuse DPLL, normalization,
component decomposition or the memo table**. The observed Work
Proxy cannot influence this semantic oracle.

The fixed policy matrix is `off / legacy / proxy1 / proxy4`, with
three fresh Linux child workers per family/size/policy and rotated
policy order: **3×3×4×3 = 108 workers**. Each worker checks SAT and
exact WMC before emitting its row. CSV records nodes, hits, retained
entries, proxy rejections, logical payload, wall-clock solve duration
and process peak `VmHWM`. The parent enforces a 5-second deadline
and Linux `RLIMIT_AS=512 MiB` **virtual address space** (not a
physical RSS cap). The standalone suite reruns both timeout and
allocation-denial negative controls. Failed or timed-out workers
produce no certified WMC.

Reproduce on Linux:

```sh
cargo test --workspace --features cache-admission-research
cargo test --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --features cache-admission-research
cargo run --locked --release --manifest-path benchmarks/rust_core/Cargo.toml \
  --features cache-admission-research --example cache_profile -- --work-suite
```

The new `.github/workflows/cache-work.yml` uploads
`work-proxy.csv` and `work-guards.log`. The earlier 108-row guarded
structural-admission study and all legacy suites stay independent.
Three samples on one CI runner are not a statistical finding. Use
the resulting **actual** cross-family medians and search nodes to
determine whether the approach merits a further calibrated discovery
and out-of-sample corpus. There is no inference of hard RSS savings,
general speedups, or performance on QuPRS CNFs.

## Decision criteria

A hypothesis is not promoted merely for retaining fewer keys.
Inspect net execution cost **including** key construction, lookups,
exact-rational value clone/insert and memory pressure, plus the
reduced recomputation on each specific family. Require several
non-synthetic, independently oracled holdouts, stable controlled
hardware comparisons, and combined fast/slow cases before considering
any production change. Keep QOO-207 branching-policy variation outside
this experiment, with the same frequency heuristic throughout.


## First independent selector-free cohort (preliminary)

[Feature-gated Linux study #43189151826](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43189151826)
at exact feature source SHA `a23d83826466444ca002f79cfb7d97c55e60e373`
completed the source-locked **108/108** worker cases. Each row was
accepted only after an independent exact Gaussian-integer Full WMC/SAT
oracle; the source-locked timeout and memory-denial negative controls
reported success. The CSV and control logs are saved in the GitHub
Actions artifact. This cohort uses only **one** Linux GitHub-hosted
runner, three fresh workers per policy/case. The results are
exploratory and are not a statistically significant speedup claim.

Median in-child solve WallTime (ms):

| Selector-free CNF | Variables | OFF | Legacy | Proxy1 | Proxy4 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Ring | 8 | 0.090 | 0.108 | 0.101 | 0.095 |
| Ring | 10 | 0.172 | 0.179 | 0.183 | 0.172 |
| Ring | 12 | 0.293 | 0.283 | 0.281 | 0.309 |
| Chorded | 8 | 0.089 | 0.108 | 0.098 | 0.095 |
| Chorded | 10 | 0.155 | 0.188 | 0.178 | 0.178 |
| Chorded | 12 | 0.266 | 0.291 | 0.280 | 0.263 |
| Mixed | 8 | 0.105 | 0.125 | 0.119 | 0.113 |
| Mixed | 10 | 0.142 | 0.168 | 0.156 | 0.153 |
| Mixed | 12 | 0.285 | 0.329 | 0.338 | 0.312 |

At 12 variables, deterministic node counts, hits and retained entries
were as follows:

| CNF | Policy | Nodes | Hits | Entries |
| --- | --- | ---: | ---: | ---: |
| Ring | OFF | 65 | 0 | 0 |
| Ring | Legacy | 51 | 16 | 35 |
| Ring | Proxy1 | 51 | 6 | 24 |
| Ring | Proxy4 | 65 | 0 | 4 |
| Chorded | OFF | 49 | 0 | 0 |
| Chorded | Legacy | 49 | 6 | 43 |
| Chorded | Proxy1 | 49 | 0 | 21 |
| Chorded | Proxy4 | 49 | 0 | 3 |
| Mixed | OFF | 56 | 0 | 0 |
| Mixed | Legacy | 56 | 6 | 50 |
| Mixed | Proxy1 | 56 | 0 | 24 |
| Mixed | Proxy4 | 56 | 0 | 8 |

**Negative results matter:** Chorded-12 and Mixed-12 demonstrate that
the default Cache can report six hits **without reducing visited
nodes**, while storing many keys. In Mixed-12 the median WallTime of
Cache OFF is 0.285 ms, versus 0.329 ms Legacy and 0.312 ms Proxy4.
Filtering obvious cheap entries helps somewhat but is insufficient.
Conversely, Ring-12 has real node savings under Legacy/Proxy1
(65→51), while Proxy4 is too restrictive and loses the savings.
A single fixed work-to-payload ratio does **not** generalize across
these selector-free families.

Linux process `VmHWM` medians vary by tens of KiB, with no
consistent physical-memory saving from either proxy. Neither retained
logical items nor `RLIMIT_AS` constitutes an RSS hard limit.
Values reflect the same runner and only three trials, so avoid
ranking near-equal runtimes as significant. No real-world QuPRS
data were included.

The experiment supports collecting better calibrated evidence for
**future reuse probability**, incremental recomputation, CNF
structure, exact-value copy costs and physical memory footprint,
rather than promoting a fixed threshold or an arbitrary numerical
factor. Keep Legacy as the normal default and retain both proxy
thresholds as research-only controls pending external holdouts.
