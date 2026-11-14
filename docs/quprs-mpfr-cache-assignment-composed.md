# Matched-MPFR long-QFT cache/assignment composability study

This is a research-only, controlled interaction test of two previously
measured Rust MPFR performance hypotheses: borrowed residual-cache
lookup (avoids full Key copies on hits) and normalized two-literal
clause assignment. Both operate on the original QuPRS
PathSum/WmcManager-emitted weighted CNF at exactly the **same MPFR
mantissa precision** as the GPMC reference: 100 and 333 bits.

Four modes run in the **same compiled executable**, each with five
rotating fresh-process trials per case: unchanged std-owned cache
(`std_original`), standard cache with specialized assignment
(`std_assign`), borrowed-cache baseline assignment
(`borrowed_original`), and the combined path
(`borrowed_assign`). The default remains `std_original`; opt-in
flags are `--fast-assign` and `--hashbrown-borrowed`.

For each mode, require matching source CNF SHA, SAT, full MPFR
stdout byte hash and search nodes/cache hits/components against the
qualified original exact-rational reference, with proper censored
timeout/resource statuses. Keep all per-trial WallTime, internal
solve time and RSS; report the actual combined ratio, never a
product of separate published speedups.

The **highest-cost original Feynman qft_4** is the primary decision
case. Original Feynman mod5_4 and QAOA3/Grover2/QFT3 controls test
regressions and generalization. GPMC remains the original unchanged
native C++ solver. This experiment changes no production
`rawmocs-core` API or QuPRS SolverAdapter setting, and does not
assert an improvement until the source-pinned CI completes.
