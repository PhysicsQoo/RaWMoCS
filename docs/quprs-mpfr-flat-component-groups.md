# Source-locked exact component grouping: flat root buckets and DSU reuse

## Research hypothesis

After unit propagation, every DPLL residual is scanned for connected
components. The original `split_components` allocates a fresh DSU
parent vector and a `BTreeMap<root, (vars, clauses)>`. For the source-pinned
MQT Bench-derived QNN4 complex weighted CNF, the preceding native
study observed **1,718 component splits** and **6,827 DPLL nodes**.
These are search statistics, not yet a measured cost breakdown.

This research adds **two opt-in MPFR CLI flags**, without modifying
production `rawmocs-core`, the QuPRS Python/Rust SolverAdapter or
the existing default DPLL/cache behavior:

| Mode | CLI position after `--reuse-frequency-scratch` | Root groups | DSU arena |
| --- | --- | --- | --- |
| Unchanged control | no extra flag | `BTreeMap` | Per-call allocation |
| Flat | `--flat-component-groups` | Sorted contiguous `Vec` buckets | Per-call allocation |
| Flat with reuse | `--flat-component-groups-reuse` | Sorted contiguous `Vec` buckets | Reused Engine scratch |

Every arm uses the *same* exact Packed Fast cache at 50,000 entries,
same flat CSR unit propagation, same branch selection/frequency scratch,
same original CNF, and same binary MPFR precision (100/333 bits).

## Exact semantic contract

The original implementation groups by canonical DSU root in ascending
`BTreeMap` key order. Every group's variable scope retains original
scope order; every clause list retains original input clause order.
Complex MPFR accumulation therefore occurs in the same component order.

The experimental flat mode sorts and deduplicates roots and stores
root-to-group indices in a dense vector. It then appends variables and
clones clauses in the same order as the original. The reuse mode restores
the DSU parent and index buffers to the Engine **before** any recursive
component evaluation, resetting parent entries to identity at the start
of each call; unused root-to-index slots are never read. This avoids
aliasing or persisting stale DSU state across branches.

Both test modes must preserve full real and imaginary MPFR output
**byte-for-byte**, SAT/UNSAT including SAT-but-zero WMC, Cache-hit count,
DPLL nodes, component decomposition count and fixed Cache capacity.
A deterministic generated CNF regression plus disconnected, connected,
contradictory and weighted-zero motifs are exercised at 100/333 bits
and Cache capacity 0, 1, 10, 10k and 50k.

## Native datasets and fairness

**MQT Bench:** Original committed MQT QASM in pinned QuPRS source
`0b28736651b3740ed3b201edd2efdef3da541e0d`, converted without
changing source circuit through original QuPRS PathSum/WmcManager.
These are **QuPRS-emitted complex weighted CNFs derived from MQT QASM**,
not native MQT-published CNFs. Seven original MQT families (WState4,
VQE4, QNN4, Grover4, QWalk3, QAOA7, QPEexact8) each get five
order-balanced native trials for GPMC C++ / Fast / Fast+frequency-scratch
/ flat / flat+DSU-reuse.

**Long Feynman:** Original unchanged HWB6 plus held-out Barenco tof5
and ModMult55 CNFs under eight rotated trials for GPMC /
Fast+frequency-scratch / flat / flat+DSU-reuse. Grover5 remains
censored at 1M bounded DPLL nodes and is **not** ranked.

Each native process uses a 30s/2GiB limit; original QuPRS and native
GPMC C++ source commits are pinned, and precision matches **100 or
333 MPFR mantissa bits** exactly. GPMC and Rust consume the same
emitted GPMC-dialect CNF bytes. GPMC numerical agreement uses the
published complex-result scaled witness; no independent Full WMC
certificate is implied for large cases.

Record per-trial native WallTime, peak RSS, Rust solve-only time,
engine drop time, complete complex output, and matched-capacity search
statistics. Report paired medians **within the same CI runner**, never
cross-run absolute performance ranks. Failing numerical/search
qualification blocks merging and performance claims.

## Decision

Only retain the flat/DSU-reuse strategy for further research if the
completed source-pinned tests show benefit on long HWB6 and at least
representative MQT cases without correctness or resource regression.
The optimization remains opt-in and is not an instruction to adopt
a default production method. If improvements are negligible, use the
evidence to prioritize *clause persistence, zero-copy component views
and reversible assignment trails* instead of additional micro-tuning.
