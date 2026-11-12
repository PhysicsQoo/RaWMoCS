# Exact WMC cache payload accounting

RaWMoCS uses a bounded-entry `HashMap<Key, Outcome>` cache. An entry limit is
**not** a byte limit. During a completed solve, the following newly reported
counters describe **retained logical payloads**:

| `Report.stats` field | Meaning |
| --- | --- |
| `cache_entries` | Number of retained memoization entries |
| `cache_scope_items` | Sum of the lengths of stored `Key.scope` vectors |
| `cache_clause_items` | Total count of stored `Key.clauses` vectors |
| `cache_literal_items` | Sum of literal counts in the stored clause vectors |
| `cache_value_bits` | Sum of numerator and denominator significant bit widths in cached real and imaginary Gaussian values |
| `max_cached_value_bits` | Maximum single cached Gaussian value bit width |

These are exact counts for the **stored logical content** at successful
insertions (with saturating counters if integers overflow). For
`cache_entries=0` they stay zero, including in correctly solved nontrivial
CNF examples. They enable controlled comparisons of key widths and
rational-number growth across cache policies.

**These counters do not measure heap allocation or RSS.** They intentionally
exclude `Vec` capacity, HashMap buckets, struct fields, allocator overhead,
temporary residual clauses, recursion stack, and the original input CNF.
`cache_value_bits` measures *significant* numeric bits, which may be smaller
than the underlying limb allocation. It is not an RSS estimate, byte budget
or hard memory guarantee.

## Measurement procedure

Use fixed input CNFs and the same source revision, comparing cache caps of
0, 1, 8 and the default. Record exact SAT and Full WMC parity, stored key
scope/literal counts, numeric bit widths, nodes, decompositions, and cache
hits. The independently derived 16/32/64-variable chain and pair families
are available in `benchmarks/rust_core/src/scalability.rs`.

CodSpeed measures CPU Simulation, **not heap bytes**. When adding these
counters to `save`, compare all original benchmark identities against the
frozen `main` baseline. Do not infer memory improvement from CPU data.

Next separate stages: characterize actual process peak RSS using an
isolated worker, define a cache byte-admission policy with precisely
specified accounting, then implement process-level memory/time cancellation
that fails closed. A `HashMap` payload counter by itself cannot isolate an
out-of-memory solver.


## Optional logical cache-admission quota

The opt-in API solve_with_cache_payload_budget(problem, limits, budget)
declines to store an otherwise valid memoization entry whenever any
cumulative retained logical payload would exceed a CachePayloadBudget:

- scope_items, clause_items and literal_items count elements stored in keys.
- value_bits counts numerator and denominator significant bits of stored
  exact Gaussian-rational counts.

The existing cache_entries limit and the logical payload quota both apply.
Rejected insertions increment stats.cache_payload_rejections but **do not**
fail a solve, round a value, return an unknown, or change SAT semantics. The
original solve() and solve_with_limits() continue to use their prior
entry-count-only behavior.

All admission checks use checked arithmetic and reject overflows. These
quotas are **logical items and significant bits**, not bytes, physical RSS,
HashMap allocation, transient rational normalization, a true OOM guard, or
an OS process limit. For OS resource-denial/timeout experiments see the
separate isolated Linux research profile; do not claim production safety
from cache admission alone.

An additive CodSpeed Simulation benchmark evaluates a 64-variable
independent-component Full WMC fixture under the quota; the original native
benchmark IDs and test fixtures are not modified.
