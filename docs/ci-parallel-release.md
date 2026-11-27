# Release CI critical-path rationalization

Baseline: main full science 46044562938. The durable source job took 763 s:
build 117 s, independent regeneration 215 s, R6 transport 379 s, remaining
52 s. The three GPMC paired jobs only need the regenerated corpus, not
the R6 result. The initial maintenance change moves R6 to a separate
runner after the source and cancellation evidence is archived. All four
downstream jobs remain dependent on the same source artifact; they run
in parallel. The seven rounds, four arms, locked baselines, precision,
witness checks, thresholds, and timeouts are unchanged.

The old release receipt topology remains valid. The split topology
requires successful cancellation/archive and R6/archive steps; a skipped
or missing transport job is rejected. The combined shared-core artifact
keeps its old name and contains both cancellation and transport evidence.
No change to the active v0.3.0 release revision or request.

PR default is fast. On this maintenance branch an explicit [parallel-full]
PR title requests a full rehearsal, not an exact-main release receipt.
Do not enable it while the active release is still running. Compare
wall-clock critical path and actual runner minutes separately; extra
builds may raise runner cost. The 379 s serial step is the available
critical-path reduction, not a measured speedup.

Next: after release, qualify a strict versioned scope classifier using
the published source and complete receipts as reference. Unknown files,
solver or dependency changes, tests, fixtures and workflow semantics
must default to full science. A source/package-only proposal must retain
small original-family numerical checks, independent witness, API/failure
and source installation checks; never allow an unqualified fast run to
authorize a release.

## R6-Light first (shadow rehearsal)

Use previously qualified original QPE Inexact 8, Grover 4 and GF2^4, identical pinned QASM/CNF, fresh independent 550-bit witnesses and the matching original family precision (432/432/333 bits). Compare current CLI, native file, native text once in auto and auto-frontier: 18 solves instead of 168. Verify exact output, plan signature and independent numerical interval. Record walltime for diagnosis only; no single-trial regression verdict. The maintenance PR fast run rehearses this alongside existing witness/cancellation checks; it does not yet replace any required release step. Seven offline receipt controls reject missing/extra/failed arms and count/plan/protocol drift. After actual success, qualify scope-aware promotion against the published release; keep full R6 and the parallel topology for explicit core/transport changes. Do not claim a measured CI reduction from solve count alone.

## Automatic ordinary CI routing

Normal pushes and PRs run bounded six-family numerical qualification and three-family R6-Light, not historical long-corpus regeneration/replay or seven-round R6. Explicit full science retains the original corpus, natural cancellation and parallel seven-round R6/GPMC. Manual science dispatch defaults to fast. The strict exact event-base Git diff classifier chooses whether ordinary CI must include local Python build/API/sdist checks; missing or unknown scope requests them, not a weaker gate. Scope receipts are never release authorization, and do not replace exact published-source comparison for a future package release. CodSpeed routine runs retain compile/tests and short original QPE8 proxy; the 68-identity simulation requires explicit performance intent. Full release receipt and the active v0.3.0 revision are unchanged. Small-family plan drift is a failure requiring explicit requalification, not automatic acceptance. The old v0.2.2 version-specific release policy remains unchanged. A future package-only publisher admission remains pending a published reference and validated exact-main package receipt.

## v0.3.0 same-release integration

The original main 4165785 completed full Science 46044562938 and CodSpeed 46044562898. The one-version `v030-ci-only` admission explicitly checks ancestry and an exact regular-file allowlist of CI/test/docs changes against that revision (all production source, dependency locks, numerical oracles and performance scripts must remain byte-identical). It revalidates both old full main receipts and demands a fresh `[release-package]` exact-main run with new source archive, sdist, local Python/API contract, bounded family witnesses, R6-Light, short CodSpeed and Rust portability. Historical Python platform qualification is carried forward because binding and lock sources are unchanged. No previous source archive may be reused. Ordinary CI does not authorize this release or any later version. PR `[package-rehearsal]` is non-publishing; no large native experiment is triggered. Generic future package-only policy remains separately scoped.

## Manual-only long-running suites

Full native science, all Python platform wheel matrix qualification, R6-Full, GPMC seven-round native pairs and the 68-identity CodSpeed simulation require `workflow_dispatch` with `suite=full`. Push and PR title markers no longer activate long runs. Both workflows default to fast; routine updates and manual experiments have separate concurrency groups. The bounded `[release-package]` source-only admission remains automatic. Historical full release evidence remains valid; new full runs must be deliberately dispatched. No additional workflow or solver changes.
