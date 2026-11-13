# QOO-219 — External SATLIB provenance and prepared-weight lifetime

Status: **research-only, source-locked external CNF evidence**, not a production solver optimization, not quantum-circuit WMC, and not an assertion of industry representativeness.

## Dataset lineage

Origin: [SATLIB — Satisfiability Library](https://www.cs.ubc.ca/~hoos/SATLIB/benchm.html), the well-known Uniform Random 3-SAT `uf20-91` and UNSAT Dubois generators.

Redistribution reference: archived public [mitchellh/go-sat](https://github.com/mitchellh/go-sat), SATLIB testdata mirror, repository commit `fc0e735aff48989326f256121b5ed6fc585858c3` (archive and MIT repository metadata at retrieval). The mirrored testdata [README](https://github.com/mitchellh/go-sat/blob/fc0e735aff48989326f256121b5ed6fc585858c3/testdata/satlib/README.md) explicitly attributes SATLIB; the original SATLIB data and any upstream terms remain separately attributable.

The three files in `benchmarks/rust_core/fixtures/satlib/` are copied **byte-for-byte** from that fixed mirror commit. Git blob IDs verified by CI (not arbitrary hash tracking on every solver run):

| File | Original path inside mirror | Git blob | Variables | Clauses | Evidence |
|---|---|---|---:|---:|---|
| `uf20-01.cnf` | `testdata/satlib/sat-uniform-20-91/uf20-01.cnf` | `bb5bdb347aa61f6c9f3079cf85f89d66595693e5` | 20 | 91 | Independently exhaustive Boolean SAT count **8** |
| `uf20-02.cnf` | `testdata/satlib/sat-uniform-20-91/uf20-02.cnf` | `f924caaef0d868bf62b1658e83e030ad8daee865` | 20 | 91 | Independently exhaustive Boolean SAT count **29** |
| `dubois20.cnf` | `testdata/satlib/unsat-dimacs-dubois/dubois20.cnf` | `fbad81bb8793f08cd9b12b0028d4d741263edb5b` | 60 | 160 | Upstream labeled UNSAT; **no independently checked proof yet** |

The `uf20-02` SATLIB original includes an end-of-data `%` marker and trailing `0`. Its raw bytes are preserved. The DIMACS parser stops at `%`, validates `p cnf` counts, literal ranges, terminators, and all 91 clauses; it does not silently delete or rewrite original clauses.

## Input and oracle semantics

SATLIB supplies **Boolean CNF only**. Every real/imaginary positive/negative literal weight is assigned through an explicitly declared **synthetic**, heterogeneous exact rational function `weight_for_variable(i)` in `satlib_provenance.rs`. These weights are not original SATLIB measurements and must **not** be cited as SATLIB WMC fixtures.

For the two SAT uf20 files:
1. An independent 2^20 Boolean truth-table test verifies that the predeclared satisfying-assignment masks are **complete** (8 or 29 exact masks). This test never invokes the solver.
2. The independent exact Full Complex WMC oracle enumerates **only those checked satisfying masks** and uses the ordinary four-product schoolbook formula over `BigRational`. It does not invoke DPLL, cache, or `Gaussian::mul`.
3. The default Rust solver is checked against that exact witness for cache caps 0 and 10,000.
4. The opt-in native experimental worker checks **every** baseline, gated and prepared solve for exact Boolean SAT, full real/imag rational count, canonical output fields for gated/prepared, and nodes/cache-hit/decomposition parity. Numeric zero must not be used to infer UNSAT.

For `dubois20`, only the exact original file parsing and upstream UNSAT **label** are currently verified. It is **excluded** from native Full WMC perf metrics pending independent UNSAT proof and bounded resource qualification; do not claim independent exact oracle coverage of this file.

## Experimental matrix

Dedicated `.github/workflows/satlib-prepared-research.yml`:
- Linux x86-64 source-locks the three original files by Git blob SHA before running any tests.
- `canonical-mul-research` remains **disabled by default**. The opt-in `PreparedCanonicalProblem` still owns privately canonicalized weights and creates fresh DPLL Engine/cache for every solve.
- Native research script `examples/satlib_prepared_paired.rs` executes the two independently verified SATLIB sources, **both** cache caps (0 / 10,000), **N = 1, 4, 16, 64** repeated solves and **three rotated orders**, with baseline, per-call-gated and prepared-once in a **different process per arm**.
- The exact source-locked evidence should contain **144 PAIRED rows** and **16 SUMMARY rows**. Setup cost is always inside the prepared arm's elapsed time and also separately reported.
- `VmRSS` is reported before and while the prepared object remains alive, alongside Linux process `VmHWM` and `PreparedCanonicalProblem::estimated_retained_payload_bytes()`. The estimate counts some Vec capacities, structs and exact rational bit payload, but **not** BigInt capacity slack, allocator metadata, fragmentation, cache or RSS; it is **not an allocation profile or strict memory bound**.
- No hard memory cap is implied by the VmHWM evidence. Workflow wall-clock timeout remains the only CI process-level bound in this study. QOO-208 owns platform resource enforcement.

## Decision gate

Even positive SATLIB results cannot prove default performance improvements on unknown industrial/quantum WMC instances, particularly since the complex rational weights are synthetic. Retain mandatory UNSAT and SAT-but-zero negative controls from the preceding studies. Do not switch default `solve()` or `Gaussian::mul`, modify cache policy, or integrate changes into QuPRS/GPMC without new, representative, source-locked holdouts and explicit API review.

**Next evidence gap:** independently checked UNSAT proof for the original `dubois20`, realistic large CNF with bounded native RSS/WallTime and adequate **Full Complex WMC** oracle, and stability of preparation break-even under changed input weights (never reuse a prepared object after changing weights without a new preparation step).
