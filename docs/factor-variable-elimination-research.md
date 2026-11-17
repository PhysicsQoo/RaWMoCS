# Exact signed-complex factor variable elimination on original Weighted CNF

Status: research-only Draft; NO production Rust/main or QuPRS default change.

## Motivation and negative controls

[PR #108](https://github.com/PhysicsQoo/RaWMoCS/pull/108) source-identical
GPMC C++ feature ablation found QNN4 was 26.9%/22.4% slower when TD was
disabled, but this does NOT imply any arbitrary structural search
ordering helps. [PR #109](https://github.com/PhysicsQoo/RaWMoCS/pull/109)
tested source-primal min-degree bag-centroid branching and passed 14/14
original MQT exact SAT/signed Complex WMC cohorts. Yet at 100-bit
equivalent precision QNN4 full-depth scoring rose from **6,055 to
29,581 DPLL nodes** and from **31.997 to 184.104 ms native WallTime**;
W-state4 rose from 938 to 15,097 DPLL nodes. Root-only also did not
improve QNN4. This is a clear no-go for that heuristic.

This independent experiment tests **a different algebraic algorithm**:
exact variable elimination in a bucket/factor join-tree using original
weighted CNF clauses, not another DPLL pivot score. Each original CNF
clause becomes a Boolean factor, and each surviving variable's
original positive/negative complex weight becomes ONE unary factor.

Given bucket variables S and target variable x, construct:
  F_new(S \ {x}) = SUM_(x=0,1) PRODUCT_(f contains x) f(S,x).

The factor values carry **two independent channels**:
- Boolean SAT: AND on product, OR when summing assignments;
- signed Complex MPFR WMC: multiply factors and add the two
  assignments, without forcing SAT=false when Complex WMC is zero.

Thus every surviving variable and each of its weights is summed
exactly once, even when literal weights are negative, imaginary or
cause cancellation. The algorithm neither drops clauses nor performs
a SAT-only equisatisfiable variable elimination. Empty clause, free
variables and disconnected components are handled with the same
factor semantics. The independent brute-force small-model tests
include signed imaginary dyadic weights, SAT-but-zero complex WMC,
unsatisfiable/empty clauses, and disconnected/free components.

## Bounded runtime and fallback contract

Enable only the opt-in research CLI flag
  --factor-variable-elimination
alongside existing --weighted-equivalence --trail-count, which
must retain identical original CNF clauses and signed MPFR weights.
After existing exact weight-compensated equivalence SCC reduction,
build a factor bucket based on the smallest current union scope.
Cap inputs at 256 surviving variables, 3,000 clauses, 12
source-clause variables, 11 bucket variables, 150,000
materialized factor cells and 600,000 factor-product visits.
All bounds are deterministic. Abort the entire unfinished DP on cap
and run the UNMODIFIED original weighted Trail engine on the
same input; never use partially computed factor results.
The work already spent attempting DP is counted in complete
process WallTime. No production algorithm or API is changed.

Report separately: completed/fallback, max bucket size, factor
cells, factor products, eliminated variables, SAT-but-zero
subtotals and actual VE processing nanoseconds. Successful factor
elimination has **no DPLL nodes**. Reporting DPLL nodes=0 is
NOT a direct node-count speedup; compare factor-product operations
separately and make native full-process WallTime and memory the
primary implementation benefit criteria.

## Same-source original MQT evaluation

Freeze original QuPRS-emitted Weighted CNF from 7 pinned MQT QASM:
W-state4, VQE4, QNN4, Grover4, QWalk3, QAOA7, QPE-exact8.
These are NOT MQT-published CNFs. QuPRS pinned commit:
0b28736651b3740ed3b201edd2efdef3da541e0d.
GPMC C++ pinned commit:
89bbc89d0de918e7a5385740355df55765039230.
Check source QASM exact bytes, both emitted CNF dialect
semantics and phase-sidecar SHA. Three arms consume exactly
the same frozen original CNF per source and precision:
original GPMC, unchanged Rust Inline and Rust exact factor VE
or its provably identical fallback. Match original GPMC
30/100 decimal precision to Rust MPFR 100/333 bits as
previously source-locked. Seven rotated fresh native child
processes per source×precision cohort; 14/14 strictly required.
Process limit 30 seconds, 2 GiB address space per child,
observed maxRSS, full WallTime p50/p90, SAT separately from
both signed Complex WMC real/imag, source/CNF/output SHA,
factor operations, DPLL nodes when fallback, and all censored
or negative cases are archived.

## Scientific Go/No-Go

1. Every new factor-semiring result must match both Boolean SAT
   and signed Complex WMC versus unchanged original GPMC plus
   independent small exhaustive dyadic Gaussian WMC oracles.
2. A source exceeding width/table/work bounds must run the
   original Trail unchanged. Any nonidentical fallback search
   topology or lost source is a correctness failure.
3. The most important holdout is QNN4, where GPMC still
   outperforms Rust Inline by ~2x. Full-process WallTime and
   memory must improve on the original QNN4 and survive
   VQE4, W-state4, QAOA7 negative controls. A win on tiny
   synthetic cases or the prior ModMult55-only family
   does not license promotion.
4. Passing CI is correctness, not automatically algorithmic
   speedup or GPMC parity. Reject if real workloads mostly
   fall back or if factor table construction exceeds savings.
   Continue towards certified join-tree/factor ownership or
   efficient incremental factor tables only if real data
   supports it. Keep PR Draft with default untouched.
