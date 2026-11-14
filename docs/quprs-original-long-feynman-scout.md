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

## First original four-case source scout and follow-up

[First bounded native run #43594920329](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43594920329)
successfully emitted all **four original unmodified Feynman QASMs**
and verified original QASM/CNF/phase provenance.

| Source case | CNF vars / clauses | GPMC 100-bit median | Rust research 100-bit | Scientific status |
| --- | ---: | ---: | ---: | --- |
| Grover5 | 607 / 1,121 | 16.618 s | 100k node cap | `CENSORED_NODE_BUDGET` |
| HWB6 | 297 / 827 | 135.579 ms | 100k node cap | `CENSORED_NODE_BUDGET` |
| Barenco Toffoli5 | 97 / 180 | 11.203 ms | 17.387 ms | `SOLVER_AGREEMENT_ONLY` |
| Modular Multiplier55 | 91 / 192 | 10.440 ms | 24.240 ms | `SOLVER_AGREEMENT_ONLY` |

The QFT4 cohort was not large enough to characterize actual
multi-second QuPRS workloads. Both Grover5 and HWB6 have a
**known node-cap stop** from the research-only Rust MPFR engine,
not an observed native timeout or UNSAT verdict.

This follow-up introduces only two **explicit and finite** research
flags: `--node-budget=250000` and
`--node-budget=1000000`. The original 100,000-node default
remains exactly unchanged. Every solver process still has
30 seconds and 2 GiB virtual address space. Each higher cap
is tested twice. If Rust successfully completes under a finite
cap, two new alternating native GPMC/Rust same-100-bit
process comparisons are performed against the identical
original weighted CNF, with separate SAT and predeclared
numerical agreement gates. If no cap completes, the case
is explicitly **`CENSORED_NO_SAFE_COMPLETION_UP_TO_1M_NODES`**.

Neither source coherence nor floating agreement alone implies an
independent exact Full WMC certificate for 297/607-variable CNFs.
No private tracking IDs or solver default changes enter the
public repository.
