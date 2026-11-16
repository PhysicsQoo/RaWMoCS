# Reversible weighted-CNF trail: an independent solver architecture experiment

**Status: unmerged, opt-in, not qualified as faster.** Source project:
[Draft PR #92](https://github.com/PhysicsQoo/RaWMoCS/pull/92).

## Direct evidence and root-cause distinction

CodSpeed's [true original QuPRS MPFR executable profile](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6afaea32af135dad89c57dfb)
shows HWB6, 100-bit MPFR (simulation-modelled CPU cost):
`Engine::count` approximately 98.9% inclusive;
`unit_propagate_indexed_csr` approximately 24.3% inclusive;
`assign_fast` approximately 18.7% inclusive.
The latter clones/materializes entire residual clause lists for each
Shannon cofactor, repeatedly causing allocation, copying and cache
misses. Do not add inclusive costs, equate CodSpeed CPU Simulation to
WallTime or confuse `rawmocs-core` BigRational with this MPFR engine.

[Original full-WallTime source-paired CSR reuse study](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43860867235)
gained just 1.36% HWB6 (638.085→629.395ms, GPMC 190.833ms),
regressed ModMult55 and did not solve Grover5. Reusing temporary
buffers cannot alter a large DPLL search tree. Search topology and
the repeated residual materialization are distinct architectural
costs. GPMC additionally has a Glucose-based SAT engine, incremental
state and sophisticated component management; this PoC does **not**
magically reproduce its full algorithms.

## Opt-in, independent reversible architecture

`--weighted-equivalence --trail-count --hashbrown-borrowed 30 SOURCE.cnf`
selects `benchmarks/mpfr_research/src/trail_engine.rs`. It reads
the *same original* QuPRS Weighted CNF with both positive and negative
complex literal weights, preserving MPFR precision and rounding modes.

- Normalize and store the canonical source clauses **once**;
  construct signed-literal occurrence lists once before solving.
- Maintain one `assigned` array, reversible assignment trail,
  remaining-literal counts and satisfied-literal counts per clause.
- A Shannon assignment touches only literal occurrence-adjacent
  counters; undo restores every affected counter. No child
  `Vec<Vec<i32>>` CNF is allocated in either branch.
- At memoization boundaries, enumerate and sort the current reduced
  clause IDs lexicographically, deduplicate identical residual
  clauses, and build a flat exact cache key (source scope plus all
  reduced literal bytes). A Boolean contradiction is never inferred
  from a numerical zero or from a hash collision.
- Detect first-unit consequences in stable entry-residual order;
  multiply each forced signed complex weight exactly once; each
  branch multiplies its own signed complex literal weight.
- Dynamically compute independent components and free-variable
  `w(+v)+w(-v)` factors, and perform exact count combination.
  Component/partition evaluation remains *weighted*, not merely
  Boolean satisfiability.
- Node budgets and cache limits are enforced. There is no global
  static mutable state; the feature is off by default.

This is a distinct search algorithm, so DPLL node count, cache hits,
component partition order and non-dyadic MPFR result bytes may differ.
Unlike the CSR-buffer optimization, byte-identical stdout and identical
search topology are *not* prerequisites. Mathematical WMC and SAT
must agree; under finite MPFR, use an explicit matched-precision
numerical acceptance rule with independent dyadic exact fixtures.

## Required gates

1. Rust unit science: 100/333-bit signed/dyadic Complex WMC,
   contradiction, satisfiable zero, disconnected components,
   deterministic small independent randomized CNFs, full trail
   restoration after every call, no stale occurrence state or
   accidental double-weighting.
2. [Same original CNF native paired CI](../.github/workflows/cnf-trail-paired.yml):
   GPMC C++ `89bbc89d0de918e7a5385740355df55765039230`;
   QuPRS exporter `0b28736651b3740ed3b201edd2efdef3da541e0d`.
   Original Feynman HWB6, Barenco5, ModMult55, Grover5, no new
   QASM/PathSum/ANF encoding and no interpretation changes.
   GPMC 30/100 decimal digits ↔ Rust 100/333 MPFR bits. Seven
   interleaved independent processes per completed arm; hard 30s
   time/2GiB virtual address guard, observed RSS, p50/p90 full native
   WallTime, nodes, cache, components, both complex WMC parts and
   censored/incomplete cases.
3. [New CodSpeed exec identities](../codspeed.yml) on the exact original
   HWB6/Barenco5/ModMult55 weighted CNFs: three
   `rust_equivalence_trail_mpfr100` named runs compared against
   retained GPMC/Rust Frequency/Rust equivalence controls.
   Flamegraphs should demonstrate that `assign_fast` disappears
   from the new trail arm and reveal the remaining CPU hot path.
   CodSpeed CPU Simulation is strictly diagnostic and is not itself
   an end-to-end WallTime claim.
4. Confirm no unacceptable peak RSS growth or significant control
   regression. A 100-bit improvement that becomes slower at
   333 bits or fails original Grover5 is not enough for broad routing.
5. **Keep this Draft** until all gates pass. Any native or CodSpeed
   failure is a recorded negative or unknown, not silently removed.
   Do not modify original 68 formal-core CodSpeed names, public
   production core or QuPRS Adapter.

Future research if this isolation fails: incremental active-clause
indexes / watched literals and count-safe component cache/learning,
rather than repeated full scans to canonicalize every node.
