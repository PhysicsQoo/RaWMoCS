# Inline exact weighted Component Cache keys

**Isolated research, not a production/default change.**

PR #97 proves count-safe reuse of same-source weighted components under
order-preserving variable renaming and exact four-part signed MPFR
weights; PR #98 finds the normalization overhead can erase the
reduction in searched DPLL nodes. This experiment isolates the
**second literal traversal** spent converting an already materialized
source-ID residual key to local weighted-component variable IDs.

## Hypothesis and precise scope

With `--trail-inline-weight-relabel` (only permitted on the
existing fused, exact prehashed, compact, parent-ID, weighted-class
trail), precompute the mapping from every source variable in the
current scope to its positive 1-based local rank, and compact each
surviving literal directly into this local rank during the existing
single-pass `canonical_compact` source-clause loop. The array of
ordered exact four-part signed weight classes becomes the component
cache's `scope` key. The old two-pass
`relabel_weighted_key` path is unchanged and is a direct A/B
control. The 0 local-key namespace delimiter for limited-scope
cache admission remains unchanged.

The source variable map is *monotone increasing* in the sorted
component scope. Therefore preserving literal sign and mapping to
local rank retains lexicographic ordering of each reduced clause
and the canonical lexicographic ordering of clause lists.
No scalar weight, polarity, numerical precision, Boolean
satisfiability, zero-weight distinction, source CNF, DPLL pivot,
cache hash/equality policy or component decomposition changes.

This is an optimization to **cost per node**, not a claim of an
additional reduction in nodes. An exact mismatch in SAT, both
complex MPFR WMC components, stdout SHA, DPLL nodes, cache hits,
decompositions or scope-key admission counts relative to the
independent weighted-class-cache control fails the hypothesis.

## Reproducibility and gate

- Exact 100-/333-bit independent signed Complex WMC small fixtures;
  both unrestricted and thresholds 1/2/16, with trail restoration.
  Preserve existing broader randomized dyadic, SAT/UNSAT and zero
  checks inherited from the parent.
- Original QuPRS emitter pinned to
  `0b28736651b3740ed3b201edd2efdef3da541e0d`;
  original C++ GPMC pinned to
  `89bbc89d0de918e7a5385740355df55765039230`.
  Never edit or re-encode the generated Weighted CNF between arms.
- 13 arms (old GPMC, original Rust, equivalent literal, Trail,
  Parent-ID, Compact, Prehash, Fused, exact weighted cache,
  16/64-scoped weight cache, and inline unrestricted/16-scoped
  variants). Seven rotated fresh native subprocess runs for the
  three completed Feynman source CNFs at GPMC 30/100 decimal
  digits ↔ Rust 100/333 bits. Complete p50/p90 WallTime including
  startup/parse/solve/output, observed RSS, SAT and both
  complex WMC, full stdout SHA and search metrics.
- Hard Grover5 retains its censored status and original GPMC
  completed control; never claim a partial timed speedup.
- Six distinct original-source CodSpeed Simulation benchmarks,
  preserving previous benchmark identities. Simulation is only
  diagnostic; a native WallTime/peak RSS regression disqualifies
  default adoption. Do not merge this stacked Draft without
  a repeatable long-case benefit.
