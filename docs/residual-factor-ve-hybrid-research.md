# Exact signed-complex Factor-VE on DPLL residual CNFs

Status: **independent opt-in research Draft**. No changes to the
production RaWMoCS solver, public API, QuPRS adapter, or default GPMC.

## Evidence before this experiment

[Exact root factor sum-product PR #110](https://github.com/PhysicsQoo/RaWMoCS/pull/110)
was an algorithmic win on original QNN4/VQE4 Weighted CNF, including
QNN4/100 GPMC 14.319ms, Rust Inline 27.256ms, root Factor-VE 7.672ms
on identical source input and precision.
[Independent Feynman PR #111](https://github.com/PhysicsQoo/RaWMoCS/pull/111)
qualified six original Feynman cohorts and independently confirmed
Barenco5 and ModMult55 root VE wins. HWB6 never admitted to global VE
(the root source-envelope cap is diagnosed separately in PR #113),
while GPMC finished Grover5 and both Rust modes remained censored.

A global "all DPLL" versus "all Factor-VE" selector is insufficient.
Even successfully finished root VE was slower on MQT W-state4, QAOA7
and trivial 1-DPLL-node circuits. Global root constraints may forbid a
factor DP that becomes small only after Shannon branching and unit
propagation. The independent hypothesis is therefore **bound-aware
factor elimination on small DPLL residuals**.

## Proof of exactness

At a particular reversible-trail DPLL node, the parent residual scope
S is partitioned by the unchanged solver's current Boolean unit
closure into:
- assigned/forced literals F, whose **original signed positive or
  negative complex unary weights** have already been multiplied into
  the node prefix P;
- previously free variables Z, whose signed weight sums
  w_v(true)+w_v(false) are already multiplied into P;
- active variables A occurring in still-live residual clauses.

All satisfied original clauses are removed; falsified literals are
removed from each unsatisfied clause. This is exactly Boolean
cofactoring under the existing trail assignment. The remaining
clauses are F_res(A), and no active variable occurs outside A.
For this exact state, total weighted count is

  WMC(node) = P × FactorVE(F_res(A), original unary weights on A)

provided the FactorVE call terminates within its independently bounded
table/product limits. This is ordinary distributive sum-product
elimination, **not** SAT-only projection and not a change of physical
phase/weight precision. Boolean SAT uses the separate OR/AND channel,
and SAT-but-zero is never interpreted as UNSAT.

Any independent disconnected components are first handled by the
unchanged original Trail component multiplication. Root/source weight
equivalence continues to use its own signed-complex-compensated SCC
reduction; the residual factor table consumes only the correct
surviving variable scope. The existing cache key is constructed before
unit propagation; a completed residual DP outcome is saved behind
that exact pre-unit key, with the full prefix included exactly once.

If bounded FactorVE rejects the residual, **all partial factor
tables are discarded**; its output is never inserted into the cache
or multiplied into the result. Original DPLL runs unchanged from
the same reversible trail checkpoint. The failed DP time is still
included in native full WallTime.

## Strict research-only admission

- Opt-in CLI --trail-residual-factor-ve, requiring the original
  --weighted-equivalence and --trail-count without the other
  research root transformations.
- Attempt only after ordinary unit closure and existing component
  detection, with at most 64 active variables and at most 512
  candidate original clause IDs, and maximum active clause width 12.
- At most **8 DP probes per complete native solve**. Each probe is
  additionally bounded by FactorVE's existing maximum 11-variable
  bag, 150K table cells, 600K factor products, 256 variables and
  3K clauses. No "turn off caps" or unbounded exponential table
  allocations.
- Diagnostics: attempts, completed residual DPs, capped attempts,
  sum of factor-product visits and largest observed bucket width.
  An opt-in arm is not a speed win merely because DPLL node count
  is smaller; the DP operation count and full WallTime are mandatory.
- Independent signed Gaussian/dyadic Complex WMC and SAT unit tests,
  including a SAT case with complex zero, true Boolean UNSAT, unit
  closure and free-variable product. Baseline and hybrid results
  must agree at 100 and 333 MPFR bits across cache capacities.

## Source-identical native four-arm experiment

Use original pinned QuPRS commit
0b28736651b3740ed3b201edd2efdef3da541e0d and unchanged
original C++ GPMC 89bbc89d0de918e7a5385740355df55765039230,
with exactly the same QuPRS-emitted Weighted CNF bytes per circuit.
The four independently timed arms are:
1. unchanged original GPMC C++;
2. unchanged Rust Inline Full weighted DPLL;
3. exact root FactorVE (the PR #110 algorithm);
4. DPLL plus bounded FactorVE of live post-unit residuals.

GPMC 30/100 decimal-digit numerical settings pair with Rust
100/333-bit MPFR according to the same pinned conversion.
All variants run on the same Linux native runner with 30s and
2GiB address space per child, 7 fresh order-rotated processes
per complete case×precision cohort. Full WallTime includes
parsing, search, factor planning, and engine destruction;
peak RSS is observed rather than a hard RSS cap.

MQT: seven original committed QASM sources (W-state4,
VQE4, QNN4, Grover4, QWalk3, QAOA7, QPEexact8)
×2 precisions = **14 predeclared cohorts**.
Feynman: HWB6, Barenco5, ModMult55 ×2 precisions =
**6 predeclared complete cohorts**, plus one costly
Feynman Grover5 probe per precision, all censored outcomes
preserved. Do not silently discard benchmark families.

Boolean SAT and both signed Complex WMC components are
checked separately; all source QASM and CNF SHA, native
status/timeout/RSS, factor width/cell/product counts,
hybrid attempts/successes/caps, DPLL nodes/cache hits/
components, and p50/p90 process WallTime are archived.
Independent small-model brute-force exact oracle tests
remain separate from large-case **GPMC numerical
cross-agreement**; neither is misrepresented as the other.

## Scientific Go/No-Go

A positive decision requires material reductions of **complete
native WallTime**, accompanied by an actual completed residual DP,
on hard original Weighted CNFs (especially HWB6/Grover5), with
correct signed WMC and SAT and no major regressions on
MQT and Feynman holdouts. A censored Grover5 remains a
negative result even if other cohorts finish. The combined
first-stage approach must improve over *both* original
Inline and previously successful root FactorVE on the
families where that original FactorVE works.

If residual DP remains rare, attempts all cap, or work
overhead makes native WallTime worse, keep the negative
result and prioritize a true source graph separator/
join-tree decomposition or bounded sum-product sparse factors,
not manual manipulation of physical rotation angles.
