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
