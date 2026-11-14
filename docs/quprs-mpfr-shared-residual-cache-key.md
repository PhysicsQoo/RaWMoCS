# Source-locked MPFR residual Cache Key storage A/B

The previous borrowed-cache optimization avoids deep-cloning the
canonical Residual CNF **on a hit**, but still clones every clause and
literal on a miss to create an owned Cache Key. In a high-sharing
search such as original QuPRS Feynman QFT4, this may dominate
per-node overhead.

This research-only experiment tests a second, orthogonal idea:
after a borrowed, content-equality checked Cache lookup misses,
**move the already normalized Residual Vec into an Arc**. The
stored Cache Key keeps a cloned Arc pointer, not a deep copy of its
clauses. On a hit, check the original Vec **before** allocating an
Arc. Hashing and equality still cover the complete variable scope and
entire canonical clause set, never only the hash.

The two arms use the same native Rust MPFR executable, identical
original QuPRS emitted GPMC weighted CNF bytes and the same
100/333-bit MPFR GPMC precision contract. Both use fast two-literal
clause assignment, the original Frequency pivot, 10,000-entry cache
limit and hashbrown borrowed lookup. Shared mode is an opt-in
`--shared-residual-key` flag and requires
`--hashbrown-borrowed`. Default engine and production exact
`rawmocs-core` are unchanged.

On original Feynman qft_4, mod5_4, QAOA3, Grover2 and QFT3, run
five alternating independent native processes per mode per
precision. An admitted performance record must match SAT, full
MPFR output bytes, exact DPLL nodes, cache hits and component
decompositions, original source SHA and precision bits. Report
separate process WallTime, solve-only time, peak RSS and every
timeout/OOM/censored case. The independent exhaustive Full WMC
oracle only applies to the small eligible control cases; larger
instances use source-locked cross-solver and exact-reference
comparisons rather than independent exhaustive proof.

No production change or merge is authorized by a positive
microbenchmark alone. The primary decision is long QFT4
end-to-end improvement without mod5_4 degradation or memory growth.

## Expanded original Feynman source-scout test

The user-facing performance priority is **long-running cases**.
The separate source-scout CI now also tests the original
`feynman_hwb6` CNF (297 variables, 827 clauses) which qualified
a finite 1,000,000-node cap, together with original Feynman
`barenco_tof_5` (97 variables) and `mod_mult_55` (91 variables).
At 100 and 333 MPFR bits, perform three rotating fresh process
trials each for unchanged GPMC C++, the original Borrowed Cache
Rust control, and the shared-residual candidate. HWB6 is a
**seconds-scale** Rust solver benchmark, rather than a startup
dominated microbenchmark. Grover5 (607 variables) remains
explicitly censored at the 1M-node limit, without a speed rank.

The long scout uses identical original CNF bytes, explicit 30s
native process and 2 GiB virtual address bounds, unchanged GPMC,
finite Rust node limits, SAT and numerical acceptance and exact
stdout/search-structure parity for both Rust arms. All WallTime,
RSS, solver-only time and failures are recorded. For large source
CNFs, scientific status is cross-solver agreement only, not an
independent exhaustive WMC or quantum phase certificate.

## Qualified same-precision original Feynman outcomes

[QFT4 source-pinned study #43596162985](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43596162985)
and [original long-Feynman 30s study #43597197267](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43597197267)
qualified the opt-in shared-residual strategy with SAT, source SHA,
same MPFR bits, complete Rust numeric stdout and
nodes/cache/component structural parity. The GPMC C++ solver is
unchanged and runs in the same Linux runner with the same MPFR
mantissa precision for the long-original cases.

The source-scout study uses three alternating fresh-process
trials for each tool at 100/333 bits, **30 s / 2 GiB virtual-address**
bounds, and the prequalified finite 1,000,000-node DPLL cap for HWB6.
Process WallTime includes executable startup, input parsing,
search, MPFR operations and output. Values below are median WallTime.

| Original untouched Feynman CNF | Bits | GPMC C++ ms | Rust borrowed-owned ms | Rust shared residual ms | Rust speedup | Rust owned/shared peak RSS (KiB) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| HWB6, 297 vars / 827 clauses | 100 | 171.081 | 1687.829 | **1512.265** | **1.116×** | 67776 / 71528 |
| HWB6 | 333 | 190.315 | 1777.335 | **1589.005** | **1.119×** | 68432 / 72184 |
| Barenco Toffoli5, 97 vars | 100 | 9.984 | 20.135 | **18.240** | 1.104× | 8304 / 8624 |
| Barenco Toffoli5 | 333 | 10.392 | 21.437 | **19.379** | 1.106× | 8432 / 8732 |
| Modular Multiplier55, 91 vars | 100 | 9.638 | 28.710 | **25.824** | 1.112× | 10040 / 10552 |
| Modular Multiplier55 | 333 | 9.906 | 31.520 | **27.372** | 1.152× | 10332 / 10788 |

The longest completed original source, HWB6, visits **273,928**
DPLL nodes, records 1.512 s for shared Rust at 100 bits and
still trails GPMC's 0.171 s by approximately **8.84×**.
Rust peak RSS increases about **5.5%**, or 3,752 KiB, on
HWB6 at 100 bits. The same 5.5% relative increase occurs at
333 bits, and is not hidden by the positive WallTime.

The separate five-trial source QFT4 study also passes 10/10 case×
precision cohorts (complete stdout and search parity). QFT4
at 100 bits improves 72.147 → 66.352 ms; at 333 bits,
73.627 → 66.934 ms, with 100-bit RSS 15136 → 15776 KiB.

**Decision:** retain the optional shared-`Arc` storage in the
research MPFR tool; do not turn it on by default or integrate
it into production exact-rational `rawmocs-core` until additional
memory and correctness evidence supports doing so. The original
Grover5 CNF (607 variables / 1,121 clauses) is still
**censored at 1,000,000 DPLL nodes** under the finite limit and has
no comparable Rust WallTime. Larger cases lack an independent
exhaustive exact Full WMC oracle: the label remains
`SOLVER_AGREEMENT_ONLY`, not independently certified quantum
phase fidelity or broad algorithm superiority.

Follow-up engineering should prioritize Grover5/HWB6 **search-tree
reduction and bounded resource safety**, with equal-precision
GPMC/Rust comparisons, rather than stable/unstable residual
sorting or ShortClause/MOMS pivots. Those latter experiments
showed no benefit or large QFT4 regressions.

