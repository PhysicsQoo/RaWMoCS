# Long source-emitted QuPRS CNF: pivot-policy ablation

**Research only, no changes to the production exact solver or QuPRS.**

The existing [same-runner GPMC/RaWMoCS MPFR comparison](quprs-mpfr-cache-assignment-composed.md)
demonstrated that 100-bit Feynman QFT4 remains substantially slower in
Rust than GPMC despite improving cache key ownership and binary-clause
assignment. Optimizing DPLL **search-node count and solution
decomposition** is distinct from improving each node's allocation cost.

## Predeclared hypotheses

All arms share **the same executable**, the established
`--fast-assign --hashbrown-borrowed` path, 100-/333-bit MPFR and
the unchanged original QuPRS weighted-CNF bytes.

| Arm | Deterministic variable ranking |
| --- | --- |
| Frequency (control) | Highest literal frequency; on ties last active variable |
| ShortClause | Highest sum of `2^(8-min(clause_len,8))` at each occurrence, then frequency and original tie rule |
| MOMS | Highest occurrence count in currently shortest residual clauses, then frequency and original tie rule |

The ShortClause definition is the same predeclared integer heuristic
used in the exact-rational small synthetic branching study. MOMS
generalizes minimum-size-clause occurrence selection. No random
heuristic weights or tuning per circuit.

## Correctness and performance gates

Original QuPRS Rust PathSum/WmcManager exporter commit:
`0b28736651b3740ed3b201edd2efdef3da541e0d`.
The original original Feynman QFT4 and mod5_4 QASM and emitted
weighted-CNF input hashes are preserved without modifications.
Other holdouts are QAOA3, Grover2 and QFT3. Each 100-/333-bit
case/arm runs five alternating fresh native Linux processes under
8-second/2-GiB limits.

A pivot change can alter MPFR addition order, so requiring
byte-identical output **between different pivots** would be
scientifically invalid. Instead each result must pass an independently
predeclared absolute/relative error guard against the **exact
printed-source-decimal rational** WMC, plus a separate SAT verdict;
every arm must be deterministic across its own trials. The control
must additionally match the original exact DPLL node/cache/component
structure. The small QFT3 case has an independent truth-table
enumeration reference; larger original CNFs have only a cross-solver
exact witness, **not independent exhaustive proof**.

Report raw trials, source SHA, median process WallTime, internal
solve time, peak RSS, nodes/cache hits/component splits, timeout,
resource, unknown and numeric failures. A candidate that times out
or exceeds resources is **censored**, never scored as faster and
never omitted. Any correctness, numeric or baseline structural
failure is a failed CI qualification.

No solver is promoted based on a microbenchmark or a small
holdout alone. The primary decision is Feynman QFT4 absolute
WallTime and node count with no material Feynman mod5_4 regression;
additional large QuPRS Feynman circuits would be needed before any
production algorithm change or general-superiority claim.
