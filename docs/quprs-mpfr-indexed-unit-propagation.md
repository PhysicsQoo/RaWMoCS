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
