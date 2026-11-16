# CodSpeed: original QuPRS Weighted CNF Rust MPFR profiling

**Status:** research instrumentation only, not a speedup or production change.

## Why the historical CodSpeed runs cannot answer the GPMC gap

The existing 68 RaWMoCS CodSpeed identities measure the independent
`rawmocs-core` exact Gaussian-rational solver and small direct-CNF
fixtures. They **do not invoke** `benchmarks/mpfr_research`, the
`rug::Float` MPFR engine used in matched-original-CNF GPMC comparisons.
For example, BigUint/BigRational GCD hotspots from formal-core
flamegraphs cannot be attributed to MPFR source solver recursion.

The separate original GPMC repository's six source-pinned direct-CNF
simulation benchmarks are tiny, with five syscall-dominated cases.
Their simulation time cannot be divided into the Rust-core simulation
to infer full weighted-CNF performance.

The original Weighted CNF 100-bit native paired science
[run](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43836369743)
is the correct performance baseline. Seven fresh balanced processes:
HWB6 GPMC 192.328ms, Rust + exact equivalent-literal 641.863ms;
ModMult55 GPMC 14.386ms, Rust equivalent 22.209ms and unit-closure
18.494ms; Barenco5 GPMC 14.531ms, Rust equivalent 9.999ms,
closure 10.672ms. The six completed cohorts had same precision,
same original input and Full Complex WMC numerical agreement.
Grover5 Rust remains censored by its million-node budget.
Do not mix these native times with CodSpeed simulation numbers.

## New exact source and profiler

`codspeed.yml` defines 12 **new, non-overlapping** exec-harness
benchmark identities: three original QuPRS Feynman Weighted CNF sources
(HWB6, ModMult55, Barenco5) × four independent native solver arms
(unmodified GPMC, Rust Frequency, Rust exact signed equivalent-literal
elimination, Rust equivalent literals plus bounded Boolean unit closure).
All use effective 100-bit MPFR (GPMC 30 decimal digits and Rust
30→100 binary bits), the identical original CNF file bytes, and the
same Linux CI runner.

Before starting CodSpeed, the workflow pins:

- QuPRS original `0b28736651b3740ed3b201edd2efdef3da541e0d`;
- unmodified GPMC C++ `89bbc89d0de918e7a5385740355df55765039230`;
- all three emitted weighted-CNF SHA-256 digests, independently
  recorded in `docs/cnf-equivalence-research.md`.

The original QuPRS QASM→PathSum→CNF exporter is built and run **outside**
the profiler. The GPMC and Rust executables are also built outside
the profiler; each CodSpeed sample invokes a fresh native process,
including program startup, parsing, solver-owned preprocessing,
Full Complex WMC and output. No QuPRS encoder acceleration is credited
to RaWMoCS. The original Rust 50k cache, CSR-after-8192, packed-fast
and frequency scratch setting is fixed across its three research arms.

### How to read the resulting profiles

- CodSpeed **Simulation** estimates CPU cost and provides call trees.
  It is not directly comparable to native WallTime. Do not declare
  a solver victory from simulation alone.
- GPMC C++ and Rust may have different syscall/startup/cache-init
  fractions. Inspect `Engine::count`, `normalize_child`,
  `unit_propagate_indexed_csr`, `split_components`,
  `assign_fast`, `rug::Float`, memory allocation and
  Hashbrown key lookup **in the original MPFR executable**.
- Compare a new Rust candidate with its unmodified control within
  the **same source, precision and instrument mode**, preferably
  separate cost-equivalent constraints. A flamegraph shows which
  operations are hot, not how many entire WMC subtrees an alternative
  algorithm would avoid.
- Confirm any proposed change by re-running CodSpeed at the candidate
  commit and by fresh original same-CNF process WallTime/RSS and both
  complex WMC components, including negative/censored cases.
- Never silently replace the existing 68 benchmark identities or
  modify the production/default solver, QuPRS or upstream GPMC.

The CodSpeed exec harness has a bounded sampling time per case;
a skipped, failed, over-budget or missing profile must be reported,
not interpreted as a fast result. Workload selection is intentionally
limited to 100-bit first to make high-cost source profiling feasible.
333-bit and hard Grover5 remain required in later native science
before **any** default adoption.
