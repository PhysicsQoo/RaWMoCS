# Original QuPRS long-CNF indexed Unit Propagation study

**Opt-in MPFR research only.** Production exact-rational `rawmocs-core`,
the default research engine and QuPRS SolverAdapter remain unchanged.
Source QuPRS exporter SHA `0b28736651b3740ed3b201edd2efdef3da541e0d`
and unchanged GPMC C++ SHA `89bbc89d0de918e7a5385740355df55765039230`.

## Mechanism

After normalized residual Cache lookup, the baseline rescans every
clause for empty clauses and selects the first unit clause after
each forced literal, allocating a new residual CNF per assignment.
The research-only `--indexed-unit-prop` builds an occurrence index
**only on nodes with unit clauses**, tracks lengths and live clauses,
and maintains a min-heap of original source clause positions.
Each assignment deactivates satisfied clauses and updates only
occurrences of its opposite literal. The heap must select exactly
the same first active unit clause as the baseline, maintaining
the multiplication order of the same rounded complex MPFR weights.
The remaining CNF is reconstructed once per propagation sequence,
without changing literal order, sorted Cache Key, component split,
pivot, SAT-versus-zero semantics or finite node budget.

## Experimental controls

Combine `--indexed-unit-prop --shared-residual-key --fast-assign
--hashbrown-borrowed` in the same native Rust MPFR binary; compare
against the identical shared-Residual reference without the new
flag. The preexisting original-source QFT4 suite uses five
alternating fresh native processes per arm at 100 and 333 MPFR
bits, plus Feynman mod5_4, QAOA3, Grover2 and QFT3 controls.

The **primary long source** is Feynman HWB6 (297 CNF variables,
827 clauses, 273,928 baseline DPLL nodes) with a finite 1M-node
research cap. Compare **unchanged GPMC C++**, shared-Rust, and
indexed-Rust with three rotating fresh native runs per arm,
at both 100 and 333 bits, on identical original source CNF
SHA bytes. Include original Feynman Barenco Toffoli5 and
Modular Multiplier55 holdouts. Grover5, which previously
exceeded the 1M-node limit, remains **censored** without an
invented runtime or false success claim.

A qualifying optimization must preserve **byte-identical Rust
MPFR stdout**, SAT verdict, DPLL nodes, cache hits and
component decompositions, and cross-solver numeric agreement
under the same MPFR mantissa bits. Compare complete native
process WallTime and peak RSS under the original 8s/2GiB
small cohort and 30s/2GiB long cohort process bounds.
All failures and raw trials are retained. Larger original
sources lack an independent exhaustive complex Full WMC
oracle, so their status remains `SOLVER_AGREEMENT_ONLY`.

## Promotion criteria

No merge based on microbenchmark or synthetic speed alone:
prefer substantial, repeatable **HWB6** WallTime savings
with acceptable memory and no Feynman holdout regression.
A slower variant remains a documented negative result and
must not replace default production behavior.

## Work-based activation follow-up

The first source-locked full-index mode proved **30% lower HWB6
native WallTime at 273,928 DPLL nodes**, while short Barenco and
ModMult controls regressed 4–8%; QFT4 showed neutral results.

Because this is a *benchmark-informed* hypothesis rather than
held-out proof, also test optional
`--indexed-unit-prop-after-8192`. It preserves the legacy
unit-scan algorithm while fewer than **8,192 DPLL nodes** have
been visited, then switches to the same indexed implementation.
This avoids constructing a per-node occurrence table on most
small/easy inputs, without checking any benchmark name or
variable count, and without raising the finite node cap.

The threshold was chosen after inspecting the current corpus
(QFT4 ~5,119 nodes; Barenco ~4,325; ModMult ~5,699;
HWB6 ~273,928), so generalization to independent real CNF
corpora is **not established**. Test the unmodified shared-Cache
control, unconditional indexed propagation and admitted indexed
propagation on every retained source at the same original
100/333-bit precision. Compare the raw per-trial WallTime,
solve time, peak RSS, SAT, complete output and search stats.
Keep Grover5 censored at 1M nodes. No production default
promotion is permitted merely from threshold tuning.

## Qualified original-Feynman four-arm observations

The [original long-Feynman same-runner qualification
#43611425306](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43611425306)
passed **6/6 original source × precision groups** at unchanged
100- and 333-bit GPMC/Rust MPFR precision. All three Rust
configurations have the same **byte-for-byte native complex WMC
stdout, SAT verdict, DPLL nodes/cache hits/component decompositions**.
Each point is the median of three rotating fresh processes under
the same 30-second/2-GiB address-space bound. Source files,
uncensored raw trials, original QASM/CNF hashes and all censored
outcomes remain in the GitHub Actions artifact.

| Original Feynman CNF | Precision | GPMC C++ WallTime (ms) | Shared Rust control (ms) | Always indexed (ms) | Indexed after 8192 nodes (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| HWB6 (297v/827c) | 100-bit | 213.311 | 1799.512 | **1316.554** | **1358.327** |
| HWB6 | 333-bit | 237.863 | 1899.150 | **1423.905** | **1468.312** |
| Barenco Toffoli5 (97v) | 100-bit | 12.774 | 22.223 | 24.811 | 23.281 |
| Barenco Toffoli5 | 333-bit | 12.484 | 22.406 | 24.959 | 22.510 |
| Modular Multiplier55 (91v) | 100-bit | 12.134 | 29.947 | 32.369 | 30.208 |
| Modular Multiplier55 | 333-bit | 12.206 | 31.048 | 32.732 | 32.127 |

The primary original HWB6 instance completes **273,928 DPLL
nodes, 116,315 cache hits and 42,464 component decompositions**.
Work-admitted indexed propagation improves native WallTime
by **24.5%** at 100 bits and **22.7%** at 333 bits versus
the prior shared-Residual Rust implementation, with no
change in counted assignments or number of search nodes.
The always-indexed arm is 2–3% faster than the admitted arm
on this case but regresses small controls more.

HWB6 peak RSS (KiB) at 100 bits is GPMC 18,484,
shared Rust 71,608, always-indexed Rust 72,036, admitted
Rust 72,000. At 333 bits those values are 19,468,
72,184, 72,720 and 72,676. Thus admitted mode increases
RSS by about **0.55–0.68%** over existing Rust.

Although the selected admitted mode is substantially faster,
GPMC C++ still wins at HWB6 by **6.37×** at 100 bits and
**6.17×** at 333 bits. GPMC and Rust use the same
original weighted CNF and MPFR mantissa width, but not
necessarily the same floating-point summation order.
The large original CNFs have **cross-solver numerical agreement**,
not an independent exhaustive Full WMC oracle.

The [five-trial QFT4 cohort
#43611425162](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43611425162)
passed 10/10 source × precision cohorts and 14 Rust tests.
At 100 bits the native process medians (shared /
unconditional / admitted) were QFT4 **60.128 / 59.490 /
60.358 ms** and mod5_4 **7.359 / 7.753 / 7.369 ms**.
Admitted mode avoids the unconditional small-case penalty.
The modest differences among admitted and shared modes on
those cases, and 3-trial Barenco/ModMult fluctuations, are
not evidence of reproducible speed differences when the
8,192-node gate was never crossed.

## Decision and remaining limits

[Research PR #68](https://github.com/PhysicsQoo/RaWMoCS/pull/68)
merged only an **optional, disabled-by-default** indexed unit
propagation feature and the 8,192-node work-admission
candidate. This is **not** a QuPRS production solver change,
nor a promotion to the default Rust exact-rational algorithm.

The 8,192 threshold was selected using knowledge of these
cases and has not been independently tuned or certified on
held-out SATLIB, other QuPRS circuits or alternative CPUs.
No universal speedup or robust small-case equivalence in
physical WallTime is claimed. An independent exhaustive
complex Full WMC witness is infeasible for the large cases
in this study. Grover5 (607 CNF variables/1,121 clauses)
remains **censored at the 1M-node budget** with no valid
Rust-vs-GPMC performance ranking.

Next algorithm-level work should seek source-correct
**watched-literal binary propagation and bounded conflict
learning**, consistent with the GPMC architecture, and
component/cache sharing across explored search branches.
Any new strategy must separately preserve Boolean SAT
versus zero WMC, complex MPFR precision, original phase
provenance, finite resource safety and unchanged default
SolverAdapter. Extra benchmarks should include truly
independent long CNFs to test generalization.

