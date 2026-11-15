# Flat CSR occurrence indexing: matched-MPFR long-Feynman research

**Scope:** isolated `benchmarks/mpfr_research` executable only. Neither
production exact-rational `rawmocs-core`, the published GPMC binary, nor
QuPRS SolverAdapter is changed. The feature is **disabled by default**:
`--indexed-unit-prop-csr` and
`--indexed-unit-prop-csr-after-8192` select it explicitly.

## Mechanism

The existing occurrence-indexed unit propagator allocates independent
`Vec<clause ID>` buckets for both literal polarities of each variable
inside each normalized residual DPLL node. The alternative constructs
counts, prefix offsets and one flat CSR clause-ID buffer. Fill occurs in
original clause order; source-index min-heap propagation, exact
contradiction semantics, clause/literal order, canonical residual cache
keys, component decomposition, pivot choice, node budget and complex
MPFR evaluation order remain unchanged.

This **does not** retain the CSR arena across DPLL branches, introduce
global watcher state, learn clauses or change production search. The
`after-8192` gate comes from the previous source-informed indexed
propagation study; it is not a universally qualified heuristic.

## Fair numerical and resource contract

- Original, unchanged QuPRS Feynman QASM and weighted CNF captured from
  QuPRS source SHA `0b28736651b3740ed3b201edd2efdef3da541e0d`.
- Unchanged GPMC C++ source SHA `89bbc89d0de918e7a5385740355df55765039230`.
- Identical source weighted CNF and GPMC/Rust **100/333-bit MPFR mantissa**
  widths, four individually rounded complex products (no implicit FMA).
- SAT verdict, complete Rust printed complex stdout, nodes/cache hits/
  component splits and source SHA checks must pass. GPMC numerical
  agreement has an explicitly limited guard; large original CNFs lack
  an independent exhaustive complex Full WMC oracle and are labelled
  `SOLVER_AGREEMENT_ONLY`.
- Same GitHub-hosted Linux runner **within each experiment**, alternating
  independent full native processes; 30-second/2-GiB virtual-address
  bounds, finite DPLL node budgets and all censored outcomes retained.
  Do **not** rank cross-run absolute times between different runners.
- The isolated MPFR test suite includes 96 deterministic random weighted
  CNFs at 100/333 bits, conflict/SAT-but-zero WMC controls, and
  source-order unit propagation checks; 16 tests passed in both CI runs.

## Source-locked qualification

[First three-trial CI run](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43675425657)
and [independent five-trial CI run](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43676002532)
both completed with the same SAT, complete Rust output and DPLL
structure. Numbers are within-run medians, in milliseconds.

| Original source | MPFR bits | CI trials per arm | GPMC C++ | Indexed Rust | CSR Rust | CSR WallTime improvement |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| HWB6 | 100 | 3 | 227.586 | 1440.239 | 1364.999 | 5.22% |
| HWB6 | 333 | 3 | 254.073 | 1582.969 | 1483.040 | 6.31% |
| HWB6 | 100 | 5 | 151.769 | 1015.197 | 910.709 | 10.29% |
| HWB6 | 333 | 5 | 169.356 | 1113.661 | 980.315 | 11.97% |
| Barenco Toffoli5 | 100 | 5 | 13.044 | 21.637 | 21.654 | -0.08% |
| Barenco Toffoli5 | 333 | 5 | 13.270 | 21.070 | 21.290 | -1.04% |
| Modular Multiplier55 | 100 | 5 | 13.066 | 27.722 | 28.101 | -1.37% |
| Modular Multiplier55 | 333 | 5 | 13.533 | 29.498 | 29.468 | 0.10% |

Original HWB6 completes 273,928 DPLL nodes. In the five-trial run,
Rust solve-only medians improved **979.881→847.572 ms** at 100 bits
and **1044.750→913.226 ms** at 333 bits, consistent with a real
propagation and allocation saving rather than startup effects.

Peak-RSS medians (KiB) on HWB6:

| MPFR bits | GPMC C++ | Indexed Rust | CSR Rust |
| --- | ---: | ---: | ---: |
| 100 | 18548 | 71976 | 71616 |
| 333 | 19440 | 72724 | 72456 |

CSR peak RSS is slightly lower than indexed Rust on both HWB6
precisions. The smaller holdouts are below the 8,192-node activation
threshold (4,325 and 5,699 nodes) so their roughly ±1.4% native
WallTime variation should not be interpreted as a universal
microbenchmark regression or a meaningful improvement.

## Scientific decision

**Retain the CSR implementation as an opt-in, disabled-by-default
research feature only.** Its repeatable long-source improvement,
unchanged complete MPFR output and search structure, and non-increasing
HWB6 RSS justify preservation. They do **not** justify switching
production exact arithmetic, claiming general speed superiority or
relaxing the source/precision/resource acceptance contract. In the
five-trial run, unchanged GPMC C++ remains approximately 5.8–6.0×
faster than CSR Rust on HWB6.

The next higher-leverage experiment should address the remaining
search representation difference: GPMC initialises a persistent
occurrence pool and maintains Glucose-derived watches/trail across
branches, while this Rust MPFR research engine still reconstructs
residual clauses and local indices per DPLL node. Prototype
persistent immutable clause storage and reversible assignment trails
as separate research variants, and compare under the same frozen
source/MPFR/resource contract. Never derive a performance ranking
from CodSpeed CPU Simulation compared with native WallTime.
