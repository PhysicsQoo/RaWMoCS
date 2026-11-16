# Bounded CNF cofactor separator lookahead: Rust weighted model counting

**Status:** experimental opt-in. Not part of released RaWMoCS solver core.

## Scope and exact semantics

RaWMoCS receives an unchanged Weighted DIMACS CNF and signed complex
literal weights, already exported by QuPRS. It does not parse QASM,
perform Path-sum/RR, or receive native ANF factors. GPMC C++ and all
Rust variants receive **byte-identical weighted CNF inputs**.

The baseline Rust search branches on the highest-frequency variable.
The new optional flag --separator-lookahead estimates whether
assigning a candidate variable makes the *cofactor* CNF split into
independent variable-incidence components. It looks at both True
and False cofactors of up to eight source-CNF high-frequency candidates
and favors a smaller worst retained component when at least one
branch demonstrably disconnects. Otherwise the branch is exactly
the same as default Frequency.

This is an exact **Shannon WMC expansion**, not a SAT-only rewrite:

WMC(F) = w(v=true) * WMC(F given v=true)
       + w(v=false) * WMC(F given v=false).

No clauses, variables, weights, signs, cache-key semantics or SAT
verdict are modified by the lookahead. The solver still handles
Component multiplication, SAT-but-zero-complex-WMC and independently
rounded MPFR arithmetic through its existing paths. A change in
summation order can affect last-bit rounding for non-dyadic inputs;
source-matched 100/333-bit tolerance checks are required, not
identity claims for every inexact arithmetic order.

## Admission, budgets and rollback

- At most 320 scanned residual nodes, 4 million total inspected
  literals, eight candidate variables at each admitted node.
- Residuals with fewer than four or more than 640 active variables,
  more than 1,300 clauses or 7,000 literals are not examined.
- The score uses DSU-connected components of each candidate cofactor,
  without recursively solving or allocating an exponential table.
- If a cap is exceeded, all later nodes use unchanged Frequency,
  and the diagnostic reports CENSORED; partial lookahead results
  never change a cached result or weighted model count.
- It can be combined with the earlier proven model-bijective
  --weighted-equivalence optimization, which correctly folds
  positive/negative Complex weights when source-CNF SCCs prove
  equivalent or opposite variables. This research tests both
  equivalent-literal-only and combined versions independently.

## Test design

Independent small complex weighted-CNF tests at both GPMC-matched
100 and 333 MPFR binary bits, a source-owned separator witness,
a no-improvement control and a strict work-cap fallback. The exact
WMC remains the original CNF's model sum.

Source-pinned four-arm comparison (GPMC C++, original Rust Frequency,
Rust Frequency+weighted-equivalence, and the same Rust equivalence
plus bounded cofactor separator lookahead) on original QuPRS Feynman
HWB6, Barenco5, ModMult55, and hard Grover5. QuPRS source pin:
0b28736651b3740ed3b201edd2efdef3da541e0d.
GPMC C++ source pin: 89bbc89d0de918e7a5385740355df55765039230.
Same native Linux runner, 100/333-bit precision, seven balanced
rotating fresh native subprocesses per completed case/precision and
one bounded all-arm Grover5 pilot. GPMC and Rust process WallTime
include startup, weighted CNF parse, preprocessing, DPLL and output.
QuPRS's fixed source export is outside solver timing and not
interpreted as RaWMoCS performance.

The full raw reports include original Weighted CNF SHA-256,
both Full Complex WMC components, separate SAT verdict, p50/p90
full process WallTime, observed peak RSS, nodes, Cache hits,
Component splits, number of cofactor lookahead choices/scans
and all CENSORED time/node/OOM outcomes. An uncompleted solver
must **never** appear in speedup rankings. A 2 GiB RLIMIT_AS guard
limits virtual-address-space, not physical-RSS cgroup memory.

**Admission** requires positive *whole native process* speedups in
multiple unseen original weighted CNFs and no severe high-cost
regressions. Do not select the candidate globally on the basis of
isolated small-circuit results. Preserve all CodSpeed benchmark
identities and existing production Frequency/Cache defaults.

Reproduction:
- benchmarks/mpfr_research/src/separator_lookahead.rs
- benchmarks/rust_core/tools/quprs_cnf_separator_ab.py
- .github/workflows/cnf-separator-science.yml
- .github/workflows/cnf-separator-paired.yml
