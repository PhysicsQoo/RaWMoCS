# Bounded min-fill ordering for exact signed-complex Factor-VE

Status: independent opt-in Research Draft. This is a **pure ordering
algorithm**, not a CNF conversion or SAT-only rewrite. The exact signed
Complex WMC sum-product and separate Boolean SAT channels are unchanged.
No production RaWMoCS main/QuPRS default changes.

## Empirical motivation and the falsifiable hypothesis

[Previous original source-scope experiment #115](https://github.com/PhysicsQoo/RaWMoCS/pull/115)
passed 72 Rust tests and all six source-locked Feynman
source×precision native cohorts. HWB6 original source contains
297 variables, 285 live after exact weight-compensated equivalence,
803 original CNF clauses, and max source clause width 4.
Changing only the input admission cap from 256 to 320 allowed the
existing min-union elimination order to do 36,248 factor-product visits
before encountering a 12-variable candidate bucket, exceeding the
**unchanged** 11-variable limit. It rejected the DP and fell back to
unchanged 73,424-node Rust DPLL. Native p50 was 499.596ms Inline,
500.315ms VE256, **567.439ms VE320** versus original GPMC 175.969ms
at 100-bit effective precision. At 333 bits, 531.967 / 529.225 /
602.404ms versus GPMC 195.383ms. More source admission did not help.
A width-12 candidate in this one order is NOT a proof that HWB6's
optimal induced width is at least 12.

## Exact algorithm being tested

In an independent module, construct the **primal variable graph of the
same original source Weighted CNF clause scopes**; positive/negative
complex weights remain original unary factors. For each remaining
variable, count the number of absent edges among its active neighbors
and choose a feasible-width-first minimum-fill variable (ties by
neighbor count, original variable ID). After eliminating that variable,
add the fill clique, remove the variable and repeat. If even the
selected candidate has width greater than 11, return
`cap_reason=bucket_width` and **reject Factor-VE before building
exponential tables**.

This structural order is applied ONLY to the exact unchanged
sum-product elimination engine. It never alters clause truth tables,
literal weights, source CNF bytes, free-variable factors, SAT channel,
or bucket partial product semantics. The unchanged default min-union
order remains in the original count() / count_clauses() paths.
Independent diagnostics expose predicted maximum width, fill-edge
count, candidate evaluations, planning WallTime and exact cap reason.

**All bounds are kept identical:** <=320 live source variables for
this probe, MAX_BAG=11, MAX_CELLS=150K, MAX_PRODUCTS=600K,
MAX_CLAUSE_WIDTH=12, MAX_CLAUSES=3000, MAX_VARS=4096. All caps fail
closed into unchanged Rust Trail, and all time spent planning is
included in full native process WallTime. The opt-in flags are
`--factor-variable-elimination --factor-ve-scope-probe-320
--factor-ve-minfill-order`, requiring original exact weighted
equivalence and Trail flags. No performance claim if the width
remains >11 or factor work still exceeds a cap.

## Independent correctness and source-locked native protocol

Tests include a deterministic 4-cycle (one fill edge), a width-12
clique that must be rejected, a 257-unit sparse graph with no fill
and equal 100/333-bit exact SAT and signed Complex WMC versus Trail,
plus all existing small imaginary, cancellation, UNSAT and
disconnected-factor brute force fixtures.

Five source-identical native arms:
1. unchanged original pinned GPMC C++,
2. unchanged original Rust Inline,
3. original exact FactorVE with 256-source-variable cap,
4. baseline exact FactorVE with 320 cap and min-union order,
5. exact FactorVE with same 320 cap and bounded min-fill order.

Freeze original pinned QuPRS-emitted HWB6/Barenco5/ModMult55/Grover5
Weighted CNF files outside solver timing. QuPRS commit
0b28736651b3740ed3b201edd2efdef3da541e0d, GPMC commit
89bbc89d0de918e7a5385740355df55765039230. All arms consume
identical source-CNF bytes and original complex literal weights,
with GPMC 30/100 decimal digits paired to Rust MPFR 100/333-bit
under pinned original conversion. Seven rotated fresh child trials
per six regular source×precision cohorts, one Grover5
100-bit costly/censored negative control. Every child has 30-second
native wall and 2GiB address-space bound (observed peak RSS is
not itself a hard cap). Record original source and CNF SHA, both
signed Complex WMC components and independent Boolean SAT,
p50/p90 full WallTime, RSS, structural plan width/fill/planning
cost and actual factor cell/products, capped/fallback DPLL topology,
all censored and negative results.

**Go/No-Go:** All six regular cohorts must qualify. If the min-fill
order reaches a smaller maximum width but still performs poorly,
the correct next approach is structural score/work control or
sparse/join-tree/ADD factors, not unbounded table expansion.
If HWB6 still caps at 12+ variables, no generic min-fill promotion.
If HWB6 becomes solvable, independent held-out source-CNF tests and
full native WallTime/RSS improvement against both GPMC and Inline
are required; do not call VE DPLL-nodes=0 a performance metric.
All production defaults and private tracking exposure policies
remain unchanged.
