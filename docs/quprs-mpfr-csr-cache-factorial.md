# CSR × bounded Cache capacity on original QuPRS Feynman WMC

## Scope and source lock

Research-only evaluation of the *combination* of flat CSR occurrence-indexed
unit propagation and bounded exact residual Cache. Neither the exact-rational
production `rawmocs-core` nor the published GPMC / QuPRS SolverAdapter
defaults is changed. Both optimizations must be explicitly enabled in the
isolated MPFR research executable.

- Pinned QuPRS Feynman exporter: `0b28736651b3740ed3b201edd2efdef3da541e0d`.
- Pinned unchanged GPMC C++: `89bbc89d0de918e7a5385740355df55765039230`.
- Original QASM and emitted complex weighted CNF files are source/SHA verified.
- Matching GPMC/Rust **100-bit or 333-bit binary MPFR mantissa precision**
  and four separately rounded complex products (not mixed arbitrary
  floating-point vs exact rational accuracy).
- All Rust variants must produce **byte-for-byte identical entire complex
  stdout**, consistent Boolean SAT and full-equivalence Cache key lookup;
  GPMC numerical guard is independently checked.
- **Seven arms:** GPMC C++; Indexed and CSR × 10k, 25k, 50k Cache entries.
  Seven rotating, independent native trials per arm, forming a complete
  balanced seven-arm order cycle on one CI Linux runner per experiment.
  30-second and 2-GiB native process bounds, fixed finite DPLL node caps.
- Within any fixed Cache size, Indexed and CSR must also reproduce
  identical DPLL nodes, Cache hits and component decompositions. Across
  Cache sizes, exact hits are allowed to change the visited search tree.
- Original Grover5 remains **censored at 1M nodes**; it has no valid
  completion-time rank. The qualified larger original CNFs are
  `SOLVER_AGREEMENT_ONLY`, not independent exhaustive Full WMC or
  original quantum phase fidelity certificates.

## Reproduced GitHub Actions evidence

- [First 7-arm factorial CI #43677957507](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43677957507):
  19/19 Rust MPFR tests and all six original source × precision cohorts PASS.
- [Independent 7-arm factorial CI #43678580760](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43678580760):
  same qualification PASS with further solve-only time, WallTime MAD and
  Pareto-frontier output. All other relevant Rust science, CodSpeed,
  source encoder, SATLIB and metadata workflows passed.
- The archived `original-feynman-long-source-scout` artifact contains
  `csr-cache-factorial-ab.json`: source hashes, original QASM/CNF,
  all individual native measurements, WallTime, RSS, per-arm search
  statistics, paired ratios, and censored outcomes.

### HWB6 within-run native WallTime medians (milliseconds)

Do **not** compare raw absolute runtimes across different CI machines.

| Run | Bits | GPMC | Indexed10k | CSR10k | Indexed25k | CSR25k | Indexed50k | CSR50k |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| First | 100 | 174.797 | 1134.787 | 1085.720 | 932.817 | 853.378 | 930.917 | **756.140** |
| First | 333 | 193.560 | 1230.870 | 1164.058 | 980.217 | 905.783 | **876.219** | 878.251 |
| Replication | 100 | 176.987 | 1143.100 | 1098.641 | 976.812 | 895.617 | 1020.408 | **788.415** |
| Replication | 333 | 195.113 | 1240.609 | 1170.381 | 1022.083 | 926.311 | **899.260** | 932.546 |

Cache capacity changes the *number of searched states*, and CSR
changes their *cost*. For both precisions, both source-locked runs,
and either propagator, HWB6 DPLL nodes are:

| Cache limit | DPLL nodes |
| --- | ---: |
| 10k | 273,928 |
| 25k | 151,245 |
| 50k | 94,209 |

Source-locked original holdouts Barenco Toffoli5 (4,325 nodes) and
Modular Multiplier55 (5,699 nodes) remain below the 8,192-node CSR
activation threshold. Small holdout WallTime differences should not
be marketed as general algorithmic gains.

### Exact-cache memory cost and precision interaction

HWB6 **CSR peak RSS**, KiB, by native `/usr/bin/time`:

| Run | Bits | CSR10k | CSR25k | CSR50k |
| --- | ---: | ---: | ---: | ---: |
| First | 100 | 71,692 | 157,900 | 226,048 |
| First | 333 | 72,464 | 159,724 | 228,756 |
| Replication | 100 | 71,708 | 157,956 | 226,024 |
| Replication | 333 | 72,468 | 159,740 | 228,784 |

The median per-trial paired **Indexed/CSR WallTime ratios** are:

| Run | Bits | 10k | 25k | 50k |
| --- | ---: | ---: | ---: | ---: |
| First | 100 | 1.0465 | 1.0931 | 1.2318 |
| First | 333 | 1.0575 | 1.0820 | 1.0021 |
| Replication | 100 | 1.0413 | 1.0923 | 1.2972 |
| Replication | 333 | 1.0570 | 1.0986 | 0.9596 |

**The effects are not generally additive or precision-invariant.**
CSR+25k improves native WallTime at both precisions in both studies.
CSR+50k is the fastest measured 100-bit HWB6 candidate, but at
333 bits it does not consistently beat Indexed+50k in full process
WallTime. In the 333-bit replication, internal solve-only time
improved from 827.937 ms (Indexed50k) to 742.445 ms (CSR50k),
**while complete process WallTime worsened** from 899.260 ms to
932.546 ms. The outside-solver overhead has not been causally
attributed. Full-process WallTime remains the primary performance
ranking; do not re-label this case a WallTime speedup based on
solve-only measurements.

At 25k, CSR consumes about 155–156 MiB peak RSS compared with
about 221–223 MiB at 50k. Although both configurations remain under
the bounded process limit, larger Cache capacity has a substantial
memory cost. GPMC C++ remains multiple times faster and generally
uses substantially less memory on original HWB6.

## Decision and reproduction

**Preserve both combinations as opt-in MPFR research, keep all defaults
unchanged.** The **balanced long-case candidate is CSR+25k**, whereas
**CSR+50k is a memory-heavier 100-bit HWB6 time candidate** and is not
qualified as a universal improvement at 333 bits. No automatic
precision-based mode selection is justified by a single long source
family.

Example 100-bit reproduction on the **original pinned** HWB6 CNF:

```bash
rawmocs-mpfr-research \
  --node-budget=1000000 \
  --cache-limit=25000 \
  --indexed-unit-prop-csr-after-8192 \
  --shared-residual-key \
  --fast-assign --hashbrown-borrowed \
  30 ORIGINAL_PINNED_Feynman_HWB6.cnf
```

Pass `100` instead of `30` for the corresponding 333-bit cohort.
The original source CNF and precision must not be silently replaced.

## Next optimization target

Prioritize a **packed, fully exact residual Cache key** or selective
admission policy to limit the high RSS cost of 25k/50k caches.
Then investigate persistent immutable clause storage and reversible
assignment trails to reduce per-DPLL-node copying. Each candidate
requires fresh original-source, equal-MPFR, complete SAT/WMC/search
and bounded native WallTime/RSS qualification. Never use CodSpeed
CPU Simulation as a substitute for the same-runner child-process
comparison against native GPMC C++.
