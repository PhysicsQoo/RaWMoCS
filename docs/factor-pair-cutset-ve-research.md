# Original weighted CNF: bounded two-variable Shannon cutset

Status: **Research-only / Draft**. Do not enable in production or the
default MPFR solver.

## Why a new representation?

Pinned same-input, same-precision 7-process experiments reject:
- dense Factor-VE width 11 on HWB6 (certified primal-minor lower bucket 12),
- ADD at 1×/2×/4×: all HWB6 profiles hit exact-terminal quotas,
- Boolean ROBDD: node cap and fallback,
- one-variable Cutset: the first 24 degree-ranked candidates had no
  admissible **both-polarities** width-11 cofactors. On the matching
  100-bit CI, full p50 was 1,201.151 ms, versus original GPMC 177.800 ms
  and Rust Inline 504.811 ms. This is a decisive negative result.

The next hypothesis is intentionally narrower than a general
treewidth-modulator search: two conditioned source-CNF variables might
remove enough of the width-12 obstruction to make all four residual
scopes fit the existing dense Factor-VE width-11 gate.

## Algebra and weight ownership

For **unchanged source** CNF F, original source literal weights w and
conditioned variables x, y:

W(F) = Σ_{a,b ∈ {0,1}} w_x(a) w_y(b) W(F|x=a,y=b).

SAT(F) = ∨_{a,b} SAT(F|x=a,y=b).

The original CNF is read from the same byte-identical QuPRS-generated
Weighted CNF as original pinned GPMC C++. Conditioning is **internal
solver algebra**, not a QASM, ANF or Path-sum preprocessing substitution.
The two cut variables are removed from each cofactor's residual scope
and their source +/- literal weights are accounted **exactly once** in
the branch prefix. All remaining variables retain the original
signed-complex weights (including free variables); SAT is independent
of WMC cancellation.

Each complex multiplication/addition occurs at the requested pinned
MPFR precision. Finite precision is not mathematically associative;
therefore independent small dyadic-oracle equality and real/imag
tolerance checks versus GPMC are both required.

## Strict admission and fail-closed behavior

- Only explicit research CLI \`--factor-pair-cutset-ve\`, requiring
  source weighted-equivalence and original Trail mode.
- Original source envelope: <=4096 named variables, <=320 surviving
  variables, <=3000 clauses; original min-fill width >11 but <=14.
- Rank by original primal degree, deterministically; at most 12
  variables and 24 ordered pairs; 180,000,000 ns **planning** budget.
- To avoid four expensive preliminary numeric solves for every rejected
  pair, delete both candidate vertices from *every clause* without
  deleting any surviving edge solely due to a satisfied literal. This
  defines a supergraph of each of the four true cofactors. A min-fill
  width <=11 for that supergraph provides a sufficient structural
  certificate that all four branch graphs have a width-11 elimination
  order. It is not a necessary test, so its rejection means merely
  **no admitted candidate within this heuristic**, not proof that no
  two-variable cutset exists.
- For an admitted pair, construct each exact Boolean cofactor from
  original clauses and solve all four with the existing bounded
  min-fill dense Factor-VE; do not partially reuse a failed branch.
- Any failed cap, empty source guard, non-admitted candidate, branch
  rejection or elapsed planning budget abandons all results and calls
  the unchanged original weighted Trail from the same source.
- Both failed attempts and eventual fallback remain in complete
  process WallTime; no positive-only reporting, no estimated timeout
  wins.

## Acceptance gates

1. **Independent oracle**: synthetic 13-variable 78-clause complete
   primal graph. Original min-fill requires bucket 13, so no
   single-vertex deletion can attain width 11; deleting two leaves
   width 11. Count all four signed-complex dyadic weighted cofactors
   at 100/333 bits. Compare against independent exhaustive 2^13
   assignment enumeration and unmodified Trail; keep SAT distinct
   from signed complex count.
2. **Fail-closed**: an original small width<=11 CNF never enters the
   costly pair branch and retains Trail correctness.
3. **Same original Feynman CNFs**: original pinned QuPRS encoder and
   unchanged original GPMC; all arms on same runner and machine at
   matched 30/100 decimal digits vs 100/333 MPFR bits. Seven
   order-rotated fresh-process samples per regular case × precision.
   Report complete process p50/p90 WallTime, measured peak RSS, source
   SHA and both signed complex parts. Grover5 remains a documented
   censored negative; do not delete it.
4. **No-Go hard cases**: if HWB6 pair-search is rejected, resource
   capped, inconsistent, or slower than original GPMC, retain the
   negative outcome. A synthetic positive proves only exact
   structural correctness; it is not enough to change solver routing.

The research branch is independently opt-in; the production solver,
QuPRS source encoder, source CNF, GPMC original executable and
default solver choice are unchanged.
