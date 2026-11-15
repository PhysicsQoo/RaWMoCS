# DPLL frequency workspace reuse (research-only)

## Hypothesis

The MPFR research solver previously allocates a fresh
`Vec<usize>` of `variables + 1` entries at every visited DPLL node,
then populates it by scanning its residual CNF to select the branch
pivot. On long source-pinned CNFs, these per-node allocations can add
up. This experiment reuses one engine-owned scratch workspace.

The explicit research-only CLI flag is
`--reuse-frequency-scratch` after `--fast-assign` and before
`--hashbrown-borrowed`. The existing `--packed-residual-key-fast`
mode **without** this flag remains the control. Production
`rawmocs-core`, exact rational arithmetic and QuPRS selection
remain unchanged.

## Semantics and safe recursion boundary

* Move the reusable vector out of `Engine` for counting the current
  node. All actual residual variables belong to the active scope.
* After the unchanged frequency scan, create the same active variable
  list and preserve the same `max_by_key` tie-breaking order.
* Reset every active entry to zero and return the vector to `Engine`
  **before** any component-recursion or positive/negative branching.
  This avoids aliasing scratch state across recursive calls.
* No changes to canonical residuals, exact packed Cache key, admission,
  complex MPFR operations, SAT-versus-zero distinction, or branch order.

## Verification

Rust regression fixtures cover satisfiable, conflicting, SAT-but-zero
and decomposable motifs across both MPFR100/333-bit precision and
0, 1, 10, 10k, 50k Cache capacities. They compare complete real and
imaginary output, SAT outcome, all DPLL nodes / Cache hits / component
counts, packed Cache size, and scratch allocation identity and
cleanliness after return.

The existing source-pinned native
[`quprs-feynman-long-source-scout.yml`](../.github/workflows/quprs-feynman-long-source-scout.yml)
compares six independently spawned, two-cycle balanced GPMC C++,
original Fast and Fast+scratch runs on original HWB6 and held-out
Barenco tof5 / ModMult55. It uses unchanged source QuPRS
`0b28736651b3740ed3b201edd2efdef3da541e0d`,
GPMC `89bbc89d0de918e7a5385740355df55765039230`,
100/333-bit matched MPFR, 50k exact Cache, bounded DPLL nodes,
30s/2GiB native process isolation, full Rust complex stdout byte
identity, structural parity and GPMC numeric agreement.
Separate solve-only, native WallTime, engine-drop and peak RSS medians
are archived. Grover5 remains censored and is not ranked.

**Do not infer that workspace reuse is faster without finished
same-runner native measurement.** Large original Feynman source/solver
agreement is not independent complete Full WMC proof. The research flag
must not become a production default without additional varied-case
evidence, including original MQT Bench-derived weighted CNFs.
