# Exact flattened residual Cache keys: original QuPRS Feynman study

## Research boundary

This document covers a deliberately opt-in, isolated **MPFR research**
representation of the exact residual Cache key. It is not a default
production `rawmocs-core` change, a modified QuPRS PathSum encoder,
or a new GPMC release.

The previous [CSR × Cache capacity factorial](quprs-mpfr-csr-cache-factorial.md)
showed that higher Cache capacity substantially reduces original HWB6
DPLL nodes, but a 25k/50k Cache retained very large nested vectors
and had high peak RSS.

The new `--packed-residual-key` stores complete canonical CNF
residuals in **one flat `Vec<i32>`** with a literal zero after
**every** clause, including empty clauses, plus the original exact
variable scope. A zero is never a DIMACS literal; clause boundaries,
original literal order, polarity and duplicates therefore remain
reconstructible without ambiguity. No lossy fingerprint is used.

- On lookup, hash the **original nested residual directly**, matching
  its original structural hash stream. The packed key implements that
  same hash stream by reconstructing clause slices (including per-clause
  length prefixes) from its separators.
- For a candidate hit, compare the **complete variable scope, every
  clause boundary and every literal**. A deliberately forced hash
  collision never confers equality.
- On a miss, retain the existing shared canonical residual temporarily
  while computing the answer; convert it to flat packed storage
  **only when the exact bounded Cache actually admits an entry**.
  No packed-key allocation occurs on lookup hits or after capacity fills.
- All previous Cache search behavior, CSR activation threshold and
  MPFR arithmetic are unchanged. The flat representation changes
  memory layout, not the counted weighted Boolean formula.
- Explicit `RESEARCH_MPFR_ENGINE_DROP_NS` measures Cache destruction
  separately from the Rust `solve_ns` counter and external native
  full-process WallTime.

The new option requires `--hashbrown-borrowed`. The original
`--shared-residual-key`, default 10k Cache and all production APIs
are preserved unchanged.

## Immutable source and accuracy admission

- Original QuPRS source commit:
  `0b28736651b3740ed3b201edd2efdef3da541e0d`.
- Original GPMC C++ source commit:
  `89bbc89d0de918e7a5385740355df55765039230`.
- Original source-emitted Feynman QASM and weighted CNF SHA256 checks;
  no QASM rewrite or transpilation.
- GPMC and research Rust both use **100/333-bit binary MPFR**
  with the same four-rounding-step complex multiplication model.
  Each Rust mode must match full native complex stdout byte for byte;
  the GPMC numeric comparator is a separate tolerance-limited check.
- Exactly distinguish **UNSAT**, **SAT with zero complex weight**,
  and nonzero SAT. Unit tests also test deliberately colliding hashes,
  incorrect scope, different clause partition/order, duplicate
  literal patterns, and 96 deterministic small CNF cases at both
  precisions and 1/10/10k/50k Cache limits.
- **Identical DPLL nodes, Cache hits and component splits** between
  Shared/Packed at every fixed capacity. Capacity itself may reduce
  nodes due to legitimate exact Cache hits.
- Original Feynman HWB6, Barenco Toffoli5 and ModMult55 qualified.
  Original Grover5 remains censored at 1M nodes and has **no
  completion-time rank**. The larger original source cohorts are
  `SOLVER_AGREEMENT_ONLY` rather than independently certified
  complete Full WMC/quantum phase results.

Each seven-arm cohort runs unchanged native GPMC C++ plus research
CSR propagation × {Shared, Packed} × Cache {10k,25k,50k},
**seven independently spawned processes per arm**, balanced across
all seven execution-order positions on the *same* GitHub Linux
runner. Individual processes have a **30-second / 2-GiB limit**.
Compare only within-run WallTime and RSS, never raw milliseconds
between different runner allocations.

## Two independently passed CI cohorts

- [Initial source-locked packed Cache run](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43679867793):
  full original Feynman cohort PASS, 22/22 MPFR Rust unit tests.
- [Independent repetition with optimized hash and explicit teardown clock](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43680892836):
  same source/accuracy/tree checks PASS, 22/22 Rust tests and all
  other repository research workflows PASS.
- Full per-trial JSON (including censored cases) is retained as
  `original-feynman-long-source-scout/csr-cache-factorial-ab.json`
  and `original-feynman-long-source-scout/packed-residual-cache-ab.json`
  in the corresponding CI workflow artifacts.

### HWB6 native full-process WallTime, milliseconds

These are the medians of seven independent native processes **within
each individual CI run**. The point is the within-run matched pair.

| Run | MPFR bits | GPMC | Shared25k | Packed25k | Shared50k | Packed50k |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Initial | 100 | 231.479 | 1116.451 | **1051.912** | 1019.326 | **970.235** |
| Initial | 333 | 256.589 | 1182.137 | **1136.070** | 1180.038 | **1035.447** |
| Repeat | 100 | 229.485 | 1088.890 | **1044.391** | 996.541 | **948.840** |
| Repeat | 333 | 254.192 | 1151.965 | **1112.285** | 1137.213 | **1013.241** |

Original HWB6 DPLL nodes, **unchanged Shared vs Packed** at
every capacity and precision:

| Cache entries | DPLL nodes |
| --- | ---: |
| 10,000 | 273,928 |
| 25,000 | 151,245 |
| 50,000 | 94,209 |

### HWB6 peak RSS, KiB

| Run | Bits | Shared25k | Packed25k | Saved | Shared50k | Packed50k | Saved |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Initial | 100 | 157,812 | 49,852 | 68.41% | 225,888 | 71,324 | 68.43% |
| Initial | 333 | 159,620 | 51,496 | 67.74% | 228,668 | 74,264 | 67.52% |
| Repeat | 100 | 157,820 | 49,800 | 68.45% | 225,880 | 71,344 | 68.42% |
| Repeat | 333 | 159,588 | 51,496 | 67.73% | 228,600 | 74,248 | 67.52% |

On the long HWB6 source, **50k Packed fits in approximately the
same native RSS as the previous 10k Shared Cache**, while retaining
the much smaller 50k search tree. This is a large memory
representation gain; it does *not* imply reaching GPMC WallTime.

The held-out original Barenco and ModMult source cohorts passed
unchanged scientific checks with roughly 38–39% and 44–45%
native RSS savings, respectively. Their very short wall times and
sub-8,192-node CSR activation imply that small measured timing
differences should not be interpreted as general search speedups.

### Why did packed full-process WallTime improve?

The second study measures Cache teardown separately. On original
HWB6, Cache 50k:

| MPFR bits | Key | Rust solve-only | Cache teardown | Native full process |
| --- | --- | ---: | ---: | ---: |
| 100 | Shared | 877.216 ms | 94.221 ms | 996.541 ms |
| 100 | Packed | 926.446 ms | **12.602 ms** | **948.840 ms** |
| 333 | Shared | 920.810 ms | 193.315 ms | 1137.213 ms |
| 333 | Packed | 987.864 ms | **14.191 ms** | **1013.241 ms** |

The new representation **does not improve the measured DPLL
solve-only time**: packing on admission and packed-hit equality
introduce additional work during the solve. Its large reduction in
allocation count and retained bytes instead makes the final engine
destruction much cheaper. The explicit destruction clock supports
this attribution; remaining external process overhead and runtime
noise are not fully decomposed.

This difference matters by consumption model:

- **Single-shot CLI:** native full-process WallTime is primary; Packed
  improves full WallTime on long HWB6 in both independent cohorts.
- **Long-lived service or Python/Rust API reuse:** prioritize solve-only
  throughput. This experiment does not establish an improvement for
  such use, and no production promotion is justified solely by
  faster Cache teardown.

At the same precision, GPMC C++ remains several times faster than
the best research Rust arm on long HWB6. This note does not claim
RaWMoCS now beats GPMC.

## Reproduce

After regenerating the **original pinned** HWB6 weighted CNF,
run the isolated research executable with matching MPFR precision:

```bash
rawmocs-mpfr-research \
  --node-budget=1000000 --cache-limit=50000 \
  --indexed-unit-prop-csr-after-8192 \
  --packed-residual-key --fast-assign --hashbrown-borrowed \
  30 ORIGINAL_PINNED_HWB6_WEIGHTED.cnf
```

Use `100` instead of `30` for GPMC's 333-bit MPFR precision.
Compare to the *identical* command with
`--shared-residual-key` in place of `--packed-residual-key`,
and a native GPMC `-mode=1 -prec=30` or `-prec=100`
on the same CNF. Never replace the pinned source CNF with an
arbitrarily transformed circuit.

## Decision and next work

Keep the **packed exact Cache as a qualified opt-in research memory
optimization**. Both independent original-source runs support large
RSS savings and lower full-process WallTime, but the additional
solve-only CPU overhead precludes making it a production default.

Next work should minimize the remaining packing and exact lookup
overhead on Cache misses, particularly through selective admission
and/or lifetime-stable canonical residual storage. Evaluate a
persistent-original-clause representation and a reversible assignment
trail as separate research candidates, always retaining exact
scope/clause/literal equality, true SAT-vs-zero distinctions,
native process resource caps, and matched-MPFR original-source
GPMC/Rust comparisons.
