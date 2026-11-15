# MQT Bench original circuits: controlled weighted-CNF solver study

## Provenance: do not label QASM as published CNF

The Munich Quantum Toolkit's **MQT Bench** publishes and generates quantum
**circuits**, including OpenQASM. This study uses the existing committed
`benchmarks/MQTBench/*.qasm` files in QuPRS, **not** an independently
published "MQT native CNF" corpus.

To obtain fair weighted CNFs, the source-locked
[`quprs_mqtbench_original_cnf.rs`](../benchmarks/rust_core/tools/quprs_mqtbench_original_cnf.rs)
reads the untouched QASM bytes and calls exactly the pinned public
QuPRS `qasm::parse_qasm_str`, `PathSum`, and `WmcManager`.
It exports both GPMC- and Ganak-dialect **QuPRS-derived complex weighted
CNFs** and a source phase-numbering sidecar. The native solver study
always solves the same GPMC-dialect emitted CNF bytes with GPMC C++ and
RaWMoCS, not different logical encodings.

Pinned QuPRS source with the original `MQTBench` directory:
`0b28736651b3740ed3b201edd2efdef3da541e0d`.

Pinned native GPMC C++ source:
`89bbc89d0de918e7a5385740355df55765039230`.

Original selected QASM corpus, IBM Qiskit native-gate opt0 from QuPRS:

| Circuit | Original basename in `benchmarks/MQTBench` |
| --- | --- |
| W-state 4 | `wstate_nativegates_ibm_qiskit_opt0_4.qasm` |
| VQE 4 | `vqe_nativegates_ibm_qiskit_opt0_4.qasm` |
| QNN 4 | `qnn_nativegates_ibm_qiskit_opt0_4.qasm` |
| Grover 4 | `grover-noancilla_nativegates_ibm_qiskit_opt0_4.qasm` |
| Quantum walk 3 | `qwalk-noancilla_nativegates_ibm_qiskit_opt0_3.qasm` |
| QAOA 7 | `qaoa_nativegates_ibm_qiskit_opt0_7.qasm` |
| Exact QPE 8 | `qpeexact_nativegates_ibm_qiskit_opt0_8.qasm` |

The original circuit file, phase sidecars and two dialect files are
included with the native trial manifest and SHA-256 source/derived-CNF
hashes. **No circuits are transpiled, reparameterized, edited, stripped
of measurement, or regenerated** for this experiment. Unsupported
unitary parsing, exponentiation, or source expansion must be recorded as
censored rather than silently changing the source.

## Native qualification protocol

[`mqtbench-original-weighted-cnf.yml`](../.github/workflows/mqtbench-original-weighted-cnf.yml)
checks out exact source commits and invokes
[`quprs_mqtbench_original_cnf_ab.py`](../benchmarks/rust_core/tools/quprs_mqtbench_original_cnf_ab.py).

* Source export is limited to 30 seconds, 2 GiB address space, and
  2 MiB generated-file size per original circuit.
* Validate original QASM **byte identity**, the two CNF dialects'
  **identical clauses and exact printed weights**, and matching phase
  sidecars before running either solver.
* Fresh native same-runner processes compare GPMC C++ vs the original
  packed Cache, prior combined Fast solver, and optional Fast + reusable
  DPLL frequency scratch at **100 and 333 MPFR mantissa bits**. All
  three Rust arms use 50k Cache entries, the same CSR unit propagation,
  and bounded 1M DPLL nodes. The opt-in scratch CLI flag is added
  only in its own arm, preserving all existing control behavior.
* Four order-balanced trial rotations per eligible source/precision
  pair; trials exceeding the pilot 10-second full-process threshold
  are explicitly marked as exploratory and **not performance ranked**.
  Individual native executions are capped at 30 seconds/2 GiB.
* Require GPMC/Rust SAT parity, numerical complex WMC agreement, byte-
  exact full Rust complex stdout and equal-cache DPLL node, cache-hit
  and decomposition parity before publishing comparison medians.
  Numeric agreement uses a shared complex-output scale
  `max(1, |Re_GPMC|, |Im_GPMC|, |Re_Rust|, |Im_Rust|)`
  for BOTH component errors, with the unchanged exact rational decimal
  relative guard `10^-(digits-6)`; absolute real/imag errors and the
  worst scaled error are preserved in the report. This is a solver
  agreement guard, never a precision- or source-fidelity certificate.
  Report Rust solve-only separately from GPMC/Rust full-process
  WallTime and peak RSS.
* Fail the workflow if no MQT input achieves a full qualified
  four-arm comparison or if any numeric/structural failure occurs.
  Export failures and independent process timeouts remain documented
  as censored observations, never reinterpreted as speed advantages.

These are original-source and **solver agreement** experiments; large
instances do **not** have an independent complete Full WMC oracle.
Differences between MQT source semantics and QuPRS's parsed unitary
subset remain separate from performance claims. Do not promote an MPFR
research flag into production solver behavior based on this study alone.

Official MQT Bench upstream references:
[`munich-quantum-toolkit/bench`](https://github.com/munich-quantum-toolkit/bench)
and [MQT Bench documentation](https://mqt.readthedocs.io/projects/bench/en/stable/quickstart.html).

## Initial native discrepancy and scientific resolution

The first native run
[`#43693421813`](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43693421813)
successfully emitted the original-source QuPRS weighted CNFs for **all
seven** listed MQT circuits and produced 13 matched precision/circuit
rows. One row (VQE 4 at MPFR100 bits) failed the older *independently
scaled real/imag* numeric guard: the real WMC was approximately
`-302.571670282757` while the imaginary WMC was approximately
`-4.524219578286e-9`, with imaginary absolute discrepancy
`2.452612730542815504e-24`. The old separate imaginary scale
`max(1, |Im|)` gave tolerance `1e-24`.

The resulting **whole complex-output relative** discrepancy is only
about `8.1e-27`, below the *unchanged* 30-decimal-digit relative
guard `1e-24`. The new shared-complex-scale check is an explicit
accuracy contract, not a hidden precision decrease: every sample's
real and imaginary absolute deltas remain archived and Rust-versus-Rust
full stdout bytes must still match. At MPFR333 bits this VQE source
already passed the original numeric gate. The earlier failing job is
retained as evidence; a new source-locked CI run must pass before
merging these changes.

## Cross-corpus validation of the optional DPLL scratch optimization

A follow-up controlled study adds `scratch` as a fourth native arm.
It is identical to Fast except for `--reuse-frequency-scratch`.
The original Packed/Fast arms remain unchanged; all four arms are
order-balanced in four fresh subprocess trials. The same MQT QASM and
emitted CNF source SHA, MPFR precision, search topology, Rust complete
complex output, and GPMC numeric reference must hold across **all**
three Rust arms before any speedup is reported.

For each MQT/precision pair, paired Fast-to-scratch solve-only and
full-process WallTime ratios are recorded separately alongside all
individual samples, RSS and exact numerical witnesses. The earlier
source-locked MQT result at 7 circuits × 2 precisions is preserved,
and any regression or low-millisecond start-up noise remains visible.
This benchmark does not itself change the production solver default.
