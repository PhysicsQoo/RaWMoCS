# Primal-graph treewidth lower-bound experiment for exact complex WMC

Research-only, source-identical original Weighted CNF. No production
solver/default or GPMC changes.

## Motivation

Independent five-arm native Feynman [min-fill study](https://github.com/PhysicsQoo/RaWMoCS/pull/116)
completed 76 Rust tests and six precision/source cohorts. For HWB6
the 285-variable weighted-equivalence surviving original CNF has
**candidate bucket size 12** under both min-union and bounded
min-fill; the unchanged exact Factor-VE bucket limit is 11.
Min-fill rejects before multiplying any factors, then executes the
original 73,424-node fallback DPLL. It does NOT establish that every
elimination ordering must have bucket 12.

## New algorithm and what its number proves

Build the primal graph of the **same original clause scopes**. Repeatedly
choose a minimum-degree live vertex, record its current degree,
and contract it into a live neighbor (minimum-degree tie-break,
then stable original variable ID). The maximum degree recorded over
the resulting sequence of graph minors is a **valid lower bound on
treewidth**: edge contractions and vertex deletions produce minors,
and treewidth is minor-monotone. It is NOT necessarily tight.
The bucket-size lower bound is treewidth_lower+1, since the
elimination bucket includes the eliminated vertex.

This independent algorithm has no factor tables, no CNF rewrite,
no weight approximation, and does not change the existing
Factor-VE elimination or SAT/signed Complex WMC semantics. It
runs only under the existing opt-in min-fill research arm.
Native complete-process WallTime **includes the extra bound
computation**, preventing free diagnostic cost.

Interpretation:
- If **minor bucket lower >=12**, a width-11 dense primal
  variable-elimination representation is provably insufficient
  on that specific surviving primal graph, irrespective of
  ordering. The next exact research direction is algebraic
  simplification, decomposition after cofactoring, sparse
  factor/ADD representations, or structural change that
  preserves weighted counting; do NOT increase factor memory caps.
- If **minor bucket lower <=11** while min-fill upper candidate
  is 12, the result is *inconclusive*: stronger bounded
  elimination-order search or certified treewidth decision is
  warranted. Do not claim existence of width-11 order solely
  from a lower bound.
- If graph scope or clause mapping invalid, fail closed;
  no fake certificate. Original solver continues unchanged.

## Verification

Four graph fixtures: clique of 12 (exact lower=11),
four-cycle (lower=2), disconnected/isolated nodes (lower
1/0), and invalid clause/scope fail-closed. The existing
signed Complex WMC/SAT unit and five-arm original Feynman
native tests must remain PASS. The runner checks
`minor_bucket_lower <= minfill_predicted_width` for
completed planner orders and records the bound in every
source×precision matched log. Grover5 censored negative
remains censored. Pinned QuPRS and GPMC, original source
SHA, precision, 30s/2GiB child budgets and all resource
caps unchanged. This is a structural research result, not
a speed optimization claim or production router approval.
