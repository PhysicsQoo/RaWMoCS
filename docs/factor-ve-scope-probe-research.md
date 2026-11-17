# Factor-VE source-scope admission: controlled 256-to-320 variable experiment

Status: independent opt-in Draft. No production RaWMoCS core, QuPRS,
GPMC or default solver configuration changes.

## Why this is the next discriminating experiment

The source-locked [Factor-VE PR #110](https://github.com/PhysicsQoo/RaWMoCS/pull/110)
and [independent Feynman holdout PR #111](https://github.com/PhysicsQoo/RaWMoCS/pull/111)
demonstrated true same-original-CNF full-native WallTime improvements on
MQT QNN4/VQE4 and Feynman Barenco5/ModMult55. This success is qualified:
W-state4 and one-DPLL-node MQT cases get slower when factor DP is
forced on universally, and original HWB6 and Grover5 still lose.

The [bounded residual hybrid PR #114](https://github.com/PhysicsQoo/RaWMoCS/pull/114)
ran **71 Rust science tests PASS**, all 14 original MQT cohorts and all
six regular Feynman cohorts with source-locked GPMC/Inline/root FactorVE/
residual hybrid. Native results show hybrid on QNN4/100 runs 33.618ms
while the same-source root FactorVE finishes in 7.685ms. For HWB6,
the original source envelope is:
- 297 original variables
- 285 live variables after exact weight-compensated equivalence
- 803 original Weighted CNF clauses
- source max clause width 4

Root FactorVE fails **before processing any buckets** with
cap_reason=source_scope, width=0, products=0, because the existing
MAX_SCOPE=256 admission limit is deliberately conservative.
The result does not prove HWB6 has high induced treewidth. Residual
hybrid completes 8 small VE subproblems, but does not outperform
original Inline overall; Grover5 remains source-censored.

## Isolated question and implementation

Would allowing a **285-variable live input** into the same bounded
exact factor elimination expose a viable narrow-width join-tree, or
does it demonstrate a genuine width/product/cell cap barrier?

The default count() and count_clauses() functions RETAIN the original
MAX_SCOPE=256. Only a new opt-in count_scope_probe() permits
MAX_SCOPE=320. The new guarded CLI flag
--factor-variable-elimination --factor-ve-scope-probe-320
requires original --weighted-equivalence and --trail-count and still
rejects all incompatible root transforms. This cannot change the
production core, default, original DPLL fallback, CNF representation,
scope or complex literal weights. An explicit diagnostic reports
enabled and limit 256/320 to prevent accidentally treating it as the
ordinary FactorVE arm.

**All real resource bounds are IDENTICAL:** MAX_VARS=4096,
MAX_CLAUSES=3000, MAX_CLAUSE_WIDTH=12, MAX_BAG=11,
MAX_CELLS=150000, MAX_PRODUCTS=600000, existing original node
budget and 30s/2GiB native process limits. If ANY work or
bucket bound trips, exact FactorVE rejects ALL partial factor
results and calls the original Trail solver; prep work is included
in complete-process WallTime. A zero Complex WMC is never Boolean
UNSAT, since exact factor semantics keep SAT separate. Every
forced/free signed literal weight is owned exactly once.

An independent synthetic 257-unit variable fixture crosses the old
source envelope and checks new admitted VE full SAT+Complex WMC
against original Trail at 100/333 bits; this is ONLY an API/correctness
control, not a performance success fixture.

## Native source-locked scientific contract

- Pin original QuPRS at
  0b28736651b3740ed3b201edd2efdef3da541e0d and original
  unchanged GPMC C++ at
  89bbc89d0de918e7a5385740355df55765039230.
- Freeze identical original QuPRS Weighted CNF bytes from original
  HWB6/Barenco5/ModMult55/Grover5 QASM; record QASM and CNF SHA.
- Four native arms, identical source CNF and MPFR conversion:
  unchanged GPMC, unchanged Rust Inline, original factorVE 256,
  bounded factorVE 320. Seven order-rotated fresh processes
  per six completed HWB6/Barenco5/ModMult55×100/333-bit cohorts,
  one costly Grover5 control (333 only if original source runner
  proceeds after 100-bit).
- Native per-child wall cap 30s, process address space 2GiB,
  original 50K Rust cache and hard-case 1M node budget unchanged.
- SAT checked separately from BOTH signed Complex WMC components at
  matched decimal precision. Store complete-process p50/p90 WallTime,
  actual RSS, exact cap_reason, width, factor cells, product visits,
  fallback DPLL nodes/cache/decompositions, source hashes, stdout
  hashes, and all censored/negative outcomes.
- Six completed regular source×precision cohorts required. On
  failure, do NOT weaken qualification; incomplete and censored
  results remain archived. Grover5 need not complete to keep a
  negative control; it cannot be claimed as solved without evidence.

## Go/No-Go

This is a **cap-diagnosis experiment**, not a performance promotion
or a rationale for generally relaxing exponential limits. If HWB6
remains capped, report the precise new cap reason (especially
bucket_width, bucket_cells or bucket_products) and actual work before
fallback, then move to source-CNF separator/join-tree structure and
sparse factor representation rather than blindly enlarging tables.
If HWB6 completes, require full native WallTime and peak-memory
wins against original GPMC and Inline, and a separate independent
validation before considering any production router. Negative-case
barrier and source-specific speed changes remain important.
