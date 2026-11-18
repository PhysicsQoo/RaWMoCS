# Bounded signed-complex ADD factor elimination from original weighted CNF

Research-only algorithm design / falsifiable implementation contract. **No solver implementation, production API change, or measured ADD performance result is included in this document.** A future implementation must remain opt-in, fail closed, and preserve the original GPMC and Rust Inline controls.

## Why a different factor representation is necessary

The independent original-Feynman primal-graph minor study ([PR #118](https://github.com/PhysicsQoo/RaWMoCS/pull/118), native [run 44090135268](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44090135268), HEAD `ef1d644a8a4ee39088823de31e39516297b564d8`) completed 80 Rust tests and six matched Feynman source×precision cohorts. For HWB6, the same 285 surviving variables and 803 source clauses have a **certified primal graph treewidth lower bound of at least 11**, hence a bucket-size lower bound of 12. The existing dense exact Factor-VE admits no bucket larger than 11. Therefore **no elimination ordering on that same primal graph can satisfy the current dense cap**. This does not show that HWB6 has no useful sparse or shared-factor structure, or that conditioned subproblems must share that width.

The same-run 100-bit original Feynman HWB6 p50 full process times were GPMC 191.130 ms, Inline 516.014 ms, min-fill/width-11 fallback 540.799 ms; at 333 bits they were 211.805/556.862/580.431 ms. Original GPMC Grover5 completed, all Rust research arms remained censored. This is negative hard-case evidence, not an exact ADD speedup.

The independent original MQT [14/14 cohort min-fill holdout](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44043535595) demonstrates low-width Factor-VE gains beyond Feynman while retaining short-case regressions. QNN4/100 GPMC/Inline/min-fill were 21.535/41.563/8.638 ms; VQE4/100 were 17.552/15.716/7.789 ms; QPE-exact8/100 were 6.458/5.657/7.827 ms. A generic factor-only default is not justified.

## Hypothesis and algebraic invariant

**Hypothesis to test, not yet established:** An ordered, hash-consed Algebraic Decision Diagram (ADD) can retain exact Boolean feasibility and finite-precision signed-complex values for clause and unary-weight factors while using far fewer nodes and arithmetic operations than the dense factor tables on some original source CNFs, potentially including HWB6.

Read the **same original QuPRS-emitted Weighted CNF bytes**. Every original CNF clause contributes a Boolean indicator `1[clause satisfied]`. Each surviving variable `x` contributes exactly one unary factor `w_x(false)` or `w_x(true)`; its original positive and negative complex weights are never dropped, replaced with absolute values, renormalized or moved into QuPRS. Never use QASM, Path-sum, ANF, or independently re-encoded CNF to build the diagram.

For an assignment-dependent factor the semantic domain is a pair

`F(a) = (SAT(a): bool, WMC(a): complex)`.

The mathematical operations are

`(s1,z1) ⊗ (s2,z2) = (s1 AND s2, z1*z2)`

`(s1,z1) ⊕ (s2,z2) = (s1 OR s2, z1+z2)`.

The clause indicator is `(true,1)` if satisfied and `(false,0)` otherwise. The unary weight indicator is `(true,w_x(a))`. For a variable `x`, `abstract_x(F) = F[x=0] ⊕ F[x=1]` (weights already owned by their unary factors); apply(product) combines all incident factors. This is ordinary finite-domain sum-product elimination *mathematically* and must not identify `SAT=true, WMC=0+0i` with UNSAT. Every genuinely free variable must contribute `w_x(false)+w_x(true)` once. All final disconnected components compose using the same pair operations.

Important: finite MPFR arithmetic is **not exactly associative or distributive**. The source mathematical identity is exact over complex numbers; the concrete pinned 100/333-bit implementation must preserve original MPFR precision, specify arithmetic order, and validate real/imag numeric cross-check tolerances separately. Do not assert bitwise equality across altered operation orders or call a different arithmetic backend "same precision".

## Candidate algorithm: canonical ordered weighted-factor ADD

1. Fix and record a bounded variable order computed **only from the original CNF scopes**, with the existing min-fill order as the first baseline. Do not optimize using source circuit labels. Use a terminal for each distinct pair `(SAT, real, imag)`, including `SAT=true, WMC=0`; a nonterminal is `(variable, low_node_id, high_node_id)`. Reduce `low==high` only when their complete outcome pairs are equal. Canonicalize terminal values using lossless MPFR identity or explicitly checked exact-value equality; never hash a rounded decimal string or coerce a signed-complex weight to Boolean.
2. Compile each original clause indicator directly from its literal scope and each original variable's signed positive/negative unary weights to ADDs. Keep the original immutable input and source digest available for fallback.
3. Perform `ApplyMultiply(A,B)` by cofactoring on the next variable and recursively combining terminals by ⊗, with an operation cache indexed by operation and canonical child IDs. At each chosen elimination bucket combine all incident factors and compute `AbstractSum(x,F)` via Shannon cofactors and `ApplyAdd`, using ⊕, not Boolean OR of WMC. A static order is the initial comparison; measure alternative order only after a valid baseline.
4. Track and enforce independent limits for live and cumulative ADD nodes, hash-consed unique nodes, terminal count, intermediate apply entries, recursion depth, arithmetic terminal operations, resident MPFR allocation estimate, elapsed planning/construction time, total memory, and existing solver worker budget. Bound all allocations before committing them.
5. Any rejected/capped attempt must destroy *all* partial ADD state and solve the **original** CNF with the unchanged research Inline/Trail fallback. No incomplete ADD outcome enters the original weighted cache. All attempted preprocessing and compilation time remains in measured native full-process WallTime.
6. Record literal factor count, mean/max clause width, diagram node count, terminal count, apply calls/hits, reduction/sharing ratio, peak intermediate node count, MPFR operation counts, cap reason, fallback DPLL nodes, peak RSS and full WallTime. The numerical `0 DPLL nodes` of successful ADD factor elimination is not a performance comparison with DPLL work.

## Discriminating studies

**First: correctness and compression feasibility.** Before large CNFs, compare independently against exhaustive exact dyadic Gaussian WMC and SAT on short signed/imaginary motifs; include true UNSAT, SAT-but-zero, cancellation, all-zero weights, free variables, disconnected factors, repeated/tautological clauses, polarity changes, duplicate literal appearances and exact cap rejection. Verify both 100/333-bit numerical results plus a proof-oriented exact reference for small cases. Run adversarial high-width low-sharing and sparse/high-sharing synthetic controls with identical source semantics. Observe exact diagram size rather than assuming primal graph width predicts it.

**Second: actual hard-case pilot.** Use the original unchanged HWB6/100 and /333 file hashes, signed weights, compiler flags, resource policy and independent native child. Compare original GPMC C++, unchanged Inline, existing dense Factor-VE min-fill, and new bounded ADD Factor-VE in the **same interleaved run**. Include Grover5 negative/censor and Barenco5/ModMult55 easy positive controls. A method that creates many shared diagram nodes but remains slower in full WallTime is a negative result.

**Third: independent out-of-family holdout.** Seven fixed original MQT QASM→QuPRS Weighted CNF sources (W-state4, VQE4, QNN4, Grover4, QWalk3, QAOA7, QPE-exact8) × 100/333-bit original precision correspondence. Require 14/14 matched cohorts, seven order-rotated fresh native trials per arm, same runner, 30s/2GiB per-child policy, p50/p90 native complete-process time, peak RSS, all signed complex real/imag fields and separate Boolean SAT, and raw logs/source QASM/dialect/phase/CNF SHA. The independent small exact oracle and original GPMC are different roles: large-CNF GPMC numerical agreement is not a formal proof of exact WMC.

**Go** only if high-cost HWB6 improves full-process WallTime/RSS and correctness without censor, while the independent holdout confirms non-regressive admission (not necessarily globally running the ADD mode). **No-Go** if the diagram grows beyond caps, fallback dominates, numeric accuracy fails, or benefits arise only on trivial/synthetic inputs. If No-Go, a separate solver-internal CNF-conditioned AND/OR decomposition with context-safe caching is the next hypothesis; the current root residual Factor-VE hybrid already had negative results and must remain as a control.

## Literature foundations and limitations

- [Dudek, Phan and Vardi, ADDMC, AAAI 2020](https://mlanthology.org/aaai/2020/dudek2020aaai-addmc/): literal-weighted model counting from CNF using ADD-based dynamic programming. Transfer the representation idea, **not** unverified runtime claims for signed-complex MPFR.
- [Kimmig, Van den Broeck and De Raedt, Algebraic Model Counting](https://arxiv.org/abs/1211.4475): semiring-oriented correctness view; an explicit separate satisfiability channel is required for cancellation semantics.
- [Certified Knowledge Compilation, SAT 2023](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.SAT.2023.6): independent checkable compilation/weighted model-counting proof direction, longer-term rather than a claim of current proof certificates.

### Project boundary

All changes in this experiment are research-only, opt-in, source-CNF-derived, and isolated from production RaWMoCS main and QuPRS's GPMC default. Do not mutate the original CNF bytes, weights or source material. The GitHub branch, PR, workflow, commits, and documentation contain no private task identifiers.
