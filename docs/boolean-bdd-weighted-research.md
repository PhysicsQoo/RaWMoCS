# Research-only Boolean ROBDD before signed-complex WMC

Status: **implementation PoC, independent native acceptance required**. This is
an isolated solver representation experiment, not a production RaWMoCS or
QuPRS default and not a claim of new BDD theory.

## Motivation from pinned original-CNF experiments

In the six-arm, precision-matched, source-locked study
[GitHub Actions #44215456450](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44215456450),
HWB6/100-bit GPMC/Inline/Dense MinFill/ADD1×/ADD2×/ADD4× p50 full
WallTime was 229.080/614.627/641.500/643.985/649.305/669.000 ms.
At 333-bit, it was 253.638/646.580/678.050/677.989/685.042/697.645 ms.
All ADD arms hit their finite **unique terminal** caps (4,096 / 8,192 /
16,384 terminals) and fell back. More memory did not produce a completed
algebraic ADD result. Do not infer performance trends across independent
CI runners from these values.

This PoC tests a *different representation*: perform canonical structural
compression of the **Boolean** source CNF before materializing any weighted
complex terminal. The BDD leaves are only False and True. Weighted MPFR
arithmetic occurs in a separate memoized DAG traversal.

## Algebra and correctness obligations

Let the unchanged QuPRS-emitted Weighted CNF over scope
\(X=\{x_1,\ldots,x_n\}\) be \(F\), with signed complex weights
\(w_i(0), w_i(1)\) attached to every original positive and negative literal.
The target is

\[
  \mathrm{WMC}(F)
  = \sum_{x\in\{0,1\}^{n}}\mathbf1[F(x)]
      \prod_{i=1}^n w_i(x_i).
\]

The companion SAT channel is purely Boolean:
\(\mathrm{SAT}(F)=\exists x\,F(x)\). It cannot be derived from
\(\mathrm{WMC}\ne0\), because opposite complex terms may cancel.

For a reduced ordered Boolean BDD node at rank \(r\), the low and high
edges select \(w_r(0)\) or \(w_r(1)\). Each skipped rank \(j\) between
parent and child **must** contribute
\(w_j(0)+w_j(1)\) to the resulting product. The same rule applies to
variables skipped before the root and after a True terminal. False
terminal contributes UNSAT and complex zero; True terminal contributes
SAT and multiplicative identity before skipped-rank multiplication.
Memoizing the weighted result for each BDD node is valid because its
suffix variable scope is fixed by the global BDD variable order.

All original source clauses are conjoined as Boolean factors. There is
no QASM/Path-sum rewrite, CNF renumbering, alteration of signed weights,
or root formula mutation in this experiment.

Only the *algebraic* equivalence is exact. The two MPFR modes use the
same source-pinned digits→bits policy as original GPMC (30→100 bits,
100→333 bits), but variable order and arithmetic association may differ;
therefore agreement on arbitrary rotations must be tested on both real
and imaginary channels under the established precision-aware criterion,
rather than requiring bitwise equality from independent association
orders. Dyadic small examples have a separate exhaustive assignment
oracle and independent original Trail cross-check.

## Competing BDD ordering policies

- **Natural**: increasing source scope variable IDs. Do not reinterpret
  source literals.
- **Reverse min-fill**: the reverse of the bounded primal-graph min-fill
  elimination order. Compute this solely from original clauses. This
  heuristic is a candidate, not a theorem of BDD-size optimality.

Both use the same restricted Boolean ROBDD builder (hash-consed internal
nodes, AND Apply cache, reduced identical children). Both use the same
MPFR WMC evaluator, independent SAT channel and resource budgets.

## Admission and fail-closed behavior

Hard finite bounds on *this research arm only*:

| Quantity | Limit |
|---|---:|
| Original variable IDs | 4,096 |
| Active scope variables | 320 |
| Source clauses | 3,000 |
| Clause width | 12 |
| BDD nodes (including True/False) | 50,000 |
| AND Apply calls | 1,000,000 |
| Hash-consing and Apply/evaluation caches | 160,000 each |
| Weighted BDD evaluation calls | 180,000 |

If eligibility or a resource bound fails, return no BDD answer, discard
all partial compiled state, and run the original unchanged Trail path on
the full weighted source CNF with existing node/time limits. A fallback
is **not** a BDD solution and its compilation cost is included in full
process WallTime. No dense Factor-VE bag-width cap is changed.

## Native acceptance

The independent workflow
[\`cnf-boolean-bdd-paired.yml\`](../.github/workflows/cnf-boolean-bdd-paired.yml)
pins the same original QuPRS and unchanged GPMC source SHAs as the prior
ADD studies. It generates original Weighted CNFs outside solver timing
but times each *solver process* from launch to termination, including
parsing, structural planning, BDD compilation, MPFR evaluation, any
rejection/fallback and output.

Six competing native arms on one GitHub runner:
GPMC C++, Rust Inline, Dense Factor-VE MinFill, bounded algebraic ADD,
Boolean BDD Natural, Boolean BDD Reverse MinFill. 100/333-bit MPFR
equivalent precisions; 7 rotated fresh processes for each regular
source×precision cohort; 30-second/2 GiB per child; p50/p90 total
WallTime, observed peak RSS, and all success/censor outcomes.

Required scientific gates:

1. Independent SAT truth and **both** signed complex WMC channels match
   original GPMC on every completed cohort. Preserve unsat, SAT-but-zero
   and free-variable test cases, plus separate brute-force dyadic oracle.
2. On capped BDD runs, the unchanged Trail/DPLL topology matches Inline
   as a fail-closed regression control; never drop negative cohorts.
3. For HWB6, require complete bounded BDD output **and** substantially
   better p50/p90 WallTime than GPMC (with no excessive RSS) before
   claiming this representation solves the original hard-case gap.
4. Grover5 failure or censor remains reported. Independent MQT Bench
   original-CNF holdout is mandatory before enabling any router or
   global default. A source-specific victory alone is not enough.
5. Public PR, files, logs and workflow names contain no private work
   tracking identifiers.

**Stop condition:** If both BDD orders hit node/Apply caps or incur
materially slower full WallTime despite completing small cases, do not
raise limits indefinitely. Move to source-CNF AND/OR decomposition,
dynamic conditioning or structured compilation with bounded evidence.
