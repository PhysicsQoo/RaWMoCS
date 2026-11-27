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

## C1 evidence and C2 structural coverage audit

[Science run 45974883395](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45974883395):
8/8 non-skipped jobs, 24/24 numeric rows, artifact 15187440596,
ZIP SHA256 `1e3d48f27deb06d7be465ddc8796f5d874e6784254303901e7e70897c4ea22fe`,
checkout `cac908e5c5515e30620c33a1d08661167fb0e73f`.
CodSpeed and metadata checks passed. All rows actually completed Prejoin:
zero cuts, one leaf; width 5–11, work 4,664–44,678. All solver rows
under 20 ms; total exporter + oracle + solver spans 5.884 s on this runner
(excludes checkout/build; not total CI latency). Source and plan signatures
are frozen for the unchanged-solver candidate; do not impose permanent
identical plans on future algorithm changes without requalification.

| Coverage obligation | Existing minimal control / evidence | C2 decision |
| --- | --- | --- |
| Six original families and numeric support | C1 24 rows, 550-bit witness, source digests | REUSE; routine matrix can be reduced after fault-detection review |
| General/Prejoin/AutoSeparator fallback | lib tests auto_general_on_wide_clause_and_empty_scope, auto_separator_preserves_prejoin_and_fallback_contract | REUSE; 10-var wide clause |
| SAT-zero vs UNSAT / four MPFR precisions | exact_reference_and_prejoin_agree, optimized_general_matches_independent_reference | REUSE |
| General components | component_transfer_preserves_order_and_connected_residual, optimized_general_matches_independent_reference | REUSE |
| Cache key/admission correctness | borrowed_key_matches_owned_exactly | REUSE; actual nontrivial hit benefit remains distinct research evidence |
| Separator scoring and signed count | separator_scores_and_signed_wmc, separator_matches_independent_dyadic_counts | REUSE; scoring override tested directly, actual end-to-end override not yet proven by C1 |
| Genuine Frontier transaction stop | shared_core::kernel_diagnostics_are_route_specific; native_stdio | REUSE existing 24 forced literal fixture; add independent analytic count check (10^6144, imaginary zero) |
| Native file/text/CLI contract and stdio | shared_file_memory_cli_parity, native_stdio | REUSE; no new duplicate corpus |
| Cancellation, timeout, reentry | cooperative_cancellation_is_distinct_and_call_local, in_flight_cancellation_at_real_checkpoints | REUSE deterministic controls; long uninstrumented cancellation research remains |
| Prejoin multi-leaf cut execution | C1 has no nonzero cut | GAP: inspect historical receipts / smallest genuine cut before claiming coverage |
| Long General / hard OOM / GPMC / R6 timing | distinct resource/performance claims | keep historical gates; conditional policy not yet admitted |

No solver or release-policy changes in this slice. The C2 baseline check
fails closed if source, route or observed plan drifts; only this research
qualification uses the pinned plan signatures. The new analytic assertion
must pass CI before its evidence is admitted. Remaining gap: measured
fault-detection coverage, minimal true cut witness (if required by previous
suite), and proof of equivalent routine scope; no blanket removal yet.

## C2 follow-up: existing Separator witness

[Science run 45980898701](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45980898701): 8/8 executed jobs passed; long GPMC job intentionally skipped under the existing fast policy. The witness log reports 24/24 small-family rows, all Prejoin; CodSpeed 3/3 and metadata 1/1 passed. The analytic Frontier count assertion passed as part of the CLI job. The witness artifact is 15188897894 (API ZIP digest sha256:8242e7a99ab73b6066d65178e77540283c45c618213911640fab372d4a39f8fa; not independently downloaded in this check).

The existing eight-variable signed Separator fixture now requires actual search diagnostics `checks > 0 && overrides > 0` at all four precisions in addition to the independent reference counts. This is an assertion on the existing solve, not another expensive case. The direct pivot checks remain. The new assertion is pending CI; a failed assertion means the fixture is not yet a genuine end-to-end substitute, not permission to weaken it. Multi-leaf Prejoin and real cache hit evidence remain open; no release policy is relaxed.
