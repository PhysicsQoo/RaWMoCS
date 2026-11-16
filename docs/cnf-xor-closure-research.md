# Bounded XOR implication closure from the original Weighted CNF

**Status: opt-in research; no released API/default/QuPRS adapter changes.**

## Problem boundary

RaWMoCS receives the **same immutable QuPRS-generated Weighted DIMACS CNF
bytes and signed/complex literal weights** as unchanged original GPMC C++.
QASM parsing, Path-sum, reduction, ANF production and weighted CNF
generation belong to QuPRS; the solver does none of those operations.
The original QuPRS and GPMC revisions are respectively
`0b28736651b3740ed3b201edd2efdef3da541e0d` and
`89bbc89d0de918e7a5385740355df55765039230`.

Research feature `--xor-implied-closure` runs **after CNF parsing,
inside the Rust solver's measured solve time**, before DPLL. It has no
effect unless explicitly requested.

## Exact transformation and WMC semantics

1. From each unordered 2- or 3-literal clause group on distinct
   variables, require **all** 2^(k-1) forbidden truth assignments of one
   parity before admitting an implied XOR equation; duplicates count
   only once. Incomplete and unrelated clauses stay in the CNF.
2. Perform bounded GF(2) Gaussian elimination and RREF. Every derived
   equation is a linear consequence of source equations and therefore
   of the original CNF. When a zero-LHS, one-RHS row exists, append an
   **empty clause**, proving Boolean UNSAT; WMC is exactly zero.
3. For RREF equations with support one, add the proven unit clause.
   For support two, add the two standard clauses for equality or
   inequality. **No original clause is removed, no variable is deleted,
   no complex literal weight is changed and no new auxiliary variable
   is introduced.** Consequently every complete satisfying assignment
   survives exactly once with its original signed/complex weight.
4. Never infer SAT from lack of an XOR contradiction and never confuse
   SAT with numerically cancelled complex WMC. Ordinary DPLL still
   processes the full original residual CNF, strengthened only by
   these logically redundant clauses.

The implementation caps original dimension at <=1024 variables,
<=5000 clauses, <=512 source-admitted XOR equations, <=250k GF(2)
row-XOR operations, and <=1024 added clauses. On any cap breach,
it returns **the entire original CNF unchanged** with a CENSORED
diagnostic. It is an implication-preprocessing experiment, **not**
unbounded variable elimination, arbitrary-width GF(2) WMC, an
incremental branch-propagation system, or an independent DRAT proof.

## Independent science and fixed benchmarks

- Exact assignment-set identity for every assignment of 6-variable
  generated CNFs, including incomplete XOR groups, unrelated mixed
  clauses, duplicates and source-supported contradictions.
- Original versus strengthened DPLL complete complex WMC and separate
  SAT at both 100 and 333 MPFR binary bits with signed dyadic complex
  literal weights. Explicit contradiction, binary implication and
  conservative budget/negative controls.
- The fixed same-CNF performance CI uses unmodified QuPRS source
  fixture generation outside solver timing, and then passes **the
  identical Weighted CNF bytes** to original GPMC C++, baseline Rust,
  and XOR-closure Rust on one Linux runner. Original Feynman HWB6,
  Barenco5, ModMult55 and bounded Grover5 represent easy, medium and
  costly cases. Three order-rotated independent subprocesses per arm,
  both 100/333-bit MPFR precision, unchanged GPMC 30/100 decimal
  digits, 30s timeout and a 2 GiB **virtual address-space** guard.
- Preserve source SHA-256, raw SAT, both Full Complex WMC components,
  solver-process p50/p90 WallTime, peak observed RSS, DPLL nodes,
  cache hits, component splits, number of inferred units/binary
  clauses, XOR rank and budget fallback. Large original QuPRS sources
  have solver-to-solver numerical agreement, not independent
  exponential-time model-count certificates.
- In the case of UNKNOWN/node budget, timeout or process failure,
  record **CENSORED**, never fill speed rankings with missing samples.
  Do not interpret baseline UNKNOWN versus proof UNSAT as a completed
  solve time speedup.

Reproduce using:
- `.github/workflows/cnf-xor-closure-science.yml`
- `.github/workflows/cnf-xor-closure-paired.yml`
- `benchmarks/mpfr_research/src/cnf_xor_closure.rs`
- `benchmarks/rust_core/tools/quprs_cnf_xor_closure_ab.py`

## Original QuPRS same-Weighted-CNF paired result (2026-11-16)

**Qualified isolated science:** [100/333-bit Rust CI run 43824987830](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43824987830):
31/31 full research tests passed, zero failures, restricted legacy Clippy
allowances and module Rustfmt check passed. This includes small-CNF
independent full Boolean assignment-set enumeration and both signed
complex MPFR WMC components. It is not an independent large-circuit
WMC certificate.

**Original same-CNF comparison:** [GPMC / Rust three-arm CI
43824987808](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43824987808)
SUCCESS; archived original QuPRS input bytes, SHA-256, raw per-trial
process WallTime/RSS, SAT/complex outputs and XOR inference diagnostics
in artifact ID **14206507003**
(`original-quprs-weighted-cnf-xor-closure-gpmc-three-arm`).
Exactly 6/6 completed original circuit x precision cohorts
agreed on SAT and both complex WMC components within matched
source-decimal precision; each complete arm has **3 rotating fresh
native subprocess trials**. The Grover5 baseline reaches a declared
Rust 1M-node limit and is CENSORED, not ranked as an optimization.

| Same original Weighted CNF | Bits | GPMC process p50 ms | Rust unmodified p50 ms | Rust XOR p50 ms | DPLL nodes before to after | XOR detected / derived units |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| HWB6 | 100 | 176.844 | 731.466 | 725.027 | 94209 to 94209 | 29 / 0 |
| HWB6 | 333 | 195.405 | 781.678 | 779.799 | 94209 to 94209 | 29 / 0 |
| Barenco5 | 100 | 11.851 | 18.349 | 18.179 | 4325 to 4325 | 4 / 0 |
| Barenco5 | 333 | 11.538 | 19.443 | 19.649 | 4325 to 4325 | 4 / 0 |
| **ModMult55** | **100** | 10.915 | 25.771 | **20.452** | **5699 to 4577** | **10 / 2** |
| **ModMult55** | **333** | 10.888 | 27.393 | **21.922** | **5699 to 4577** | **10 / 2** |

The genuine ModMult55 improvement is **19.7% fewer Rust DPLL nodes**,
and **20.6% / 20.0% lower full Rust child-process p50 WallTime** at
100/333 bits (control vs opt-in). Those results include GF(2) inference
cost, unlike kernel-only timing. Associated ModMult55 Rust p50 observed
peak RSS at 100 bits is 5928 KiB (control) versus 5428 KiB (XOR);
at 333 bits 6040 vs 5488 KiB. The other two completed families
have **no inferred units, no changed DPLL tree**; there is no qualified
cross-corpus speedup.

**GPMC remains faster on the positive case:** on identical ModMult55
CNF it completed in 10.915 / 10.888 ms versus optimized Rust
20.452 / 21.922 ms. This is a measured but insufficient algorithmic
step towards GPMC performance, not a claim of solver victory.

At 100 bits, Grover5 GPMC completed a *single* fresh child process
in **21.943 s** with observed RSS 609188 KiB; the Rust control
returned **node-limit CENSORED_EXIT** after 10.825 s with no
completed output. The experiment intentionally stopped before
comparing the candidate on that cohort. That case is neither an
XOR-fast-path success nor a valid speed ranking.

Per-case original Weighted CNF SHA-256 (all solver arms consume the
same file bytes in each experiment):

- HWB6: `e0381c270963e0715273530e48f159dc05b1cd96c9b85a556741a72a4289471d`
- Barenco5: `f1fb459a058ba3dc17d2bd538a7e8b34e451383ae3384cb58a624bc8194499c3`
- ModMult55: `f8c76d883923363a7fcb5e32d6c6d8914f8443af3582fbe3d2e1d9c0b99c35b7`
- Grover5 (censored): `eeb8e82273b1e9dc105e004d73b3e59f1d48bb893fe768bb4034262224a780b0`

The observed 2 GiB limit is `RLIMIT_AS` virtual address space,
**not a kernel hard physical RSS cap**. Three measurements on one Linux
runner are preliminary, not a portable or robust benchmark. Large
non-exhaustively certified inputs retain the explicit
`SOURCE_MPFR_AGREEMENT_ONLY` category; GPMC/Rust may evaluate
complex products in different summation order. The source preparation
is excluded from the timed solver children and must remain QuPRS-owned.

**Decision: keep opt-in, no production promotion.** The source
XOR extraction alone has real benefit on one original CNF, but no
meaningful improvement on HWB6 or Barenco5; Grover5 remains
unresolved. Next experiment should target derived low-support parity
constraints that survive past root RREF, with **bounded branch-aware
propagation** and explicit search/Cache cost accounting. Variable
substitution must compensate all signed/complex literal weights and
cannot simply reuse SAT-only elimination.

## Adoption gate

No production default or automatic router change is warranted on
synthetic XOR demonstrations or a single source-specific speedup.
Only promote after soundness, true completed-source WallTime, RSS,
negative-control overhead and independent held-out QuPRS/irregular
CNF sources qualify. Grover5 remains counted as a failed or
censored holdout if the finite Rust node cap is reached.
