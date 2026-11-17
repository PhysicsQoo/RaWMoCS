# Projected exact weighted post-unit cache

**Research-only opt-in, Draft, stacked on the independent
Post-Unit Closure Cache arm. No production, QuPRS, standard MPFR
DPLL or original C++ GPMC change.**

## The now-qualified structural result and its cost

The [original QuPRS Weighted CNF 15-arm native experiment](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43995733251)
qualifies post-unit exact weighted component caching, with the
same original source GPMC executable and 7 rotated fresh
subprocesses at 100/333-bit MPFR:

| Input, 100 bits | Prior Inline full nodes | Post-unit nodes | Prior Inline full native p50 ms | Post-unit p50 ms |
| --- | ---: | ---: | ---: | ---: |
| HWB6 | 73,424 | **62,312** | **508.167** | 592.084 |
| Barenco5 | 1,740 | **1,480** | **8.507** | 9.890 |
| ModMult55 | 3,879 | **3,405** | **17.754** | 22.275 |

All 6 completed case×precision cohorts match original unmodified
GPMC for Boolean SAT and both Complex WMC parts. The original
Grover5 GPMC completes and all Rust arms remain node-censored.

The structural cache benefit is real (~12–15% fewer DPLL nodes),
**but the extra source-clause scan and canonical sorting at each
counted node more than erases the savings**. The direct
post-unit prototype is a qualified *performance negative*.

## New isolated hypothesis: project the already canonical pre-unit arena

The post-unit prototype first enumerates a canonical compact
residual (source clause IDs and flattened signed literals) to
seed the deterministic unit heap. After unit propagation it
re-enumerates the *same source clauses* to build a second, fully
canonical residual cache key. That duplicates all source-clause
scanning even on nodes that force no literals.

This opt-in `--trail-post-unit-projected` avoids the second
**original clause traversal**, without changing the search tree:

1. Preserve the first compact canonical `pre_lits` arena
   (explicit zero delimiters, sorted/deduplicated clauses).
2. After the same Boolean unit-closure, if this node forced
   **zero** variables, the pre-unit arena is already the exact
   post-unit residual. Reuse it unchanged; if the weighted local
   namespace is enabled, perform the existing exact local
   weight-class relabel pass once.
3. If the node forced variables, traverse the *already compact
   pre_lits buffer*, removing satisfied clauses and falsified
   literals, applying monotone local signed-variable ranks if
   appropriate. Sort and deduplicate the resulting reduced spans
   again: two previously different source residual clauses may
   become identical after assignment. Detect a live empty
   projected clause as Boolean UNSAT.
4. Produce precisely the same `PackedKey` scope and literal
   ordering as the old post-unit source-rescan implementation.
   Retain exact full complex signed MPFR weight classes, namespace
   sentinel and full hash collision equality.

The post-unit algebra is unchanged:
`W(F|α) = product(forced_literal_weights) × W(residual_after_UP)`.
The forced-unit complex prefix remains outside every cache
entry. Boolean `SAT` and numerical `WMC=0` are separate.
This experiment **does not claim any further search-node
reduction relative to Post-Unit Cache**; a different node count
or final MPFR output from the post-unit control is an automatic
failure.

## Correctness and benchmarking contracts

- Independently compare exact signed Complex WMC at 100 and
  333 MPFR bits, including zero weights, Boolean UNSAT,
  unit chains, disconnected components, positive projection
  examples and the pre-existing 40 randomized dyadic CNFs.
  Exercise cache disabled and capacities 1/256; all branch
  trails must restore.
- For each completed original QuPRS CNF trial require **exact
  stdout SHA, SAT, both complex numeric fields, DPLL nodes,
  cache hits, component decompositions, count of post-closure
  visited/hit/forced nodes** between old post-full and
  projected-full, and old post16 versus projected16.
- 17-arm, seven interleaved fresh process trials on
  unmodified QuPRS `0b28736651b3740ed3b201edd2efdef3da541e0d`
  emitted Weighted CNF SHA, original GPMC C++
  `89bbc89d0de918e7a5385740355df55765039230`.
  GPMC 30/100 decimal digits ↔ Rust MPFR 100/333 bits.
  Report complete process WallTime p50/p90, observed max RSS
  under 30s/2GiB address-space limit; keep full per-trial
  logs and Grover5 node-censored cases.
- Six new source-identical 100-bit CodSpeed Simulation
  benchmarks, retaining the previous 51 MPFR names and original
  formal-core benchmark identities. Simulation is diagnostic,
  never a surrogate for native WallTime.
- Accept for further work only if the additional source
  traversal is measurably reduced *and* long-source native
  WallTime p50/p90 and RSS pass without sacrificing complex
  WMC correctness. The projected path must remain opt-in
  until a separate full native repeat shows a robust benefit.
