# Opt-in cache-transaction cost attribution

This is an experimental **Full Complex WMC** profiling instrument, not a
production cache optimization or a formal wall-time estimator.

## Scope and build isolation

The `cache-cost-profile` Cargo feature is **disabled by default** in both
`rawmocs-core` and the benchmark workspace. Its timer and aggregate counters
are therefore absent in the normal solver build. The default Rust core API,
counting semantics, cache admission and all CodSpeed benchmark identities are
unchanged. This research must not be used to infer projected WMC correctness
or QuPRS solver integration.

To run the instrumented source-pinned Linux worker cohort:

```sh
cargo test --workspace --features cache-cost-profile
cargo test --locked --manifest-path benchmarks/rust_core/Cargo.toml --features cache-cost-profile
cargo run --locked --release --manifest-path benchmarks/rust_core/Cargo.toml \
  --features cache-cost-profile --example cache_profile -- --cost-suite
```

The corresponding `.github/workflows/cache-cost.yml` stores its 27-row CSV
and negative-control logs for 30 days. Every row is a fresh Linux child with
a **5-second parent deadline**, a **512 MiB RLIMIT_AS virtual-address-space
restriction (not a physical RSS cap)** and an independent Gaussian-rational
transfer-DP exact Full WMC + SAT check. The standalone suite requires its
memory-denial and timeout controls to pass before producing rows.

## Counter boundaries

For each solver call the feature records the total time spent in five
disjoint phases, using `std::time::Instant`. Values are nanoseconds
**in the instrumented binary only**, not the corresponding costs in the
production binary.

| Counter | Measurement boundary |
| --- | --- |
| `key_build_ns` | Cloning the already-normalized residual clauses and variable scope into `Key`; excludes `normalize()` |
| `lookup_ns` | `HashMap::get` (hashing and key equality); excludes the hit-result clone |
| `hit_clone_ns` | Clone of the `Outcome` returned by a cache hit |
| `admission_ns` | Cache payload accounting and optional quota check, including declined admissions; excludes insert |
| `insert_ns` | `HashMap::insert` including `Outcome` clone and possible bucket allocations; excludes subsequent retained-item statistics |

The CSV also has per-phase call counts, total solver elapsed nanoseconds,
physical peak `VmHWM` and conventional nodes/hits/entries/rejections.
Nothing is an allocation-byte accounting or a byte-precise RSS cap.
The current cache does not implement eviction, so no eviction savings can
be inferred. Cache keys are scoped to one invocation with immutable weights;
this experiment does **not** license cross-invocation, precision- or
projection-context cache sharing.

## Experimental design

The fixture is the identical guarded connected-cycle formula already
qualified in `expensive_reuse.rs` by an independent exact oracle.
Use 6, 10, and 14 cycle vertices (7, 11 and 15 variables) and three
independent child workers for each of these policies:

- `off`: no retained entries and **no cache key build**; residual CNF
  normalization still occurs and is shared by all policies.
- `on`: existing entry-count cache with no logical-payload quota.
- `tight`: same cache lookup logic with a deliberately restrictive
  retained logical payload budget, so admission rejection can be observed.

Each trial rotates policy order; there are 27 source-pinned records.
The enabled instrumentation adds timer overhead and may perturb the
allocator and branch timing; do **not** subtract per-phase sums from an
uninstrumented total and call the difference precise recomputation time.
Report median observations separately and compare the instrumented cohort
against the **separate** uninstrumented `--cycle-suite` as a qualitative
measurement-overhead sanity check only.

## Research admission gate

Do not modify default cache admission merely because an instrumented
counter, a cache-hit rate, or a single measured worker run improves.
A candidate threshold requires separate out-of-sample connected/irregular
CNFs, independently checked exact algebraic counts, same-source cache
ON/OFF/threshold studies, and repeatable physical wall-time/RSS results.
The CodSpeed Simulation results measure modeled CPU instruction costs,
not Linux wall-clock or peak RSS. Preserve the existing benchmark IDs and
report runner/CPU changes before quoting cross-run deltas.


## First source-qualified feature-gated observation

[Isolated Linux qualification #43184733187](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43184733187)
at exact instrumented source HEAD
`8f0ac8408155b26d5b4f301e9e0307a486d60d99`
passed all **27 exact Full Complex WMC/SAT worker rows**, all transaction
accounting invariants, and both independently executed negative controls:
`GUARD_MEMORY_DENIED=PASS STATUS=RESOURCE_FAILURE` and
`GUARD_TIMEOUT=PASS STATUS=TIMEOUT`. This is **one** GitHub-hosted
Linux runner, three fresh child processes per case, not a cross-runner
statistical performance certification.

Median measured values (nanoseconds) on the **instrumented** solver:

| Cycle vertices | Policy | Solve WallTime ns | Nodes | Cache hits | Cache entries | Quota rejects | Key build ns | Lookup ns | Admission ns | Insert ns |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 6 | off | 96,736 | 23 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 6 | on | 76,095 | 13 | 1 | 12 | 0 | 2,075 | 1,132 | 490 | 7,511 |
| 6 | tight | 108,073 | 23 | 1 | 1 | 21 | 2,484 | 1,904 | 780 | 661 |
| 10 | off | 344,772 | 75 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 10 | on | 206,292 | 37 | 9 | 28 | 0 | 7,930 | 3,332 | 1,143 | 10,746 |
| 10 | tight | 355,908 | 75 | 3 | 1 | 71 | 9,001 | 5,957 | 2,455 | 651 |
| 14 | off | 1,041,015 | 231 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 14 | on | 420,827 | 69 | 25 | 44 | 0 | 14,194 | 7,446 | 1,693 | 23,855 |
| 14 | tight | 1,100,225 | 231 | 13 | 1 | 217 | 24,052 | 17,542 | 6,650 | 2,364 |

At 14 vertices, the remaining hit-result clone median was approximately
950 ns for `on` and 490 ns for `tight`. The `on` case has 25 hits and
visits 69 nodes instead of 231. However, `tight` still reports **13
hits without reducing node count at all**, and its median measured
WallTime exceeds `off`; a cache hit is therefore **not** evidence of
saved expensive search. Individual phase medians may come from different
trials and cannot be summed as an exact per-run breakdown. Timer calls
and instrumentation modify timings and code generation; do not
subtract measured phase totals to claim a precise algorithmic
recomputation cost.

The 14-vertex median Linux process `VmHWM` for
`off/on/tight` was **2444/2500/2460 KiB**. These small variations
do not demonstrate any physical RSS improvement. The `tight`
budget constrains *logical payload* rather than bytes or actual RSS.
No eviction, cross-invocation weighted cache or new default admission
policy has been implemented.

**Next scientific gate:** directly compare different connected CNF
families with varied reuse patterns, amortize instrumented timer
overhead and evaluate selective/adaptive admission as a separate,
independently checked policy experiment. Retain the present production
default until controlled same-hardware, same-source holdout evidence
supports a net benefit.
