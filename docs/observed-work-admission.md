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
