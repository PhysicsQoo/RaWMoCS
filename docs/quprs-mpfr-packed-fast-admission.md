# Exact Packed Cache: borrowed-hash reuse and full-cache allocation bypass

## Research boundary

This is an **isolated MPFR research CLI** optimization. Production
`rawmocs-core`, the default Cache behavior, published GPMC C++, and
the QuPRS SolverAdapter are **not** changed. The previous
[lossless Packed residual Cache](quprs-mpfr-packed-exact-cache.md)
successfully reduced native memory use but sometimes increased
the Rust **solve-only** time; much of its one-shot CLI WallTime benefit
came from shorter Cache destruction rather than a faster DPLL.

The opt-in flag `--packed-residual-key-fast` preserves the existing
`--packed-residual-key` as its experimental control. Both flags use
the same complete canonical CNF literal contents, exact variable
scope, no lossy fingerprints, the same 8192-node CSR gate and the
same bounded 10k/25k/50k Cache policy.

## Algorithmic changes

1. **Reuse the hash of the complete borrowed residual at Cache
   insertion.** The old Packed Cache already hashes the original
   canonical residual on lookup, but independently rehashes the
   newly packed flat key when it is admitted. The new mode retains
   the lookup hash for an exact `hashbrown` raw-entry insertion.
   The insert **does not trust the hash as equality**: if recursion
   has inserted the same canonical key in the meantime, the raw
   entry is checked against the *complete* Packed key and updates
   the already occupied entry. Hash collisions alone cannot
   produce a Cache hit or overwrite.
2. **Avoid candidate allocations after bounded Cache fills.**
   Every residual still probes the Cache for valid hits. If the
   capacity is full and lookup is a miss, the new mode does not
   allocate a temporary `Arc<Vec<Clause>>`, clone the variable
   scope, or create a key that can never be admitted. Instead,
   it continues the exact same DPLL computation using its owned
   canonical residual and records a `packed_full_misses` counter.
   If capacity is available, the existing safe shared-residual
   ownership is retained until a possible exact admission.

The previous algorithm and its CLI flag are unchanged.
No Cache eviction, altered Search heuristic, approximate WMC or
different numerical operations were introduced. The code keeps
per-entry exact key equality despite the raw-entry hash reuse.

## Original source and accuracy contract

- Pinned unchanged QuPRS Feynman PathSum/WmcManager source:
  `0b28736651b3740ed3b201edd2efdef3da541e0d`.
- Pinned unchanged GPMC C++ source:
  `89bbc89d0de918e7a5385740355df55765039230`.
- Original committed Feynman QASM and its emitted complex
  weighted CNF are checked by SHA256; no transpilation.
- Both Rust research solver and GPMC C++ run at identical
  **100-bit / 333-bit MPFR mantissa precision**, preserving
  the four individually rounded complex multiplications.
- Seven native arms: GPMC C++ and original Packed/optimized Fast
  × exact Cache capacities 10k/25k/50k. Each arm receives
  **seven independently spawned native trials**, rotating across
  all seven execution positions on the **same CI runner**,
  for each source family and precision.
- **30-second and 2-GiB process caps**, 1M DPLL node budget on
  original HWB6; lower bounded caps on smaller source holdouts.
- Fail closed on changed source, SAT vs UNSAT, changed full
  complex WMC **stdout bytes**, disagreement with same-precision
  original GPMC C++ numerics, different DPLL nodes/Cache
  hits/component counts at a fixed capacity, missing solve/drop
  phase timings, or nondeterministic per-arm search.
- SAT with exactly zero complex WMC remains distinct from UNSAT.
  96 deterministic 100/333-bit generated CNF fixtures and
  0/1/10/10k/50k Cache limits test the fast-admission branch.
  Force-collision and exact-key tests are also present.
- Original Feynman Grover5 remains **censored** at the
  finite 1M-node cap and is not assigned any time rank.
  Original high-variable-source verdicts are
  `SOLVER_AGREEMENT_ONLY`, not independently certified
  exact Full WMC / quantum phase equivalence.

## First native qualification

[GitHub Actions #43685451755](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43685451755)
at head `88a9c86963aa6066946bc00d236cb2514d69a54d`
passed all workflows, including **24/24 isolated MPFR Rust
tests** and all six original source × precision scientific
cohorts. The workflow artifact
`original-feynman-long-source-scout/packed-fast-admission-ab.json`
contains complete source digests, raw trials and censored
outcomes, plus full-process WallTime, solve-only, RSS, teardown,
and per-arm exact search statistics.

### Original HWB6: matched native medians (milliseconds)

Every entry is a **within-run median of seven independent processes**
at the same original CNF and MPFR bits. The 10k/25k/50k
capacity influences the number of search nodes; comparing
Packed/Fast at the same capacity isolates the new hash/admission
mechanism.

| Bits | Cache | Packed solve | Fast solve | Packed WallTime | Fast WallTime |
| --- | ---: | ---: | ---: | ---: | ---: |
| 100 | 10k | 1306.298 | 1299.858 | 1318.123 | 1310.944 |
| 100 | 25k | 1044.896 | **1011.872** | 1063.360 | **1029.795** |
| 100 | 50k | 949.553 | **915.931** | 975.352 | **942.439** |
| 333 | 10k | 1424.565 | **1356.845** | 1436.257 | **1368.301** |
| 333 | 25k | 1108.896 | **1071.943** | 1126.678 | **1090.876** |
| 333 | 50k | 1006.266 | **971.467** | 1033.508 | **997.747** |

At HWB6 25k, the optimized mode reduces measured solve-only
time by about **3.2–3.3%**; at 50k by **3.5%** for both precision
groups. The full native WallTime moves in the same direction,
unlike the earlier isolated Packed key redesign. A single
cohort does not establish stable gains across all machines.

The 25k / 50k search trees are exactly **151,245 / 94,209
DPLL nodes**, with identical Cache hit and component split
counts across the Packed/Fast pair. Full complex MPFR output
is unchanged. HWB6 Fast25k peak RSS is
49,692 KiB (100 bits) / 51,364 KiB (333 bits); Fast50k
71,352 / 74,260 KiB. The memory advantage of the previous
lossless Packed key is retained.

For held-out original Barenco Toffoli5 and Modular
Multiplier55, the 100/333-bit solve-only time improved by
roughly **2–4%** across tested capacity settings. These
short cases do not reach the 8,192-node CSR gate, so this
observed improvement is consistent with lower Cache
hash/admission overhead, **not** an expanded CSR benefit.
The relative contribution of hash reuse and Cache-full
allocation bypass is not yet isolated and needs a
separate follow-up experiment.

### GPMC is still the native performance reference

At the same original HWB6 CNF and precision in this cohort,
the unchanged GPMC C++ process took **232.380 ms at 100-bit**
and **253.841 ms at 333-bit**. Even the fastest Fast50k
Rust mode takes **942.439 / 997.747 ms** in full native
WallTime, so the gap to GPMC remains substantial.
Do not compare CodSpeed simulation times with GPMC WallTime.

## Reproduction

Generate the identical original weighted CNF from the pinned
QuPRS exporter, then run the opt-in research executable:

```bash
rawmocs-mpfr-research \
  --node-budget=1000000 --cache-limit=50000 \
  --indexed-unit-prop-csr-after-8192 \
  --packed-residual-key-fast \
  --fast-assign --hashbrown-borrowed \
  30 ORIGINAL_PINNED_HWB6_WEIGHTED.cnf
```

Use `100` in place of `30` to reproduce the 333-bit
cohort. Compare against the identical CLI changing only
`--packed-residual-key-fast` to `--packed-residual-key`,
and against source-pinned GPMC C++ using matched precision.

## Scientific decision

Retain Fast Admission as an **opt-in research candidate**.
This first qualification establishes a real solve-only
improvement at fixed MPFR precision on the original Feynman
source cohorts; it does not justify a production default
change. Repeat an independent native cohort before
promoting any configuration further. The subsequent
research priority is separate-factor attribution for
hash reuse versus cache-full allocations, and then
persistent original clause storage / reversible assignment
trails to reduce the much larger remaining GPMC gap.
