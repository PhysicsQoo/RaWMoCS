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
