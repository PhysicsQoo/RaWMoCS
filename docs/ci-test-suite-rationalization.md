# CI test-suite rationalization — candidate qualification

Status: research only. Published v0.2.1 and all historical release admission
policies remain unchanged. This first slice does not remove a test or claim a
smaller case exercises the same path.

## Existing gate inventory

| Existing job or control | Intended coverage | Candidate disposition |
| --- | --- | --- |
| core fmt/clippy/workspace/release validator | engineering and fail-closed release contract | KEEP |
| A0 independent 48 complex / 620 properties / cache / exhaustive | independent semantics and rewrites | KEEP small controls; study moving repeated exhaustive to relevant research |
| four platform workspace jobs | reference-core portability, not production MPFR platform admission | KEEP release; assess change-aware PR |
| CLI locked tests, native stdio and shared-core integration | actual MPFR production, typed selectors, precision, transport, route, diagnostics | KEEP; existing small controls first |
| six real in-flight checkpoints | deterministic cancellation, call-local behavior | KEEP |
| CLI tiny SAT-zero/UNSAT, unsupported, wide General, rejection, deadline, variable cap | count/failure separation and routing | KEEP |
| deep General/lifecycle | depth, cap, external worker cleanup | SHRINK only after boundary evidence |
| QuPRS pinned encoder/solver conformance | external adapter and actual process | KEEP scoped release; examine dependency cost |
| actual kernel OOM and non-OOM signals | real cgroup kill distinction | KEEP minimal real witness |
| source archive, extracted consumer | package, lock, installation and Rust consumer | KEEP release |
| seven large source witnesses | independent count and source fidelity | SHRINK using qualified original small families; preserve large cases as research |
| natural long cancellation | cancellation without instrumentation | RESEARCH_ONLY candidate; use deterministic checkpoints routinely |
| R6 168 rotated transport solves | long fresh-process regression against v0.2.0 | RESEARCH_ONLY candidate; reuse small file/text/CLI parity |
| six original CNFs independently rebuilt | source fidelity and independent arithmetic | SHRINK only after small corpus admitted |
| three GPMC seven-round quadruples | same-source/precision native performance, 5% floor | RESEARCH_ONLY candidate; mandatory for relevant algorithm changes |
| CodSpeed Simulation identities and short QPE8 | algorithm regression and qualified short source | preserve identities; investigate selection separately |

Existing `shared_core.rs` and `native_stdio.rs` already contain small route,
precision, failure, cancellation and transport controls. Do not create a second
suite until their coverage is measured. Candidate labels are proposals, not a
policy change.

## First source-locked experiment

Pinned QuPRS `0b28736651b3740ed3b201edd2efdef3da541e0d`, original QASM
Git blob identities in `small_family_qualification.py`. QPE exact8 reuses
the existing qualified QPE8 source digest. QPE inexact8 and QNN4 use the
legacy Random_Rotation directory, Grover4 and W-state4 use qiskit-2.6.1-opt0,
GF(2)^4 uses Feynman. The QPE exact8 is a same-family candidate, not a claim
that its transpiler/source lineage exactly matches the historical QPE22.

Six families, Auto at 200/432 bits and opt-in AutoFrontier/AutoSeparator at
200 bits, independent 550-bit fixed interval oracle, bounded export/oracle/solve.
No rerun of GPMC timing. Record CNF digest, independent reference digest,
actual route, diagnostic traces, raw outputs and wall time. A fast successful
Auto solve is not evidence that Frontier or Separator activated. All failed or
censored rows are retained and fail this qualification; no new release
receipt may use this report. Subsequent work maps actual branches against
existing small synthetic boundary fixtures, and independently tests fault
detection before changing versioned release policy.

This PR keeps all current required checks and uses one opt-in research step
on its own branch in the existing science workflow. No production changes.
