# WMC-safe equivalent-literal elimination from original Weighted CNF

**Status: research-only, opt-in; formal production routing not changed.**

## Input and algebraic proof

This branch uses only the **original Weighted CNF bytes**, as emitted
by QuPRS and consumed unchanged by original GPMC. QuPRS owns QASM,
Path-sum, RR and CNF encoding; RaWMoCS does not import alternative
native ANF or perform any circuit preprocessing.

The research Rust MPFR option `--weighted-equivalence` constructs the
binary implication graph from the original CNF: each (a OR b)
contributes (!a => b) and (!b => a). Strongly connected literals imply
exact logical equivalence. Two literals with the same SCC must have
identical truth values in **every satisfying assignment**. When a
variable and its negation are in one SCC, the source formula is
Boolean UNSAT (not merely a complex WMC cancellation).

For each SCC pair representing a signed-equivalence class, retain its
smallest original variable r. Substitute every member v as either r or
!r, as certified by SCC orientation. Delete only Boolean-tautological
or duplicate clauses **after this equivalence**, and exclude eliminated
members from the solver's active variable scope.

The critical WMC correction: if class K has members with signs s_v,
set the representative's true/false complex literal weights to

    W_r(true)  = product over v in K of W_v(truth(v)|r=true)
    W_r(false) = product over v in K of W_v(truth(v)|r=false)

using the same MPFR precision and four-separated-products complex
arithmetic as the research GPMC-parity solver. The removed variables
are *not* multiplied as independent free variables again. Each
complete original satisfying assignment maps bijectively to exactly
one reduced model and has the same product of signed complex weights.
This covers nonuniform, negative, zero and complex weights; no
division, normalization-by-zero or SAT-only existential rewrite.

Graph SCC uses iterative Kosaraju. Caps: at most 4096 variables,
50000 CNF clauses, 150000 implication edges. On admission failure
return the **original full CNF, original weights and original scope**;
the existing original Frequency / Cache DPLL remains unchanged unless
the research flag is given. The optional preprocessing is charged
inside native solver WallTime; no hidden QASM/RR cost is assigned to
the RaWMoCS algorithm.

## Independent science

- Small complete model-set and signed complex weighted truth-table
  oracle at 100/333 MPFR bits, mixed regular/nonregular CNF and
  variable-equivalence chains including negated relationships.
- SAT with mathematically cancelled 0+0i WMC remains SAT, including
  eliminated variable compensation.
- SCC contradiction produces Boolean UNSAT with zero WMC;
  negative and resource-cap controls return unchanged original.
- Other independent baseline MPFR and rust-core science / CodSpeed
  identifiers stay unchanged.

## Same-CNF experiment

Workflow: `.github/workflows/cnf-equivalence-paired.yml`.
Runner: `benchmarks/rust_core/tools/quprs_cnf_equivalence_ab.py`.

Pinned QuPRS original CNF corpus from
`0b28736651b3740ed3b201edd2efdef3da541e0d`
(HWB6, Barenco5, ModMult55, Grover5), unchanged GPMC C++ at
`89bbc89d0de918e7a5385740355df55765039230`.
All three solver arms receive identical per-case Weighted CNF bytes:
GPMC C++, Rust Frequency+MPFR control, and Rust Frequency+MPFR
equivalence elimination. Matched 100/333-bit precision; three
order-rotated fresh subprocesses on the same Ubuntu runner for
completed cohorts, 30s and 2GiB virtual address-space guard;
collect full process WallTime, p50/p90, RSS, DPLL/cache/component,
source hashes, SAT and both Full Complex WMC components. Grover5
gets one bounded all-arm pilot at 100 bits even when the Rust baseline
is censored, to avoid suppressing a potential completed candidate.

**Admission decision depends on results:** a smaller CNF after SCC
substitution does not automatically imply faster solver WallTime or
better numerical conditioning. Preserve negative cohorts and
CENSORED node/time/memory outcomes; do not use partial failed
times in speed ratios or change production defaults.