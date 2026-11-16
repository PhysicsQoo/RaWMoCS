# Fused single-pass source-clause reduction for weighted MPFR DPLL

**Status: opt-in research experiment, not admitted or claimed faster.**

## CodSpeed evidence

[Source-locked CodSpeed Prehash MPFR run](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6afb207de04029b693fb1693)
measured real original QuPRS Weighted CNFs with the **separate Rust MPFR
research executable**, 100-bit precision, not the BigRational released core.
HWB6 Prehash modeled CPU time is 4.2s. Self-time hotspots:

- `TrailEngine::count_relevant`: 24.51%.
- `TrailEngine::canonical_compact::{closure#0}`: 19.65%.
- `PackedKey::hash`: 4.59%; `AHasher::write`: 3.73%.
- Cache effects account for 54.87% of **modeled CPU cost breakdown**.

These self-time entries are not additive with inclusive call trees.
Simulation is not native full-process WallTime. Prehash itself has
already improved the original source-locked seven-process native
[100/333-bit experiment](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43893682255)
by 2%-4% across all six completed cohorts, but HWB6 100-bit
Rust Prehash 508.296ms still trails original GPMC 177.516ms by 2.86x,
and original Grover5 remains Rust node-censored.

## Single change being studied

With `--weighted-equivalence --trail-count --trail-parent-ids
--trail-compact-residual --trail-prehashed-cache
--trail-fused-compact --hashbrown-borrowed 30 source.cnf`,
`TrailEngine::canonical_compact` scans the *same original clauses*,
but fuses **the first live literal's component-scope admission** with
**copying all unassigned literals into one contiguous view**.
Previously the code performed an initial `iter().find` then a
second full `for` loop for every relevant source clause on each
search node. The new arm reads a live clause at most once and
short-circuits immediately when its first remaining literal is
outside the current component scope.

The old exact-key Prehash Compact path is retained as the direct
control. The signed-literal occurrence trail, original source CNF,
component membership, canonical lexicographic reduced clauses,
full `PackedKey` equality and prehashed hashbuilder,
first-unit priority, WMC positive/negative MPFR weights,
SAT-versus-complex-zero semantics and every DPLL branch are unchanged.
No partial hash, probabilistic equality, assumption of SAT/CDCL
conflict learning or floating precision relaxation is introduced.
No default, production, QuPRS or GPMC code is touched.

This local optimization may be too small, or a new branch per
literal may offset the savings. Benchmark results, not intuition,
determine whether it is worth retaining.

## Acceptance

- Full independent 100- and 333-bit dyadic/signed Complex WMC,
  SAT-but-numerical-zero, contradictions, unit chains, disconnected
  components and 40 deterministic random-CNF cases. Confirm state
  restoration on every traversal and compare all supported Cache
  limits.
- Exact per-trial parity of complete SAT, WMC real + imaginary,
  stdout SHA256, DPLL nodes, Cache hits, component splits with
  **unchanged** Prehash Compact for every finished source case.
  If any differ, **fail closed**.
- Eight original same-CNF solver arms: unmodified GPMC C++,
  Rust frequency, signed weighted equivalent-literal DPLL,
  global Trail, Parent-ID Trail, Compact Trail, Prehash Compact,
  and the new Fused Prehash Compact. Generate original QuPRS
  Weighted CNF outside timing with pinned QuPRS SHA
  `0b28736651b3740ed3b201edd2efdef3da541e0d`
  and unchanged GPMC SHA
  `89bbc89d0de918e7a5385740355df55765039230`.
  Seven interleaved fresh independent native processes per
  completed source/precision, Rust MPFR 100/333 bits matched
  to GPMC 30/100 decimal digits.
- Require native p50/p90 **complete-process WallTime**, observed
  maximum RSS, exact original source sha256, complete real +
  imaginary weighted counts and censored/unknown results.
  Grover5 runs with its existing bounded node budget and
  incomplete results are not ranked against completed GPMC.
- Add three new original-CNF CodSpeed Simulation A/B names,
  retaining all 27 prior source-CNF identities and the
  original 68 `rawmocs-core` identities.
  Compare each new fused target to the same-run Prehash target,
  inspect `canonical_compact` flamegraph, and **validate native
  WallTime** separately. No simulation-only speedup qualifies
  for any promotion.
- An insufficient, negative or unstable native gain is an
  explicit negative result; leave Draft and do not merge.
  Significant remaining GPMC differences demand a separate
  **count-safe incremental component/branch/learning algorithm**
  rather than more memory micro-tuning.

Research sources:
`benchmarks/mpfr_research/src/trail_engine.rs`,
`benchmarks/mpfr_research/tools/quprs_cnf_fused_ab.py`,
`codspeed.yml`,
`.github/workflows/cnf-fused-science.yml`,
`.github/workflows/cnf-fused-paired.yml`.
