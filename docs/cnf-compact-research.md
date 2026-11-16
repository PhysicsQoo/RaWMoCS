# Original Weighted CNF: single-pass compact residual keys

**Status:** isolated research-only opt-in; native same-source improvement verified, **not** admitted to production.
The baseline code, released core, QuPRS Adapter and GPMC are unchanged.

## Evidence that motivated this single change

The source-pinned [five-arm native full WallTime study](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43886088190)
measured original QuPRS Feynman Weighted CNF from `0b28736651b3740ed3b201edd2efdef3da541e0d`,
original unmodified GPMC `89bbc89d0de918e7a5385740355df55765039230`.
At 100-bit MPFR and seven rotating fresh processes (p50, ms):

| CNF | GPMC | Rust Eq | Original Trail global scan | Parent active-ID Trail |
| --- | ---: | ---: | ---: | ---: |
| HWB6 | 192.494 | 644.757 | 820.901 | 593.088 |
| Barenco5 | 14.109 | 9.785 | 11.355 | 9.627 |
| ModMult55 | 14.017 | 22.207 | 23.403 | 20.428 |

The parent-ID optimization **reduced HWB6 native complete WallTime
by 27.8% relative to global-scanning Trail** and 8.0% relative to
the prior Rust equivalent-literal DPLL. All six completed 100/333-bit
cohorts have equal SAT/full weighted Complex WMC, stdout bytes,
DPLL nodes/cache hits/decompositions across the two Trail strategies.
Original Grover5 remains node-censored in every Rust arm.

This shows that repeated whole-source scans, not only branch-copy
allocation, caused the first Trail regression. However HWB6 still
runs about 3.08 times slower than original GPMC; that broader
remaining gap is **not** solely attributed to one function.

## New narrowly isolated method

`--weighted-equivalence --trail-count --trail-parent-ids
--trail-compact-residual --hashbrown-borrowed 30 <original.cnf>`
enables a *sixth* experimental arm in the independent research
MPFR executable. Other flags and all existing defaults are unchanged.

The existing Parent-ID Trail already passes only still-relevant
source clause IDs down the recursion. Its `canonical_ids` sorts
IDs via a comparison that repeatedly skips assigned literals in
the same source clause, and the `key` builder repeats that filtering
again when emitting exact PackedKey literal bytes.

The new `canonical_compact` performs a **single filter pass per
active source clause** into a contiguous flat `Vec<i32>` arena.
Small `(source-id, start, end)` views sort/deduplicate based on
the exact reduced literal slices; the final exact PackedKey serializes
those sorted views with unambiguous zero clause separators.
No child `Vec<Vec<i32>>` CNFs are allocated, no hash-only equivalence
or lossy fingerprint is used, and no clause weight or Boolean
assumption is changed.

The extra per-node literal arena may increase temporary memory even
if it reduces comparator rescans. Only a source-faithful end-to-end
native benchmark can settle the tradeoff.

## Scientific contract and gates

- 100- and 333-bit independent signed/dyadic Full Complex WMC
  (real and imaginary), Boolean UNSAT, SAT with a zero numerical WMC,
  forced units, disconnected components and deterministic 40-case
  small random CNFs against the baseline solver, old Trail and
  Parent-ID Trail.
- **Exact topology equality** with Parent-ID Trail per same-CNF trial:
  SAT, both Complex WMC outputs, stdout SHA-256, DPLL nodes, cache
  hits, component decompositions. If any differ, fail closed.
- Six native original-CNF solver arms: unmodified GPMC C++, unchanged
  Rust Frequency, Rust signed equivalent-literal, global Trail,
  Parent-ID Trail, Parent-ID + compact residual. Seven rotating fresh
  subprocesses/arm/source and MPFR 100/333-bit; original Grover5 is a
  bounded censored holdout.
- Report p50/p90 native process WallTime, full raw trials, process
  RSS, original source SHA, SAT and both Complex WMC parts, complete
  failures/timeouts/censored cases. Original QuPRS CNF preparation
  happens **outside** solver WallTime.
- Three new CodSpeed Simulation original-source named identities
  supplement the existing 21 MPFR and 68 unchanged formal-core
  identities. These measure modeled CPU and flamegraph hot functions,
  **not** comparable full process WallTime.
- Admit a non-default router candidate only after repeated
  significant **full WallTime** improvement on multiple cases with
  no unacceptable memory regression and no loss of exactness.
  If gain is small, negative or unqualified, retain this as an
  explicit negative result and do not merge to the default solver.

Reproducible code, runner and CI:
`benchmarks/mpfr_research/src/trail_engine.rs`,
`benchmarks/mpfr_research/tools/quprs_cnf_compact_ab.py`,
`.github/workflows/cnf-compact-science.yml`,
`.github/workflows/cnf-compact-paired.yml`,
`codspeed.yml`.


## First qualified native full-process result (2026-11-16)

[Six-arm original-source native CI #43888223513](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43888223513) **SUCCESS**
at the pre-format code SHA. Seven rotating independent subprocesses per
arm/completed source/precision, same original QuPRS Weighted CNF bytes,
unmodified original GPMC, 100/333-bit MPFR. All six completed cohorts
match full SAT/Complex WMC real+imag. **Every completed Parent-ID vs
Compact trial also matches exact stdout SHA, nodes, cache hits and
component decompositions.**

| Original CNF | MPFR bits | GPMC p50 ms | Rust Eq p50 | Parent-ID Trail p50 | Compact Trail p50 | DPLL nodes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| HWB6 | 100 | 154.476 | 538.046 | 517.535 | **457.996** | 81,594 |
| HWB6 | 333 | 169.089 | 570.673 | 551.704 | **502.742** | 81,594 |
| Barenco5 | 100 | 13.085 | 9.493 | 9.400 | **9.364** | 2,212 |
| Barenco5 | 333 | 13.829 | 10.056 | 10.167 | **10.122** | 2,212 |
| ModMult55 | 100 | 13.132 | 19.695 | 18.600 | **18.011** | 4,269 |
| ModMult55 | 333 | 13.662 | 21.112 | 19.593 | **18.827** | 4,269 |

HWB6 100-bit Compact is approximately **11.5% faster** than Parent-ID
and **14.9% faster** than the original signed-equivalent-literal Rust
arm in the *same* runner. Versus original GPMC, however, Compact still
takes approximately **2.96x** longer. HWB6 333-bit improves Parent-ID
by ~8.9%. ModMult55 100-bit improves Parent-ID by ~3.2%. The
smaller Barenco gains are subpercent, not a broad breakthrough.

Original Grover5 100-bit GPMC completed while every Rust arm,
including Compact, remained node-censored; **none of the incomplete
Rust trials are eligible for a runtime ranking**. Complete p90/RSS
and censored records are preserved in
[CI artifact](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43888223513/artifacts/14235502486).

Corroborating [Parent-ID original-source CodSpeed Simulation run](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6afb16081526ca3fcea78dc7)
identifies `canonical_ids` filtered-clause self ~11.67% and
`cmp_residual` self ~5.78% on HWB6, plus sorting and PackedKey hashing.
Those are modelled CPU costs, not native WallTime; the A/B above is
the first native evidence for this compact-key optimization.

**Decision:** preserve as a positive, qualified *research-only*
structure optimization. Do not enable it as a default or infer GPMC
parity without an additional hard holdout (MQT/long Grover), observed
RSS regression review, and a search-algorithm study that can reduce
the source-specific DPLL tree. Formatting-only final-head CI may
repeat; never inherit a pass for different source code.
