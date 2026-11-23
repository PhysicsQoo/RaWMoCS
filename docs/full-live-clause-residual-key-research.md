# Exact fully live source-clause key materialization (opt-in research)

This experiment is strictly **off by default**. Original pinned QuPRS
Weighted CNF, unmodified GPMC C++, SAT and signed Complex WMC arithmetic,
MPFR precision, signed weight ownership, cache key equality, source clause
order, DPLL branches and reversible Trail are unchanged. No public issue IDs.

## Falsifiable optimization

In the fused source-clause compact residual constructor, the existing
reversible `live[cid]` counter is exactly the number of unassigned
literal occurrences in an original source-normalized clause. For an
unsatisfied clause, `live[cid] == clauses[cid].len()` is a certificate
that *every* literal is unassigned. The new
`--trail-full-live-clause-fastpath` uses the same exact first-literal scope
admission and directly emits original signed source literals (or the
same source-order signed weighted local ordinal mapping) with **no
redundant per-literal assigned[] check**. Clauses that are partially
assigned or have insufficient certificate use the original identical
literal traversal, sort, dedup and zero-terminated serialization.

This is **not** the old Arena option, ordered-key shortcut, active
clause bitmap, reversible literal mask, or scope stamp: none of those
changes is enabled by this flag. It introduces no persistent index,
allocation cache, CNF rewrite or new resource cap; existing Trail
apply/undo already maintains `live[cid]` exactly.

The instrumented `RESEARCH_MPFR_FULL_LIVE_KEY` prints whether the
option was enabled and the number of certificate-based eligible
source clauses. Count alone is not a performance metric. This
optimization can hurt if the branch condition exceeds the work it saves,
particularly on mostly assigned or tiny clauses.

## Native original-source acceptance

Feynman [original source-pinned workflow](https://github.com/PhysicsQoo/RaWMoCS/actions/workflows/cnf-factor-ve-minfill-paired.yml)
and independent MQT [original-QASM workflow](https://github.com/PhysicsQoo/RaWMoCS/actions/workflows/mqt-reusable-key-arena-holdout.yml)
append separately named triple-arm paired studies without replacing
any prior benchmark data. The three arms are **unchanged original GPMC**,
**original Rust Inline**, and **same Rust Inline + one new flag**.
Cases: Feynman HWB6, Barenco5, ModMult55; MQT QNN4, VQE4, QPEexact8.
Each source × precision cohort performs seven order-rotated fresh
processes, 30s and 2GiB virtual-address-space limits. GPMC 30/100
decimal digits match Rust MPFR 100/333 bits. All same-CNF
source/QASM hashes, complete-process p50/p90 WallTime, peak RSS,
exact signed Complex real and imaginary WMC, separate SAT,
byte-identical original Rust stdout and unchanged DPLL/cache/
decomposition topology must be archived. Negative and censored
cases must never be silently discarded.

Rust fixtures cover nontrivial partial residuals, reversible
undo, duplicated projected clauses, SAT-but-zero, UNSAT, 100/333-bit
MPFR and cache capacities 0/1/128. No positive speed claim before
the **exact final-head CI** qualifies both source families and
unprofiled native p50/p90 with observed RSS. Existing CodSpeed
Simulation is auxiliary hotspot evidence, never WallTime parity.

Only if HWB6 and independent MQT significant holdouts **both**
show repeatable improvement over original Inline with controlled
p90/RSS, and the unchanged GPMC hard-case deficit materially
shrinks, could a separate minimal main-based production PR be
considered. Otherwise retain a documented NO-GO and do not
promote this research flag or change the default Router.

## Final-head native outcome — 2026-11-19 (performance NO-GO)

The final research HEAD `4b7eea7e426393449e32b9003e51f3cae879ff24`
completed both independent original-source three-arm workflows:
[Feynman #44468652605](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44468652605)
and [MQT #44468652217](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44468652217).
The full-live three-arm steps succeeded in both jobs. All six cohorts per family
(three original QASM sources at two effective MPFR precisions) preserved SAT,
signed Complex real/imag WMC, original Rust stdout and search topology. These
checks establish numerical/source correctness, **not** a speedup.

Unprofiled full-process same-runner native medians, in milliseconds; 7 rotated
fresh trials per arm and original source+weighted CNF, GPMC and Rust precision
locks were retained. This table never combines observations from separate batches.

| Original source / precision | GPMC p50 | Inline p50 | Fastpath p50 | Inline p90 | Fastpath p90 | Paired wins |
|---|---:|---:|---:|---:|---:|---:|
| Feynman HWB6 / 100-bit | 168.952 | 504.395 | 504.661 | 561.973 | 524.791 | 3/7 |
| Feynman HWB6 / 333-bit | 185.588 | 535.409 | 535.323 | 558.143 | 571.495 | 2/7 |
| MQT QNN4 / 100-bit | 13.724 | 31.643 | 31.532 | 32.349 | 31.746 | 6/7 |
| MQT QNN4 / 333-bit | 14.370 | 33.198 | 33.213 | 33.844 | 33.349 | 6/7 |

The remaining Barenco5/ModMult55 and VQE4/QPEexact8 cohorts also matched
source/Complex WMC, but offer no consistent p50/p90 benefit. In particular,
HWB6/100-bit has 2,465,529 exact fully-live clause hits yet **regresses p50**;
HWB6/333-bit **regresses p90**; QNN4/333-bit **regresses p50**. Counter hits
are not a substitute for a native end-to-end speedup. Current tests preserve
RSS measurements in raw CI artifacts; no standalone observed RSS reduction is
claimed.

**Decision: correctness PASS, performance NO-GO for this opt-in full-live clause
fastpath.** Keep the source-exact negative evidence, do not enable the flag by
default, do not change the production solver/Router and do not treat the
successful GitHub Actions result as a performance acceptance. The experiment's
runner-level status `SOURCE_LOCKED_NUMERIC_EVIDENCE_NOT_PRODUCTION_GO` is
consistent with this stricter scientific conclusion; it is not an automatic
production promotion gate.

### Next independent, solver-owned hypothesis

The unmodified GPMC feature-ablation [MQT native #44468652565](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44468652565)
reports QNN4/100-bit GPMC default 13.917ms versus *no tree decomposition*
18.134ms (333-bit 15.584ms versus 18.902ms) on the same original CNF. This
motivates a separate **count-safe structure-aware decomposition/branching**
study, not a claim that the feature toggle proves causality in Rust. Its
required control is original GPMC, unchanged Rust Inline and one opt-in
Rust structural candidate on identical signed Weighted CNF, at 100/333 bits,
with independent SAT+Full Complex WMC, search-work counters, p50/p90 full
WallTime, peak RSS and unchanged Grover5 censored cases. Prefer changed
search/component workload over further literal-scanning micro-optimizations.
