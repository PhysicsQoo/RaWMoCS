# Exact width-one forest component WMC (research-only)

This PoC operates only on the **original QuPRS-emitted Weighted CNF**, after
the existing optional weighted-equivalence reduction and reversible Boolean
unit closure. It is **opt-in and off by default**; production code, QuPRS
encoder, and unmodified GPMC comparison are unchanged.

## Certified structure and exact signed weights

For a complete current residual component with 8–512 remaining variables,
at most 2048 source clause IDs, and only unit/binary surviving clauses,
intersect multiple clauses on the same original variable pair as a 4-state
Boolean compatibility mask. The **distinct-variable primal graph must be a
forest**; any cycle, wide clause, cross-component clause, malformed scope,
or exceeded bound causes exact fallback to the original Trail DPLL.

For each rooted tree, the message from child v to parent u sums, separately
for parent truth values, the two compatible child states. The child's own
signed positive/negative Complex MPFR literal weight is multiplied **once**
in its local DP state; the parent multiplies independent child messages.
Root states and independent roots use explicit Boolean SAT along with the
Complex WMC sum/product semiring. Thus **SAT-but-zero is not UNSAT**.
Previously forced and free variable factors remain owned by the unchanged
Trail prefix; this kernel receives only unassigned variables in its exact
component. No constraint, weight, factor or full CNF is approximated.

A proven forest has linear numbers of DP states and edge messages; this is
an actual count shortcut, not a branch-order heuristic. It is **not**
general treewidth-12/14 Join-tree DP, and dense HWB6/QNN4 may be ineligible.

The research CLI is `--trail-forest-dp` on the original fused signed-weight
Trail; it refuses incompatible experimental branches. At most 32 structural
DP attempts per full solver process; default baseline has no extra scan.
Counters: `RESEARCH_MPFR_FOREST_DP enabled attempts completed rejected scans`.
A failed proof never retains partial messages or changes cache keys.

## Falsifiable scientific acceptance

Rust tests cover independent brute-force exact signed Complex real/imag WMC
at 100 and 333-bit MPFR, Boolean UNSAT, cancellation to zero, multiple
clauses on the same edge, free vertices, disconnected forests, cyclic/wide
rejection, 0/1/128 cache capacity, and actual reversible Trail node
reduction on a connected ten-variable source forest. These are **not**
evidence of a real hard-case speedup until final-head CI verifies them.

The existing (not newly-created) Feynman and independent MQT source-locked
workflows append the same-original-CNF three arms: unmodified C++ GPMC,
unchanged Rust Inline, and Rust Inline plus this one flag. Seven rotated
fresh native processes per arm, same machine, original SHA-locked QASM
and Weighted CNF bytes, original signed weights, GPMC 30/100 decimals
against Rust MPFR 100/333 bits, 30s/2GiB virtual-address-space guard.
Record all successful, failed and censored native outcomes; SAT and both
signed Complex WMC channels; p50/p90 complete WallTime, observed peak
RSS, DPLL nodes/cache/decomposition, forest attempts and completions.
The independent original Grover5 censored evidence remains in the
existing research suite and cannot be dropped to claim a global GO.

**No-go policy:** A correct or faster synthetic forest is insufficient.
For hard Feynman HWB6 and MQT QNN4, require actual forest completions
and repeatable p50/p90 speedup with bounded RSS; otherwise record an
explicit native **PERFORMANCE_NO_GO** and do not enable this solver
path by default. The final real-circuit result is pending native CI.
