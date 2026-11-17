# Exact MPFR-weight-class component memoization: research-only

## Experimental question

The [original source-native Fused Trail comparison](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43977746573)
measured HWB6 100-bit full WallTime: original GPMC 176.381ms,
Rust Fused 502.736ms, 81,594 Rust DPLL nodes; all completed
100/333-bit sources agree on Boolean SAT and both Complex WMC
components. All Rust arms remain node-censored on hard Grover5.

The [independent short-clause-pivot study](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43984534264)
found that generic bounded Jeroslow–Wang pivot worsened HWB6
81,594 -> 107,077 nodes and 502.779 -> 689.856ms, and regressed
Barenco5/ModMult55. This new study branches **directly from Fused
Trail**, without that negative SAT heuristic.

**Hypothesis:** Component Cache currently stores full source
variable IDs, so two disconnected subproblems with exactly
identical signed weighted CNF structure but source variables
`{1,2,3}` and `{4,5,6}` cannot hit the same cache entry even
if their positive/negative Complex MPFR weights are equal. We
should reuse the weighted count under a provably safe
**order-preserving** variable relabeling, while preserving the
original control for exact A/B. This is not arbitrary graph
isomorphism or a general variable-symmetry theorem.

## Exact equivalence relation and proof obligation

Given active scope `s=(v1<...<vk)`, compute an injective ordered
relabeling `vi -> i` in the *cache key only*. Replace every signed
source literal `±vi` by `±i` in the canonical packed residual;
each clause separator remains exactly zero. Use the **ordered
weight-class vector** as the resulting key's `scope` field:
`(class(w(v1)),...,class(w(vk)))`.

Two source variables have the same class **only** if both positive
and negative complex MPFR weights match in their complete binary
mantissa/exponent/sign representations at the current precision.
Class labels are generated **once** per solver from conservatively
precise hexadecimal radix-16 strings of each of the four Float
components. This distinguishes signed zeros; no float tolerance,
approximate phase, numerical cancellation or hash-only lookup
may substitute for full key equality. Source scope retains **all
free variables**, including their weights; a cache key lacking
these would not preserve full WMC.

For any two scopes with equal localized PackedKey, the variable
relabeling is a bijection, every clause has the same literal
signs and membership, and each pair of corresponding variables
has identical positive and negative complex weights. Therefore
their weighted assignment sums (and Boolean SAT status) agree
mathematically, regardless of WMC=0 by phase cancellation.
Hashbrown still compares full PackedKey on collisions. **All
count recursion remains in original source variable IDs** and
only the cache identity changes. Difference in MPFR reduction
order, if any, is checked at equal precision in native A/B.

Variable labels are normalized monotonically in active scope
order (no exponential isomorphism matching or sign swapping).
The extra `Vec<i32>` rewrite and weight classification are
entirely opt-in. A stamped variable-rank index verifies all
cached literals belong to the current active scope before
relabeling; an out-of-scope literal fails closed rather than
silently creating an invalid cache alias.

CLI:
```
--weighted-equivalence --trail-count --trail-parent-ids
--trail-compact-residual --trail-prehashed-cache
--trail-fused-compact --trail-weight-class-cache
--hashbrown-borrowed 30 ORIGINAL.cnf
```

## Evaluation and conditional admission

1. An **independent old DPLL** and the existing Fused Trail
   must match exact signed complex dyadic WMC at 100/333 bits.
   A pair of disconnected, identically weighted 3-variable
   components must show **more cache hits and fewer DPLL nodes**;
   a change of even one positive or negative weight must *not*
   trigger that cross-component equivalence. With cache capacity
   0/1, count remains identical. Test 40 additional deterministic
   weighted CNFs, including unsat and SAT-but-zero configurations
   in the existing 41 tests. Trail state must fully restore.
2. Pin original QuPRS exporter
   `0b28736651b3740ed3b201edd2efdef3da541e0d`
   and untouched original GPMC C++
   `89bbc89d0de918e7a5385740355df55765039230`.
   Nine-arm **same original Weighted CNF** comparison:
   GPMC, original Rust control, weighted-equivalence, global
   Trail, Parent-ID, Compact, Prehash, Fused, Weight-class
   Component Cache. GPMC 30/100 decimal digits versus Rust
   100/333 binary MPFR bits, seven interleaved fresh programs
   per completed case, full program WallTime p50/p90 including
   parse/startup/finish; max observed RSS; exact CNF SHA256,
   SAT and **both** real/imag Complex WMC; 30s/2GiB AS
   guard. Censored Grover5 results preserved.
3. Preserve exact per-trial stdout SHA, nodes, cache hits and
   decomposition parity among unchanged parent/compact/prehash/
   fused controls. New cache is permitted different nodes
   and cache hits; only matched-precision SAT and both complex
   WMC components against original GPMC are accepted.
4. Add three separate new original-CNF CodSpeed Simulation
   identities; preserve previous 30 MPFR original-source and
   68 formal-core public identities. Inspect whether weighted
   key rewrite + class initialization outweigh avoided nodes.
   **Never equate CodSpeed Simulation with native WallTime.**
5. Admit as a production solver route **only if** original
   hard-source completed p50/p90 WallTime and RSS provide a
   repeatable clear win at matched precision. Improvements
   on a synthetic duplicated component alone are insufficient.
   Any no-benefit/negative result must be reported, not hidden.

## Status

Research-only Draft; no changes to GPMC, QuPRS exporter,
public `rawmocs-core`, default solver, or production API.
Benchmarks and science run under CI, not local workstation.
