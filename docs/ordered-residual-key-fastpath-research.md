# Strictly ordered residual key fast path — source-exact WMC research

## Empirical motivation

The independently source-pinned [five-arm Feynman stage-profile run #44250824600](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44250824600)
completed all six ordinary source × equivalent-precision cohorts with
102 Rust unit tests, SAT and both signed-complex MPFR result fields matched
pinned original GPMC C++. HWB6 100-bit same-run **unprofiled** full process
p50 was GPMC 177.418 ms, original Rust Inline 513.266 ms and prior
Scoped DSU 511.194 ms, retaining all Grover5 Rust censors.
The opt-in profiled original Inline attributed (median stage clocks):
canonical packed key 239.303 ms, component extraction 62.558 ms,
unit closure 34.977 ms, frequency/free-weight handling 31.739 ms,
exact cache hash/lookup 28.917 ms. The instrumented coverage was ~76.8%
of profiler solve time, and the per-trial matched profiler overhead
median was ~10.5 ms. These **are not uninstrumented flamegraph CPU shares**.

This prioritizes exact key work over another blind DSU improvement.
In particular key construction includes both full source-clause
filtering and sorting, so a sort-only shortcut may still fail to
reduce complete WallTime. This is a falsifiable hypothesis, not
a claimed speedup.

## Narrow algorithm and proof obligation

Current original reversible source Trail materializes the current
residual into a flat signed-literal arena and `View {cid,start,end}`
spans. Its original key canonicalization always performs:

1. unstable lexicographic sort of all source-residual views by their
   *entire* reduced signed-literal slices;
2. adjacent equal-slice deduplication;
3. exact canonical residual serialization to `PackedKey`.

New opt-in `--trail-ordered-key-fastpath` checks all neighboring
residual views for **strict** lexicographic inequality
`slice[i] < slice[i+1]` before either operation. Only when the
whole nontrivial input is strictly increasing can both sort and
dedup be skipped. Otherwise, **run the original sort and dedup
unchanged**, including equality. The variable order, signed literal
polarity and complete slice comparison are identical to the
original comparator; no key hashing or weights change.

**Source-exact argument:** strict adjacent order implies total
strict order by transitivity and no equal elements. Sorting an
already strictly increasing sequence yields the identical sequence,
and deduplication is a no-op. The final `Vec<cid>` and packed signed
literal key are therefore byte-for-byte identical, not merely
equisatisfiable. If strict order fails, the old program executes
exactly its original operations; an equal projected clause may be
reduced from distinct original source IDs and therefore cannot
be silently deduplicated by a new rule. This shortcut does not
modify original clauses, weights, Boolean SAT status or any DPLL
assignment, rollback, branching, component or exact cache semantics.

Count nontrivial admission probes and completed strictly ordered
shortcuts per solve; a high hit count alone does not prove a
WallTime improvement. This is a small microarchitecture option,
not a replacement for weighted AND/OR or GPMC tree decomposition.

## Independent acceptance

- Rust 100/333-bit signed-complex exact small original CNF exhaustive
  (2^n) assignment oracles; SAT, both signed Complex MPFR components,
  cache capacities 0/1/128, and DPLL/cache/decomposition search
  topology match original. Explicit root ordered witness, one-branch
  inversion witness, duplicate projected clause witness, UNSAT,
  and SAT-with-zero-weight witness. Full original Trail rollback.
- Source-pinned [five-arm native paired CI](https://github.com/PhysicsQoo/RaWMoCS/actions/workflows/cnf-ordered-residual-key-paired.yml):
  original unmodified GPMC C++, unchanged original Rust Inline, opt-in
  ordered key, profiled original Inline, and profiled ordered key.
  Same original QuPRS-emitted weighted CNF bytes and signed weights;
  30/100 decimal precision on GPMC versus 100/333 MPFR bits on Rust;
  seven order-rotated fresh child processes per ordinary Feynman
  source/precision cohort, 30s and 2GiB per child, observed RSS and
  p50/p90 **complete process** WallTime. Keep all Grover5 censored
  first-rotation evidence. Reject any SAT or either real/imag WMC
  mismatch and any DPLL/cache/decomposition topology drift.
- Report stage clock coverage, native profiler overhead and
  sorted-key probes/hits separately. Do not compute an invalid
  full-process time by summing stage p50 fields.
- Run cargo fmt, strict Clippy and Python stage-oracle preflight.
  The independent MQT original-QASM source holdout (seven declared
  circuits) is required before any generalized router claim.

## Go / No-Go

A shortcut is eligible only with exact SAT+WMC parity, no change in
cache-equivalence semantics, and a repeatable complete native
HWB6 p50 **and** p90 WallTime benefit without material RSS regression.
Also require an independent MQT holdout to establish out-of-family
generalization before production. If hit rate is low or incremental
strict-order scanning costs more than original sort, record a
**No-Go**, leave the fastpath research-only and target residual
materialization/weight-class normalization instead. Compare original
GPMC in the **same run**; earlier absolute runner times are not
directly interchangeable.

This work remains Draft and opt-in. Production RaWMoCS, original
QuPRS source/exporter, existing benchmark identities and pinned
GPMC original C++ are unchanged.
