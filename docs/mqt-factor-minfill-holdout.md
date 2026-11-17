# Independent original MQT five-arm min-fill exact factor elimination holdout

Research-only independent Draft. The previous five-arm original Feynman
[PR #116](https://github.com/PhysicsQoo/RaWMoCS/pull/116)
found **same-CNF exact min-fill selection improved already-admitted
Factor-VE cases**, but did not solve HWB6. In the first completed source
run, Barenco5/100 native p50 was min-union 6.063ms versus min-fill
5.006ms, with maximum bucket width **6→5** and exact factor-product
visits 3654→3542. ModMult55/100 was min-union 6.071ms versus
min-fill 4.975ms even though product visits 4217→4243;
a different order can affect actual arithmetic/table work.
HWB6/100 min-union scope320 did 36,248 products before rejecting
the first 12-variable bucket; min-fill structurally rejected
predicted width12 before bucket products, but still fell back
to identical DPLL nodes and did not beat unchanged Inline.

These observations are from the first Feynman native cohort;
this independent **seven-circuit MQT original-QASM holdout** is needed
to test out-of-family generalization.

## Frozen original CNF evidence

Freeze the seven previously committed unmodified original MQT QASM
sources W-state4, VQE4, QNN4, Grover4, QWalk3, QAOA7, QPE-exact8
from pinned QuPRS, via unchanged original QuPRS WmcManager into
GPMC-dialect Weighted CNF. These are **not MQT-published CNFs**.
Check byte-identical source QASM and dual-dialect / phase sidecars.
Pin QuPRS SHA 0b28736651b3740ed3b201edd2efdef3da541e0d and
original unmodified GPMC C++ SHA
89bbc89d0de918e7a5385740355df55765039230.
All five native solver arms receive the SAME original weighted
CNF with all signed complex literal weights unchanged:

1. original GPMC C++ mode=1, decimal precision 30 or 100;
2. original Rust MPFR Inline with source exact weighted equivalence;
3. original exact FactorVE scope256 min-union;
4. exact FactorVE scope320 min-union;
5. exact FactorVE scope320 bounded feasible-first min-fill ordering.

Rust MPFR bits are the pinned original conversion 100/333. Original
Rust cache 50k, node budget 1M for all Rust arms. All FactorVE caps
are unchanged (11 bucket variables, 150K total cells, 600K products);
any cap discards every intermediate DP factor and uses identical
original DPLL fallback. The min-fill path is an ordering-only
experiment, retaining both Boolean SAT and exact original signed
Complex MPFR WMC.

Each of **14** original circuit×precision cohorts requires seven
order-rotated **fresh native processes per arm**, 30s and 2GiB
address-space cap per process, no hiding of 10s+ first-trial or
resource-censored cases. Original QASM/CNF SHA, SAT and both signed
Complex WMC numerical fields, stdout digest, complete process
WallTime p50/p90, actual observed peak RSS, DPLL fallback topology,
factor product/cell counts and min-fill predicted width/edges/
planning cost and full cap evidence retained.

Cross-agreement against original GPMC at matched precision
is a numeric **solver cross-check**, not an independent exact
large-CNF WMC proof. Independent small signed Gaussian/dyadic
SAT+WMC oracles remain in the source Rust science suite.

## Go/No-Go

Keep the acceptance gate strictly at 14/14 complete source×precision
cohorts; censored cases are preserved as negative evidence rather
than cherry-picking. A useful static structural router must select
min-fill only when it is predicted to improve **full-process WallTime**
and factor work across independent source families, not just
Barenco5/ModMult55. In particular check whether QNN4/VQE4
can improve beyond root FactorVE, and that the simple one-DPLL-node
circuit classes do not regress without benefit.

If the MQT holdout is mixed or negative, keep both PRs as Draft
and prioritize dynamic structure-aware cost admission and
sparse/join-tree factors over blanket min-fill default.
Neither production RaWMoCS main nor QuPRS GPMC default changes.
