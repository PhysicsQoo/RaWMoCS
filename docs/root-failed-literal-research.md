# Research-only exact root failed-literal preprocessing for signed Complex WMC

Status: isolated opt-in research branch, no production solver or QuPRS
default changes. Input remains the **original pinned QuPRS Weighted CNF
and signed complex literal weights**. No QASM/Path-sum/ANF
representation switch is allowed in the solver benchmark.

## Count-safe inference

For each candidate literal L from the current source-Weighted CNF,
perform bounded Boolean unit propagation on CNF F with the *temporary*
assumption NOT L. Only when a **Boolean clause contradiction** is
actually derived may the solver add the **entailed** root unit clause
L. In notation: F and (not L) unsatisfiable by unit propagation
implies F entails L. Adding F's own logical consequence produces
exactly the same satisfying-assignment set: there is no new model,
missing model, projected variable, or duplicated weighted factor.
The existing Rust solver still processes any added unit once through
ordinary WMC propagation with its original signed MPFR literal weight.

If both L and NOT L lead to proven contradiction, F is Boolean
UNSAT and logically entails the empty clause. A numerical zero
Complex WMC is NEVER considered Boolean unsatisfiability.

This inference needs **no external SAT model-count oracle** to justify
that adding a proved consequence is WMC-safe. Nevertheless,
the implementation must be independently tested with brute-force
small exact dyadic Full Complex WMC/SAT oracles, including signed,
imaginary, cancellation, UNSAT and SAT-but-zero tests.

## Bounded / fail-closed research envelope

- Invoke only with both --weighted-equivalence and --trail-count,
  via the additional CLI flag --root-failed-literal, after original
  weighted-equivalence SCC reduction and before the existing Trail
  solver constructs its clauses.
- Examine at most 32 highest-frequency variables in the surviving
  source CNF scope; both truth values are probed for each variable.
- Limit to 4,096 original variables, 50,000 clauses, 200,000 literal
  incidences, and 250,000 actual Boolean literal inspections per
  source instance. If a resource boundary is reached, retain
  **only already proved** entailed units and mark capped; never
  assert a unit from an incomplete probe.
- No unit proved means no new Problem clone or residual rewriting.
  All positive/negative complex literal weights and the exact
  source-variable scope remain untouched.
- Record eligible/probed candidates, independent refutations,
  implied units, inspected literal work, contradiction and capped
  status as Rust-owned diagnostics. Source generator time is excluded
  from timed solver execution, but root proof preprocessing is
  INCLUDED in complete native process WallTime.

## Paired native evidence

The source-locked Feynman and MQT experiments compare three separate
fresh-child native arms: unchanged original GPMC C++, unchanged Rust
Inline Full, and same Rust Inline Full + explicit failed-literal
root proof. GPMC decimal digits 30/100 pair with the Rust pinned MPFR
100/333-bit conversion; all arms consume the exact same emitted CNF
bytes within each trial. Each is bounded by 30s and 2GiB process
address space, with observed max RSS and full WallTime, plus fixed
50k Rust component-cache and 1m node budget for hard cases.

- Feynman: HWB6, Barenco5, ModMult55 (seven order-rotated fresh
  trials at each precision), plus one explicit costly Grover5 probe;
  six completed precision cohorts required.
- Original MQT source holdout: W-state4, VQE4, QNN4, Grover4,
  QWalk3, QAOA7 and QPE-exact8 from unmodified pinned QASM.
  Seven order-rotated fresh trials per source×precision cohort
  with all 14 completed cohorts required for full holdout.
  These are QuPRS-encoded original MQT QASM, not MQT-published CNFs.
- Every source must record original QASM and CNF SHA, SAT separately
  from real+imag Complex WMC, DPLL nodes, cache hits, component splits,
  proof work and implied units, stdout hash, p50/p90 complete-process
  WallTime and peak RSS, plus all censored/negative results. A
  failed or missing source is retained as a fail-closed failure,
  not removed from the denominator.
- Numeric agreement between GPMC and Rust at matched effective
  precision is **not** a separate independent exact Full WMC
  certificate for large examples; small brute-force Gaussian/dyadic
  models supply the independent oracle.

## Scientific Go/No-Go

First require exact SAT and signed Complex WMC parity, and show
nonzero entailed units for interesting source examples. If no source
yields proof units, this is an overhead-only negative result.
For a performance promotion, show repeatable lower native
**full-process WallTime and DPLL nodes** on representative
high-cost original Weighted CNFs, not just smaller synthetic CNF
fixtures; negative control source families must not regress.
Prioritize original QNN4 and Feynman HWB6/Grover5, but do not
overfit source families or silently increase node/memory budgets.

If root probing is too expensive or does not find useful units,
**reject as a default** and move to exact multi-variable
separator/cofactor DP or carefully proven weighted elimination,
not SAT-only equisatisfiable rewriting. Keep all changes in a
Draft research branch pending complete final-HEAD CI.
