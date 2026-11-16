# Original weighted-CNF MPFR: reusable CSR propagation workspace

**Status: opt-in research only.** The official RaWMoCS core, the
current MPFR Frequency default, QuPRS Adapter, GPMC C++ and existing
CodSpeed benchmark identities are unchanged.

## Observed real-CNF CodSpeed bottleneck

[Original-source 12-arm CodSpeed run](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6afaea32af135dad89c57dfb)
at source-pinned 100-bit MPFR measures the **actual separate Rust MPFR
research executable**, not the original BigRational `rawmocs-core`.
Same original QuPRS Weighted CNF and source pins for Rust and GPMC.

On HWB6 in the exact signed-equivalence Rust arm, the 5.6s
**CodSpeed Simulation modeled CPU total** attributes 98.9% to the
`Engine::count` call tree. Re-rooting shows:

- `unit_propagate_indexed_csr`: inclusive 1.4s (24.31%), with
  repeated zeroed allocations, memset, MPFR prefix multiplication,
  rebuilding literal-to-clause CSR and the reduced clauses;
- `assign_fast`: inclusive ~1.0s (18.66%), dominated by new
  residual clause allocation, not pure integer arithmetic.

Function-level inclusive costs are not additive to the self-time
table. This does **not** imply BigRational GCD is the MPFR bottleneck.
Original GPMC HWB6 is 1.7s CodSpeed Simulation, compared with
Rust 5.6s; these are diagnostic CPU simulations, **not** the native
same-runner WallTime ratios.

The prior [same-original-CNF seven-trial native result](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43836369743)
found HWB6 (100-bit) original GPMC 192.328ms and Rust weighted
equivalence 641.863ms; unit closure 644.543ms with 81,594 DPLL
nodes unchanged. The original Grover5 Rust remains node-censored.
Thus the high-cost search topology is an additional unsolved
problem; accelerating CSR alone cannot establish GPMC parity.

## One isolated cost change

The opt-in CLI `--csr-reuse-workspace` activates
`benchmarks/mpfr_research/src/csr_workspace.rs` **only with** an
indexed CSR unit propagation mode. An Engine-owned `Workspace`
retains the eight transient CSR arrays (literal occurrence counts,
clause lengths/alive flags, earliest-unit heap, assignments,
prefix offsets, insertion cursors, and clause indices). On each
propagation call it resets logical state in source order without
reallocating stable capacity or zeroing clause indices that are
guaranteed overwritten by a full occurrence pass. It returns the
arena **before any recursive `Engine::count` call**.

The original `unit_propagate_indexed_csr` remains the baseline
with independent memory and behavior. The optimization never
changes actual CNF/weights, per-clause first-unit selection,
per-literal MPFR multiplication order, SAT/zero distinction,
residual canonicalization, Component split order or Cache keys.
It is not a branch heuristic and must **preserve** nodes,
cache hits, decompositions and complete complex stdout for the
same source and precision.

This option may retain arena memory equal to the largest residual;
peak RSS and cache/arena capacities must be measured, not assumed
lower. An unsatisfied cofactor returns the workspace before
recursion or result storage. The mode remains **disabled by
default**.

## Required science and performance acceptance

1. Rust 100/333-bit exact per-residual parity with the original CSR
   on initial unit forces, chains, explicit Boolean UNSAT and
   satisfiable zero-complex WMC. Reuse the same workspace across
   different cases (including after a contradiction) and verify
   the entire first-unit prefix multiplication order.
2. Repeated full DPLL + Cache capacity parity at 100/333 bits,
   exact complex stdout, original DPLL nodes/cache hits/components;
   no premature control-flow differences or unknown turned SAT.
3. [Research-only CodSpeed 15-arm A/B](../codspeed.yml) adds three
   *new* `rust_equivalence_csr_arena_mpfr100` original-source
   benchmark identities while preserving the prior 12 and all
   pre-existing 68 formal-core IDs. Compare to the old CodSpeed
   run on same source and instrument mode; check full run issues
   and flamegraph. CPU Simulation improvements are a diagnosis,
   not grounds for production adoption.
4. Source-identical QuPRS Feynman HWB6, ModMult55, Barenco5 original
   Weighted CNF from QuPRS SHA
   `0b28736651b3740ed3b201edd2efdef3da541e0d`, original
   GPMC C++ SHA `89bbc89d0de918e7a5385740355df55765039230`.
   Seven rotating fresh native subprocesses at 100/333-bit MPFR
   for all completed cohorts, strict 30s / 2GiB **virtual AS**
   process caps. Include startup/parse/preprocessing/DPLL/output
   in native WallTime, p50/p90, observed max RSS, exact CNF SHA,
   SAT and Full Complex WMC real+imaginary components, solver
   nodes/cache/decomposition and all censored failures.
5. Grover5 gets bounded attempts of every arm. Its incomplete Rust
   computations must **never** be ranked as completed timings.
   Independent large-CNF exhaustive Full Complex WMC mathematical
   certification is not claimed from cross-solver numeric agreement.
6. Production admission requires measurable improvement on long
   source-family **native full process WallTime**, no unacceptable
   memory growth or negative-control regressions, and repeated
   qualification. A subpercent CodSpeed-only gain is insufficient.

Science workflow: `.github/workflows/cnf-csr-reuse-science.yml`.
Same-source native workflow:
`.github/workflows/cnf-csr-reuse-paired.yml`.
Runner: `benchmarks/rust_core/tools/quprs_cnf_csr_reuse_ab.py`.

The structural research question after this study is eliminating
repeated child clause materialization and searching with persistent
residual graph/watch structures, rather than endless allocator
micro-tuning. This is not yet implemented or qualified.
