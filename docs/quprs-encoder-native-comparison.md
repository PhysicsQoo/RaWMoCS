# Source-emitted QuPRS weighted-CNF solver comparison

**Scope:** Research-only source-provenance and native-process comparison of
[RaWMoCS](https://github.com/PhysicsQoo/RaWMoCS) versus
[GPMC](https://github.com/PhysicsQoo/GPMC). This does not change QuPRS
SolverAdapter/default, GPMC counting, or the RaWMoCS production API.

## Source evidence

The actual original Rust
[`PathSum → VerificationStrategy::Naive → WmcManager::encode_trace → to_dimacs_string`](https://github.com/PhysicsQoo/QuPRS/blob/0b28736651b3740ed3b201edd2efdef3da541e0d/rust/quprs/examples/interval_cnf_reference.rs)
is run at pinned QuPRS commit `0b28736651b3740ed3b201edd2efdef3da541e0d`
and original exporter blob `f9b0bb4afba35a9a67a1b9d2844be3b77884b27a`.
GPMC/Ganak DIMACS, QASM, phase numerator/denominator sidecars, and raw
input SHA-256s are archived. The extra multi-qubit research driver is
compiled as a separate example in the temporary QuPRS CI checkout; **the
original QuPRS WMC algorithm is unchanged**.

The six original single-qubit example CNFs pass a standalone Python
exhaustive exact printed-rational Full WMC oracle. All six original cases
also passed three fresh-process GPMC MPFR30 runs and three Rust exact runs
on the same Linux CI runner: 36/36 paired results
[CI evidence](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43540961944).

The representative cohort includes generated circuit motifs (GHZ2–4,
graph3, BV3, QFT2–3, Grover2, QAOA3) and **two unchanged checked-in
Feynman QASM** inputs (`mod5_4.qasm`, `qft_4.qasm`). Each input's
actual GPMC/Ganak weighted CNF and original phase sidecar is checked.
This is neither the full QuPRS end-to-end equivalence pipeline nor a
normalization-level or global-phase equivalence certificate.

## First bounded native Linux results

Two fresh-process executions per solver and case, alternating process
order; original input bytes are shared. Each record separately captures
Boolean SAT, Full Complex WMC, WallTime (including executable startup,
CNF parse, native preprocessing, solver arithmetic and output), and
`/usr/bin/time` peak RSS. Each native process has an 8-second
walltime cutoff and a 2-GiB virtual address-space budget. Numerical
comparison accepts the printed decimal MPFR30 observation in a
tolerance; Rust returns the **exact finite printed decimal-rational**
count. Time is not CodSpeed Simulation.

| Test input | Weighted CNF vars | GPMC native WallTime | RaWMoCS native WallTime | Correctness level |
| --- | ---: | ---: | ---: | --- |
| GHZ2 | 5 | 5.805 ms | 4.055 ms | independent exhaustive oracle |
| GHZ3 | 7 | 4.855 ms | 3.895 ms | independent exhaustive oracle |
| GHZ4 | 9 | 4.859 ms | 3.875 ms | independent exhaustive oracle |
| graph3 | 11 | 10.514 ms | 4.378 ms | independent exhaustive oracle |
| BV3 | 17 | 10.637 ms | 4.641 ms | independent exhaustive oracle |
| QFT2 | 7 | 10.374 ms | 4.299 ms | independent exhaustive oracle |
| QFT3 | 12 | 10.604 ms | 4.459 ms | independent exhaustive oracle |
| Grover2 | 23 | 10.378 ms | 7.154 ms | cross-solver agreement only |
| QAOA3 | 33 | 10.656 ms | 8.452 ms | cross-solver agreement only |
| original Feynman mod5_4 | 43 | **12.242 ms** | 35.700 ms | cross-solver agreement only |
| original Feynman qft_4 | 154 | **16.460 ms** | 2543.906 ms | cross-solver agreement only |

**Decision:** The Rust exact solver has lower native process WallTime on
the small controlled motif cases. The higher-complexity unchanged Feynman
circuits reverse the conclusion: Rust is approximately 2.9× slower on
`mod5_4`, and approximately 155× slower on `qft_4`. Never extrapolate
a small-CNF win into claims that it replaces GPMC on QuPRS workloads.

These are **two** source-pinned runs per tool, not a statistical CI
performance certification. The solvers differ materially in precision
contract (GPMC MPFR30 versus fully reduced exact Gaussian rational of
printed decimal tokens). For Feynman `qft_4`, the Rust numerator and
denominator output alone occupies more than 10k characters across
complex components, versus a finite-precision GPMC representation of
roughly 100 characters. The observed timing includes both arithmetic
and result serialization. More experiments are required to separate
search/branching cost from BigRational growth, normalization and output
and to compare like-for-like numeric guarantees.

### Admission / provenance gates

- **PASS:** Original emitted input and phase-source capture, two solver
  dialect consistency, source SHA, independent SAT/printed WMC for all
  six original cases and representative cases with at most 17 CNF vars.
- **PARTIAL:** Four larger representative circuits (23–154 vars)
  have **cross-solver agreement only**, not an independently enumerated
  Full WMC certificate.
- **NOT CLAIMED:** Quantum equivalence/fidelity, source-phase exactness,
  per-solver fair CPU-only CodSpeed comparison, broad 16–128 qubit
  QuPRS benchmarks, hard production memory safety, or solver replacement.

See [CI workflow](../.github/workflows/quprs-encoder-source-capture.yml)
and the raw per-trial JSON/artifact linked in the passing CI run.
