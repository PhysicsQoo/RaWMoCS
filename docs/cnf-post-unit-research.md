# Exact weighted memoization after Boolean unit closure

**Status: isolated research / Draft-only / opt-in.** The public
`rawmocs-core`, normal MPFR DPLL, QuPRS, original C++ GPMC,
default solver-routing and pre-existing cache semantics are unchanged.

## Measured motivation

Earlier exact component-weight-class cache reuses count-safe
isomorphic subproblems across different source variable IDs, reducing
HWB6 DPLL nodes **81,594 → 73,424**, Barenco5 **2,212 → 1,740** and
ModMult55 **4,269 → 3,879**. The inline-key optimization in
[the 13-arm original-source native experiment](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43992736163)
shaves about 0.5–2.8% full-process WallTime on the unrestricted
weight-class cache, but HWB6 still runs ≈2.6× slower than original
GPMC. Hard Grover5 remains bounded by the Rust node limit.
Another minor allocation/key refinement cannot close this gap.

The existing research DPLL **looks up and inserts its canonical
residual key before Boolean unit propagation**. Two different
pre-closure residual CNFs can propagate to the same residual
subproblem, particularly when their different units have different
literal weights but the remaining weighted component is identical
under exact ordered weight-class relabeling. Cache entries keyed by
pre-unit clauses cannot reuse these states.

This experiment directly changes the **memoized subproblem
semantics** from "before unit closure" to "after unit closure".

## Exact weighted counting invariant

For Boolean formula `F`, assignment `α` and a consistent unit
closure `U` with implied literals `l₁...lₖ`:

```
W(F | α) = (prod_i weight(l_i)) · W((F | α) | U)
SAT(F | α) = SAT((F | α) | U)
```

The cache **stores only the second weighted model count**, plus the
Boolean satisfiability bit. The entire implied signed-complex
multiplicative factor is kept in the local recursive stack frame
and applied after a cache hit/recursive miss. If any implied weight
is exactly zero, `SAT` must still be determined from Boolean
constraints and kept separate from `WMC = 0`. Contradictory
unit propagation produces `UNSAT`, never a numerical-zero SAT.

The scope removes all variables assigned by this node's closure;
the remaining reduced signed clauses are canonicalized exactly.
Original QuPRS CNF source, individual signed positive/negative
Complex MPFR weights, variable polarity, clause deduplication,
exact cache key equality (not a probabilistic hash), unit queue
order, component splitting and frequency pivot remain unchanged.
For source variables in equal weighted classes, local ordinal
IDs use the same ordered exact full-precision weight signatures.
Scoped local keys retain their zero namespace sentinel.

A floating point MPFR evaluation may differ in its final trailing
digits from the pre-closure control because moving the unit prefix
changes the multiplication association. Therefore **do not demand
stdout SHA equality between the old pre-unit-cache and this new
algebraic algorithm** on non-dyadic inputs. Demand SAT and both
Full Complex WMC parts to agree under the existing matched-
precision bound, and demand exact equality against the
independent counting implementation for small dyadic inputs.
Do not compare this with the unrelated exact-rational core's
CodSpeed data.

## One research option

`--trail-post-unit-cache` requires
`--weighted-equivalence --trail-count --trail-parent-ids
--trail-compact-residual --trail-prehashed-cache
--trail-fused-compact --trail-weight-class-cache
--trail-inline-weight-relabel`.
`--trail-cache-scope-max=16` is optional and stays a separate
ablation. Unmodified inline-full and inline16 are the controls.
The post-closure research algorithm lives in a separate function,
and the original pre-unit code path is intact.

Emit `post_unit_hits`, `post_unit_visited`, `post_unit_forced`,
nodes, component decompositions and full native WallTime/RSS.
A newly constructed key after propagation adds work per node;
if it does not produce materially more reuse or fewer downstream
nodes, treat the hypothesis as negative, even if a small synthetic
case improves.

## Acceptance: full scientific provenance

- Source-only signed complex WMC at 100/333 MPFR bits: exhaustive
  SAT/UNSAT, SAT-but-WMC-zero, disconnected components,
  signed weight unit chains, independent DPLL comparison,
  randomized 40-case dyadic CNF corpus and all existing tests.
  Cache limits 0/1/128 guard interactions, trail undo invariant.
- Original QuPRS generator pinned to
  `0b28736651b3740ed3b201edd2efdef3da541e0d`,
  original GPMC C++ pinned to
  `89bbc89d0de918e7a5385740355df55765039230`,
  *same unchanged emitted Weighted CNF SHA* for all solvers.
  No PathSum/QASM conversion included in measured solver WallTime.
- 15-arm seven interleaved fresh subprocess full WallTime p50/p90,
  observed peak RSS, bounded 30s and 2GiB address space. GPMC
  30/100 decimal digits ↔ Rust 100/333 binary MPFR bits.
  Source cases: HWB6, Barenco5, ModMult55, with original Grover5
  as explicitly censored hard holdout. Original 13 arms and
  prior within-arm exact SHA/node/topology control gates retained.
  The new two arms may legitimately change nodes/cache/rounding;
  SAT and both complex parts must match original GPMC.
- Six newly named original source-CNF CodSpeed **Simulation**
  targets, preserving the 45 previous MPFR research identities
  and the original formal-core benchmark set. CPU Simulation
  never substitutes for full-process native WallTime.
- If new post-closure cache does not reduce completed
  long-case search work enough to outweigh its two canonical
  scans, reject it for production. If Grover5 is censored, report
  the resource/node limit and do not infer a speedup.
- Do not change production solver defaults and do not merge
  this stacked Draft branch until a clear, repeatable qualified
  result is established.
