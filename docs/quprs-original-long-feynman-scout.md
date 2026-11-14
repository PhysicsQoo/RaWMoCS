# Original QuPRS Feynman large-case bounded source scout

**Research only.** This is an independent source discovery and
same-precision native-comparison experiment, not a claim that a source
is quantum-equivalent or an independently certified complex Full WMC.

## Original immutable source

Use QuPRS original commit `0b28736651b3740ed3b201edd2efdef3da541e0d`,
the same unchanged `PathSum / WmcManager` API as the source-qualified
QFT4, and unchanged committed QASM bytes from:

- `benchmarks/Feynman/mod_mult_55.qasm` (9 qubits)
- `benchmarks/Feynman/barenco_tof_5.qasm` (9 qubits)
- `benchmarks/Feynman/hwb6.qasm` (7 qubits)
- `benchmarks/Feynman/grover_5.qasm` (9 qubits)

No gate decomposition, transpilation, source editing, synthetic
substitution or changed weight precision is allowed. Exporter
startup, parse, encode and output are limited to 30 seconds, 2 GiB
virtual address space and 2 MiB maximum individual file size.
Every attempt is retained as `EXPORTED` or
`EXPORT_CENSORED_OR_UNSUPPORTED` with a raw diagnostic log.
Only an intact exported QASM/CNF/phase tuple whose bytes match
the original source and dual dialect is eligible for solver testing.

## Matched-MPFR native scout

The unchanged pinned GPMC C++ source at
`89bbc89d0de918e7a5385740355df55765039230`
competes with the research-only Rust MPFR CLI using both previously
qualified opt-in `--fast-assign --hashbrown-borrowed` controls.
Both solvers receive exactly the same GPMC-dialect weighted CNF,
initially at 100-bit (30 decimal digits), two alternating
fresh-process trials each, 30-second wall and 2-GiB virtual-address
bounds. Only cases that finish and agree numerically at 100 bits
progress to the 333-bit comparison.

A timed-out or failed solver has a **censored** record, not an
infinite/zero runtime or implied performance rank. A completed
cross-solver pair is labeled **`SOLVER_AGREEMENT_ONLY`**, not
`CERTIFIED_WMC`; a separate independent exact oracle for these
larger sources is absent. Numerical admission uses a predefined
absolute/relative guard at equal MPFR mantissa precision and a
separate SAT verdict. Process WallTime includes launch, parse,
preprocessing, solve and output. Preserve peak RSS, literal/phase
provenance and all raw trials.

The goal is to discover workloads with meaningful multi-second
search costs under real QuPRS encoding, rather than optimize
microsecond synthetic fixtures. Continue with longer cases only
when source and numerical admissibility are established.

Public repository text contains no private work-tracker IDs.
