# Exact-weighted CNF binary-clause assignment optimization

The ordinary `rawmocs-core` exact-rational Full WMC engine is the target
of this narrowly scoped source-compatible optimization. Only
`assign(clauses, literal)` changes: normalized binary clauses are
handled directly, and unaffected clauses are cloned without an
unnecessary per-literal filtering pass. Exact SAT, Gaussian rational
weights, tree search, component splitting, cache-key definition and
solver budget contracts remain unchanged.

**Scientific motivation:** In the pinned original QuPRS Feynman
`qft_4` emitted CNF, 225 of 296 clauses are binary. The isolated
100-bit/333-bit MPFR study
[CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43586354701)
found the same-source fast-assignment path took 61.600 vs 65.922 ms
(100-bit) and 63.079 vs 66.887 ms (333-bit), with identical SAT,
complete MPFR stdout and DPLL node/cache/component structure.
These gains motivate, but **do not prove**, a speedup in the exact
BigRational production core.

## Required exact-core gates

1. All established Rust science and CodSpeed benchmark identities
   remain correct and source-compatible; any meaningful regression
   must be investigated before acceptance.
2. Literal-level clause substitution is independently tested after
   normalization, including empty/unit/2-/3-literal cases,
   tautologies, repeated literals and recursive residuals.
3. Same pinned original QuPRS PathSum/WmcManager source emits the
   identical CNF bytes. On the same Linux runner, five alternating
   fresh-process trials compare original pinned core, new exact
   core and existing opt-in canonical rational arithmetic.
4. Every arm's exact printed-rational complex WMC, SAT, and entire
   stdout must match byte-for-byte; per-case walltime/RSS and
   original source hashes are recorded in the native workflow.
5. The **highest-cost Feynman qft_4 and mod5_4** dominate the
   acceptance decision. Retain slower cases, timeout/UNKNOWN
   and independent exhaustive oracle controls; do not extrapolate
   from short or synthetic fixtures.

No new production numeric precision configuration or QuPRS
SolverAdapter default is introduced. The separate matched-precision
Rust MPFR research engine remains a reference and is not silently
substituted for exact BigRational in this PR.
