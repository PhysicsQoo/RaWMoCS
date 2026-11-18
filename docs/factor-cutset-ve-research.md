# Bounded source-CNF Shannon cutset + dense Factor-VE (research-only)

## Motivation / evidence

In pinned original QuPRS Feynman CNF paired native comparisons, the exact
signed-complex ADD did not solve the surviving HWB6 285-variable graph even at
1×, 2×, or 4× terminal quotas. The original dense Factor-VE also fails the
11-variable bucket limit because the certified primal-graph minor lower bound
forces at least a 12-variable elimination bucket on the unchanged graph.
The experimental Boolean BDD approach also reached its node cap on hard and
some smaller Feynman cases. Increasing caps without reducing structural work
is not an accepted optimization.

## New research hypothesis

A bounded **source-only** Shannon cutset can remove the obstruction while
preserving the exact signed complex weighted model-counting equation:

W(F) = w(-x) * W(F|x=0) + w(+x) * W(F|x=1),

SAT(F) = SAT(F|x=0) OR SAT(F|x=1).

All weights are taken from the original solver-input CNF; the conditioned
variable is omitted from both residual scopes. Boolean satisfiability is
independent of the complex sum, so SAT-but-zero is preserved. Neither the
QuPRS source encoder, the original CNF input bytes nor source literal weights
are regenerated. The internal cofactor clauses and both original +/- weights
are included in the solver WallTime.

The experimental \`--factor-cutset-ve\` option first measures the primal
min-fill bucket of the surviving original source CNF under the unchanged
dense Factor-VE max-bag 11. Only original graphs rejected for exceeding that
width and no greater than the explicitly bounded root bucket 16 enter the
cutset probe. It considers at most 24 surviving original variables, sorted
by original primal degree; each candidate is conditioned both ways and is
admitted **only** when both cofactor graphs have a pre-verified min-fill
upper bucket at most 11. Exactly two independent unchanged bounded dense
Factor-VE operations are then attempted. Any source envelope, planner,
product, memory, factor-cell, or other cap discards partial results and
continues through the **unchanged original** Trail fallback.

## Correctness and independent witness

- Synthetic full 12-variable clique induced by 66 original pairwise CNF
  clauses: original dense factor width 12 rejects; single-variable
  Shannon conditioning produces width-11 cofactors and must match
  independent exhaustive 2^12 assignments at both 100 and 333 bits.
- Signed complex dyadic literal weights, with SAT and real/imag checked
  independently. Full-rank original Trail comparison also required.
- Graphs fitting original dense width 11 must fail the opt-in cutset
  admission and preserve original fallback without a false success.
- Run original Feynman weighted-CNF four-family native holdout with
  original pinned QuPRS and unchanged GPMC, seven order-rotated
  fresh-process trials at each regular case × precision, 30s child
  runtime and 2GiB address-space limits. Preserve p50/p90 full WallTime,
  peak RSS, both signed complex fields, SAT, all source/input hashes,
  cutset candidate counts, branch products and all censored outcomes.

## Go/no-go

Go requires a complete newly solved HWB6 hard case with correct SAT and both
complex WMC channels at equal precision and an empirically useful full
WallTime/RSS ratio against the *same batch* original GPMC. A still-capped
cutset is a **negative** outcome even if its structural pilot is correct.
Other original Feynman and independent MQT holdouts are required before any
default router or production change. This branch is draft, opt-in, and not
release evidence.
