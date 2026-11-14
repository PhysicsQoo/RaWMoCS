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

## First source-locked combination results

[Native four-arm CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43587869065),
source HEAD `c98421f4bc85f5804355e55880a49b7185a30f09`,
reports **10/10 semantic parity** (5 circuits × 2 MPFR precisions,
five fresh-process runs per arm). All four modes returned identical
SAT, complete MPFR stdout and structural statistics; the unchanged
GPMC C++ and exact source were checked in the same workflow.

| Source input | Bits | Original std-owned | Assignment only | Borrowed Cache only | Combined |
| --- | ---: | ---: | ---: | ---: | ---: |
| Feynman qft_4 | 100 | 70.755 ms | 65.672 ms | 64.494 ms | **59.333 ms** |
| Feynman qft_4 | 333 | 72.483 ms | 67.565 ms | 66.069 ms | **61.831 ms** |
| Feynman mod5_4 | 100 | 7.249 ms | 7.115 ms | 7.044 ms | **6.811 ms** |
| Feynman mod5_4 | 333 | 7.590 ms | 7.302 ms | 7.351 ms | **7.094 ms** |
| QAOA3 | 100 | 4.609 ms | 4.669 ms | 4.673 ms | 4.563 ms |
| Grover2 | 100 | 4.623 ms | 4.639 ms | 4.599 ms | 4.610 ms |
| QFT3 | 100 | 4.317 ms | 4.360 ms | 4.360 ms | 4.339 ms |

QFT4 source-matched **100-bit native WallTime falls 16.1%** and
333-bit falls 14.7%, with internal solve medians falling from
60.981 to **50.088 ms** and from 62.305 to **52.302 ms**.
The 100-bit process peak RSS median changes only from 15,300 KiB
to 15,240 KiB. The longer Feynman mod5_4 cases also improve,
while the small controls are within process startup noise or mixed.
This is a **within-Rust same-bit** result, not a cross-solver rank.

The follow-up workflow runs **five alternating GPMC-vs-combined-Rust
fresh native processes** at the same 100/167/333-bit MPFR precision,
with unchanged source CNF, exact reference, SAT and structural gates,
and publishes an independent
`quprs-mpfr-matched-optimized.json` artifact. A separate matched
comparison is required: earlier GPMC timings must not be combined
as a definitive contemporaneous leaderboard.
