# Research: component-local exact signed-complex Factor-VE at AND/OR splits

## Scope and existing solver

The RaWMoCS MPFR research Trail solver **already** performs reversible
unit propagation, independent disconnected-component AND decomposition,
original signed weight-class exact memoization, and Shannon OR branching.
It also has an independent whole-residual Factor-VE hybrid arm.

This new opt-in arm targets a distinct boundary: when the existing
solver has **already proved** that the current residual decomposes into
disconnected components, count a bounded small *complete component* by
existing dense exact Factor-VE instead of recursively searching it with
Trail. Other components are still solved by unchanged Trail, preserving
its exact cache. No new CNF encoder, new generic AND/OR solver, or
production/default solver change is claimed.

## Exact algebra and source weight ownership

For each exact residual under the existing reversible source trail:

W(F|a) = W(forced units|a)
         * product_{unconstrained x} (w_x(+) + w_x(-))
         * product_{independent connected components C} W(C).

SAT is an independent Boolean result; a satisfiable component may have
signed-complex WMC exactly zero by cancellation. Each still-unassigned
variable weight is owned by exactly one independent component. The
parent trail has already multiplied all forced-unit and free-variable
weights, which are *excluded* from component local Factor-VE. Complex
multiplication and addition use the requested original 100/333-bit
MPFR-equivalent precision and preserve signed real/imag values.

## Implementation and strict budgets

The explicit research CLI flag --trail-component-local-ve requires
weighted-equivalence + existing trail-count, and is incompatible with
other experimental factor/cutset/residual-VE root transforms.

Only after a valid existing DSU component split:
- Candidate whole component size: 8..32 unassigned source variables.
- At most 512 live source-clause IDs before a probe.
- Derive residual clauses by retaining only actual source clauses that
  touch this component, removing assigned false literals. Reject any
  attempted live cross-component clause instead of silently dropping it.
- Residual component has at most 128 clauses, with max width 12.
- At most 8 total completed or failed component Factor-VE attempts
  per entire source solve.
- Existing dense Factor-VE width-11, cells, products, and resource caps
  remain unchanged; any rejected probe discards its partial state and
  invokes exactly the original Trail recursion for that same component.
- SAT and signed-complex WMC are combined at the original AND parent
  and the original exact parent cache key and reversible trail survive.

All precheck work, failed attempts, fallback DPLL search, original
parsing, output and solver work are included in full process WallTime.

## Independent exact correctness

A connected 8-cycle and disconnected independent 2-variable edge make
a 10-variable original CNF whose components are not artificially
created by the research algorithm. At 100 and 333 bits, verify:
- at least one whole component is completed via Factor-VE,
- full SAT and both signed Complex real and imaginary values exactly
  equal an independent enumeration of 2^10 original assignments,
- original Trail agrees for exact caches with 0, 1 and 128 entries,
- no assigned trail elements are left after the computation,
- a separate connected original graph still uses original DPLL.

## Native fairness and go/no-go

Same original QuPRS-emitted Feynman Weighted CNF bytes, same original
signed literal weights, fixed pinned original QuPRS and GPMC revisions,
same CI runner, GPMC 30/100 decimal digits compared to Rust MPFR
100/333-bit-equivalent precision; 7 order-rotated **fresh child
processes** per ordinary original source x precision; 30s/2GiB per
child. Compare original GPMC C++, Rust Inline, original root Dense VE,
earlier whole-residual hybrid, and the new component-local VE.
Require SAT and both signed real and imaginary WMC consistency.
Collect full p50/p90 WallTime, measured RSS, DPLL-node savings,
component splits and exact cache hits, VE completed/capped attempts,
product counts and all censored cases including Grover5.

A useful result requires real source-CNF component VE completion,
significant DPLL-node savings and competitive full process WallTime
and RSS versus same-run original GPMC, **not** just a smaller
experimental inner loop. If no component completes, fallbacks must
retain original Inline DPLL node count, cache hits and decomposition
count. If short cases improve but HWB6 still lags GPMC, this remains
research-only and No-Go for hard-case routing. Broader routing requires
independent original MQT CNF held-out evidence.

No private tracker IDs in public code, PR, CI or documentation.
