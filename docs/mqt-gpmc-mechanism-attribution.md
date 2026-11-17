# Root-cause attribution: unchanged GPMC on MQT original Weighted CNFs

Status: research-only independent Draft. Neither RaWMoCS production
solver nor QuPRS/GPMC sources/default settings are changed.

## Why this experiment is needed

Previous source-locked RaWMoCS root articulation and bounded root
failed-literal experiments passed exact SAT + signed Complex WMC
cross-checks, but did not generalize to seven independent MQT circuits.
In the preceding final-head native MQT failed-literal test, QNN4 at
100-bit equivalent precision took 14.918 ms in unchanged GPMC vs
31.316 ms in original Rust Inline and 31.393 ms in the new root
failed-literal version; all three were fed the **same original QuPRS
Weighted CNF**. Both Root-only and Failed-Literal mechanisms failed
to reduce QNN4 search nodes. Feynman HWB6 remains ~2.8 times slower
in Rust and Feynman Grover5 is still node-censored. Optimizing a new
pivot or increasing failed-literal budgets without understanding
GPMC's actual winning mechanism is likely low value.

## Source contract

- Original unmodified MQT QASM in pinned PhysicsQoo/QuPRS
  commit 0b28736651b3740ed3b201edd2efdef3da541e0d:
  W-state4, VQE4, QNN4, Grover4, QWalk3, QAOA7, QPE-exact8.
- Convert via the existing pinned QuPRS WmcManager exporter in CI
  once per circuit, outside solver WallTime; require original
  QASM byte identity, GPMC/Ganak CNF dialect semantic agreement,
  and identical phase sidecars. These are **QuPRS-emitted MQT QASM
  Weighted CNFs, not CNFs published by MQT Bench**.
- Build unchanged C++ PhysicsQoo/GPMC at pinned SHA
  89bbc89d0de918e7a5385740355df55765039230. The eight controlled
  feature arms use supported existing MiniSat Boolean options:
    - original defaults
    - -no-pp_ee (equivalent-literal elimination)
    - -no-pp_ve (variable elimination)
    - -no-pp_cs (clause strengthening)
    - -no-td (tree decomposition)
    - -no-bj (backjumping)
    - -no-pp_ee -no-pp_ve -no-pp_cs
    - prior combined flags plus -no-td.
- One **unchanged** existing Rust Inline Full MPFR arm runs in the
  *same native CI cohort*, contextualizing the gap without conflating
  a GPMC-toggle effect with a new Rust optimization.
- GPMC toggles always use **exactly the same decimal precision within
  each group** (30 and separately 100), so differences are due to
  changed algorithm/configuration and process-level variability, not
  numeric precision. Rust MPFR 100/333 bits is checked as a separate
  signed Complex WMC cross-oracle, using the pinned conversion.
  Boolean SAT and full complex real/imag values are independently
  checked. These large-instance cross-solver results are not an
  independent exact Full WMC certificate.
- 7 order-rotated fresh processes per each of the 14 original
  circuit × precision cohorts, or explicit censored status. Per-child
  timeout 30 s and virtual-address-space cap 2 GiB; RSS is observed
  GNU time maxRSS, not a strict resident set quota.
  Full WallTime p50/p90 and per-trial default-to-arm ratios are
  recorded, with source SHA, output hash and all failure evidence.
  If a first-trial arm exceeds 10 s, do not multiply expensive
  processes; preserve it as censored and fail overall qualification.
- Require all **14** cohorts. A censored toggle or full signed WMC
  mismatch is not interpreted as an algorithmic speedup.

## Mechanism interpretation

A GPMC option ablation is not proof of isolated source-code
causality: disabling a mechanism changes later preprocessing states,
tree shape, learned clauses or cache interactions. Multi-flag arms
test interactions, NOT marginal additive contributions. In
particular -no-pp_ee -no-pp_ve -no-pp_cs does not disable all GPMC
internal preprocessing, and -no-bj does not necessarily remove all
CDCL/learning machinery. No claim of fair comparison against a
different CNF representation is permitted.

For the difficult **QNN4**, first isolate whether any native GPMC
mechanism (especially preprocessing, tree decomposition or backjump)
has a meaningful within-cohort effect. Repeat the analysis across
QAOA7, VQE4, W-state4, Grover4 and QPE-exact8 so a mechanism that
only helps one case cannot be promoted without supporting evidence.

## Next Rust algorithm choice, gated on evidence

- If Tree Decomposition is pivotal on QNN4 and not overly
  costly on negatives, research native exact balanced multi-variable
  separator / join-tree DP on the original Weighted CNF, with both
  signed Complex WMC components and factor ownership certified.
- If weighted-safe equivalent-variable or bounded preprocessing is
  pivotal, identify the exact transformations in GPMC and implement
  only transformations with model-preserving weighted factors, under
  independent brute-force signed Complex WMC tests. A SAT-only
  equisatisfiable rewrite is forbidden.
- If backjumping is pivotal, derive a count-safe conflict-learning
  and weighted branch accounting proof BEFORE modifying search:
  standard SAT nonchronological backjump may silently discard a
  weighted contribution.
- If all GPMC toggles have little impact on QNN4, do not blindly
  duplicate them in Rust. Profile the remaining exact search-node
  difference and per-node overhead, cache topology and unit closure
  under *source-identical* original CNF and full native WallTime.

No production main merge or QuPRS GPMC-default change before
repeatable held-out Rust gains. Preserve all original positive and
negative trial data as a scientifically useful result.
