# Short-clause weighted pivot on original QuPRS Weighted CNF

**Research-only / opt-in / not a default or production change.**
This is an independent DPLL *search-tree* hypothesis, unlike the
earlier per-node buffer/cache-layout optimizations. The complete
original `Fused Trail` search remains the direct A/B control.

## Why change the pivot now?

The original-source [Fused Trail CodSpeed run](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6afb924ca475fb5656e7e439)
profiles ~4.3s modeled HWB6 100-bit MPFR CPU; `count_relevant`
~25.6% self and `canonical_compact` ~20.8% self. Full native
[source-matched 8-arm CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43977746573)
measures HWB6 GPMC 176.381ms, Rust Fused 502.736ms, 81,594
Rust DPLL nodes; ModMult55 GPMC 11.784ms vs Fused 17.823ms
on unchanged source CNFs and 100-bit MPFR. At 333 bits,
HWB6 GPMC 194.182ms vs Rust 541.717ms. All six completed
cohorts satisfy independent original-GPMC SAT and both complex
WMC numeric checks. Source Grover5 still has original GPMC
completing and Rust bounded DPLL node-censored.

Fused compact, exact prehash and component caching reduce the cost
*per visited DPLL node*; the number of nodes remains 81,594 for
HWB6. This experiment changes only the Shannon split variable,
so it is intended to reduce work/tree size rather than to claim
more speed from allocation tuning.

## A bounded, deterministic, integer branching experiment

CLI `--weighted-equivalence --trail-count --trail-parent-ids
--trail-compact-residual --trail-prehashed-cache
--trail-fused-compact --trail-short-clause-pivot
--hashbrown-borrowed DIGITS ORIGINAL.cnf` enables the new
`use_short_clause_pivot` route. No other arm changes.

At each post-Boolean-unit-propagation node, for each currently
unsatisfied canonical source clause `C`, let `k(C)` be the
number of *unassigned* literals in its current residual. Accumulate
an integer score for each active variable:

```
score(v) = Σ [C contains v] * 2^(16 - min(k(C),16))
```

This is a bounded scale-equivalent Jeroslow–Wang-style
short-clause score. It uses **integer arithmetic only**, with
`saturating_add` on each accumulated variable score. Ties use
ordinary residual frequency and the original variable ID.
No variable sign, phase, complex weight, free-variable multiplicity,
cache-key semantics, Unit Propagation, Boolean SAT verdict or
numeric precision is changed. The order of exact weighted Shannon
recursion may change, so do **not** demand byte-identical floating
stdout or exactly identical nodes/cache hits relative to Fused.
Require SAT and **both complex WMC components** to agree within
the existing matched-precision numerical acceptance policy;
independent small dyadic cases must agree exactly. A measured
reduction in DPLL nodes is evidence of a search algorithm change.

The option requires all existing Fused prerequisites and is off
by default, including when only source Weighted CNF is loaded.

## Science and native performance gate

1. A positive branching witness where ordinary frequency and
   the integer short-clause score choose *different pivots*
   yet give the same signed Complex WMC, plus deterministic 40
   small CNF cases, UNSAT and SAT-but-complex-zero, and 100/333
   bit precision. Full trail state must restore after recursion.
2. Direct source-pinned original-GPMC native study:
   QuPRS generator `0b28736651b3740ed3b201edd2efdef3da541e0d`;
   original unchanged GPMC `89bbc89d0de918e7a5385740355df55765039230`.
   Same generated input CNF bytes for **all 9 arms**;
   GPMC `-prec=30/100` decimal digits versus Rust 100/333
   bits; seven rotated new native programs per completed case.
   Record full WallTime p50/p90 including process startup,
   parse, preprocessing, count, stdout and shutdown;
   observed peak RSS, 30s/2GiB virtual-address guard,
   DPLL nodes, cache hits, component splits and all censored arms.
   Source cases: original QuPRS Feynman HWB6, Barenco5,
   ModMult55, hard Grover5. No QuPRS PathSum/encoding time
   belongs in the solver comparison.
3. The eight unchanged non-JW research controls are
   independently checked, including exact bytewise
   Fused/Prehash/Compact semantics; the new heuristic only
   requires matched-precision numeric SAT/full Complex WMC
   agreement with the unchanged original GPMC.
4. The separate CodSpeed Simulation run adds exactly 3 new
   source-identical `rust_equivalence_trail_short_pivot_mpfr100`
   identities, preserving the prior 30 source-CNF and 68
   formal-core identities. Do not treat modeled CPU Simulation
   as actual native process WallTime.
5. The heuristic must demonstrate a meaningful long-source
   *completed* gain and preferably reduced search nodes without
   hurting the 333-bit or short-case negative controls.
   Grover5 node-budget censoring cannot be ranked as successful.
   No default routing or production adoption without a repeat
   of the native p50/p90/RSS and correctness checks.

**If this fails**: classify the regression, do not merge, and
investigate component-count-safe dynamic decomposition, cached
sub-CNF symmetry with weighted variables, or an incremental
SAT watched-literal architecture as distinct isolated experiments.
Never equate Boolean satisfiability with complex WMC nonzero.
