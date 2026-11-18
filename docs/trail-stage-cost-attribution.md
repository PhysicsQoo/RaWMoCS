# Source-exact MPFR Trail stage-cost attribution (research only)

## Motivation

The previous source-pinned Scoped DSU study reduced initialized DSU slots by 83.4%
on HWB6 but only reduced complete 100-bit p50 WallTime from 620.363 ms to
618.255 ms. Pinned original GPMC remained 230.392 ms. This excludes DSU
initialization as a *sufficient* explanation of the hard-case gap, but it
does not isolate all repeated canonical-key or clause-traversal costs.

Do not interpret a test passing, a smaller node count, or profiler clock totals
as proof of better production solve time.

## Independent hypothesis and instrumentation

An explicit, **off-by-default** `--trail-stage-profile` records cumulative
`Instant` wall nanoseconds in the original reversible weighted Trail
(`count_inner`), split into:

- `key_ns`: original source residual clause canonicalization, packed key
  construction, signed literal weight-class localization and normalizing
  scope where enabled; Boolean contradictions discovered here are included
- `lookup_ns`: cached hash construction plus exact equality-based cache
  lookup; prehashed hashes never substitute for key equality
- `unit_ns`: unit heap creation, propagation, exact signed-complex forced
  weights, reversible clause updates and contradicted paths
- `frequency_ns`: frequency scans, active/free variable ownership,
  corresponding exact free-weight factors and pivot scoring
- `components_ns`: original component DSU/extraction only

The timing scopes do not enclose recursive child calls or one another.
They do **not** cover all exact MPFR branch arithmetic, save/cache insertion,
assignment, rollback, construction, input normalization, source extraction
or full process startup. In particular, reported stage time is an
**instrumented lower bound**, not a partition of complete process WallTime.
No timing calls execute in default/unprofiled mode; the extra boolean
branches can still have overhead. Do not compare the profiled process
to the unprofiled process as though instrumentation were free.

The original CNF bytes, clauses, full positive and negative signed-complex
weights, SAT channel, canonical key semantics, MPFR precision, cache
capacity, search topology and default routing are unchanged. The profile
flag rejects incompatible experimental factor or post-unit paths.
No GPMC source changes.

## Source-locked native acceptance

`cnf-trail-stage-profile-paired.yml` uses the original pinned QuPRS exporter
and unmodified pinned GPMC C++ revision, at equivalent 30/100 decimal
digits (GPMC) and 100/333 MPFR bits (Rust), with 30 s and 2 GiB address
space per fresh child process. Independently record both the exact Boolean
SAT status and real **and** imaginary signed-complex WMC channels.

Five *same-run, same-CNF* arms:

1. Original GPMC C++ baseline
2. Current original unprofiled Rust Inline Trail
3. Same Rust Inline Trail with stage profiling
4. Prior correct source-exact Scoped DSU (unprofiled)
5. Same Scoped DSU with stage profiling

Seven order-rotated child processes for all six ordinary Feynman
source-by-precision cohorts (HWB6, Barenco5, ModMult55 × two precisions).
Keep Grover5 negative/censored records; never drop them from the artifact
or replace a timeout with an unbounded retry. Report complete process
p50/p90 WallTime and observed max RSS. Only the scoped versus inline
**unprofiled** arms may be treated as a standard walltime comparison.

The new per-arm hard gates are: 6/6 regular cohorts completed; equivalent
SAT and signed-complex counts versus original GPMC at matched precision;
identical DPLL nodes, cache hits and decompositions between four Rust
arms; profiler flag and nonnegative stage counters observed only in its
two target arms; unchanged source revision and input bytes. Separate
100/333-bit deterministic Rust oracle tests include cache sizes 0/1/128,
SAT, UNSAT and signed-complex cancellation.

A strict research Clippy gate is included to ensure previous independent
science workflows are not blocked by inherited code warnings. Earlier
workflow failures were Clippy-stage build failures; the current native
study is not accepted until the new **final-head** workflow and raw
artifacts have completed.

## Go / no-go decision

1. Attribute the measured stage buckets *within each profiling arm*.
   Inspect both frequencies (stage time as a share of profiled solver
   time) and the difference in instrumentation overhead versus its
   same-run unprofiled control. Verify stage sums are not interpreted
   as an exact exhaustive decomposition.
2. If one stage consistently dominates the HWB6 cohort, prototype **one
   change at a time**: active occurrence-adjacency BFS component discovery
   when components dominate, or exact key memo/materialization reduction
   when canonicalization or lookup dominates.
3. Preserve all old hard negatives and run an independently source-locked
   original MQT weighted-CNF holdout before routing.
4. A new solver variant is No-Go if it changes SAT, either signed-complex
   WMC channel, cache safety, reversible Trail semantics, or does not
   consistently reduce complete WallTime and RSS on hard sources versus
   the *same-run* original GPMC.

Research-only stacked Draft. No production RaWMoCS default, QuPRS
exporter, original GPMC implementation, or independent benchmark
identity is modified. Do not put private tracker references into
public documentation or PR metadata.
