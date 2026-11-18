# Research-only source-CNF cutset conditioning + exact signed-complex Factor-VE

Status: experimental single-cutset implementation, **not the production solver**.

## Evidence-based motivation

The original same-CNF dense Factor-VE branch has a fixed maximum
elimination bucket of 11 source variables. The
[certified primal-minor bound](https://github.com/PhysicsQoo/RaWMoCS/pull/118)
establishes a lower bucket size of 12 for the *unconditioned*
285-variable/803-clause HWB6 residual; changing the elimination order on
that same primal graph cannot lower the bucket to 11.

The original algebraic ADD 1×/2×/4× capacity experiment
[CI #44215456450](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44215456450)
fails to complete HWB6 even at 16,384 unique complex terminals. The
[Boolean ROBDD study #44217141041](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44217141041)
also fails to compile all six original source×precision cohorts within
the bound of 50,000 Boolean BDD nodes, even on the easy two families.
Those are **negative algorithm measurements**, not correctness failures
of the original source-CNF signed WMC.

This experiment attempts a different, testable structural hypothesis:
**conditioning one carefully chosen original variable may lower both
cofactored primal graphs enough for the existing bounded min-fill
Factor-VE without widening any dense bucket**. It is not a claim that
such a variable exists for HWB6.

## Exact identity (two channels)

For the original weighted CNF \(F\), source scope \(S\) and some
\(x\in S\), let \(F_0=F|_{x=0}\) and \(F_1=F|_{x=1}\), with all original
clauses simplified only by the indicated Boolean assignment.

\[
\mathrm{WMC}(F)= w_x(0)\,\mathrm{WMC}(F_0)
                 + w_x(1)\,\mathrm{WMC}(F_1).
\]

The remaining scope of **both** branches is \(S\setminus\{x\}\), even
if some other variables disappear from their residual clauses.
Every remaining variable retains its original positive/negative signed
complex literal weights, exactly once. The two branch WMC values are
evaluated by the already verified bounded dense min-fill solver,
*without* modifying its bucket, cell or product limits.
The removed variable's weight is multiplied back once outside the
branch solver. Do not count it twice.

SAT is checked separately:
\[
\mathrm{SAT}(F)=\mathrm{SAT}(F_0)\lor\mathrm{SAT}(F_1).
\]
A satisfying formula with complex WMC zero is **still SAT**; a truly
contradictory cofactor (empty clause) contributes false and complex
zero regardless of other free variables' weights.

The identity is mathematically exact. Independently rounded MPFR
operations may associate differently from the original GPMC C++
reference; 30/100 digits vs 100/333 bits and both complex real/imag
channels must be verified with the pinned, precision-aware numerical
criterion. Small dyadic examples use an independent exhaustive
assignment oracle and original Trail cross-check.

## Bounded candidate selection

1. Validate the original source CNF, weights, active scope and finite
   research admission envelope. No input CNF rewrite or QASM
   translation is performed inside the solver.
2. Run the **existing** bounded min-fill planner with the unchanged
   11-variable dense bucket cap. If already feasible, use the existing
   dense implementation unconditionally; do not spend a cutset branch.
3. On the first infeasible planner bucket, retain its source-variable
   witness (the current vertex plus live neighbors) and prioritize
   those original variables by descending source occurrence count.
   Fill remaining slots from the global highest-occurrence variables.
4. Consider **at most 16** original variables. For each, form both
   source clause cofactors and preflight the *same* 11-variable
   min-fill cap. A direct empty clause proves that cofactor UNSAT.
5. Execute at most **four** costly dense branch solves across all
   considered candidates. **Accept only if both cofactors are fully
   solved** within the original dense width, cell and product limits.
6. Otherwise drop the partial result and run the **unchanged original
   Trail** solver on the original full CNF. Include all planning,
   rejected candidates, partial work, fallback and output in full
   WallTime. A fallback is a negative cutset outcome, not a solved
   cutset benchmark.

Source variable IDs are unchanged; the cofactoring is an exact
original-CNF operation, not a source-circuit transformation.

## Native acceptance

The dedicated
[\`cnf-cutset-ve-paired.yml\`](../.github/workflows/cnf-cutset-ve-paired.yml)
uses the same pinned original QuPRS source and unchanged original GPMC
C++ source as prior studies. The generator and output CNF hash are
source-locked outside solver timing. Each solver process is launched
under identical 30s/2GiB limits with the same original Weighted CNF
bytes and signed complex weights.

Seven native arms, rotated across seven independent fresh processes:
GPMC, Rust Inline, Dense MinFill, algebraic ADD, Boolean BDD Natural,
Boolean BDD Reverse MinFill and the new conditional Cutset-VE.
100/333-bit comparable MPFR precision. Report native p50/p90 total
WallTime, observed peak RSS, complete SAT plus both real/imaginary WMC
agreement, source SHA, successful cut variables, planned branch
widths, rejections, fallback parity, and all censored hard cases.

GO for hard-case routing only if **HWB6 both conditioned branches
complete**, original GPMC-comparable complex WMC holds, and full
WallTime/RSS meaningfully improve versus the same-run original C++
baseline. A small proof-of-concept clique12 success is necessary
correctness evidence but *not* HWB6 success. Independent original MQT
holdout is required before considering any default policy change.

No private tracking identifiers appear in this public design. No
changes to QuPRS, original GPMC, RaWMoCS production core or default
CLI are authorized by this PoC.
