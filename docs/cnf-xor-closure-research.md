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

## Adoption gate

No production default or automatic router change is warranted on
synthetic XOR demonstrations or a single source-specific speedup.
Only promote after soundness, true completed-source WallTime, RSS,
negative-control overhead and independent held-out QuPRS/irregular
CNF sources qualify. Grover5 remains counted as a failed or
censored holdout if the finite Rust node cap is reached.
