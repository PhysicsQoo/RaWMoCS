# Research CI budget and evidence policy

This policy keeps correctness checks separate from expensive native research campaigns. It does not relax solver numeric tolerances, resource limits, or source identity checks.

## Routine changes

Use the existing Rust science workflow (production tests, independent A0, portability and one consolidated MPFR research test/Clippy job), public metadata check, and the existing core CodSpeed workflow when its paths match. Historical paired experiments and duplicated MPFR science workflows retain their manual dispatch entry points and original test commands. They no longer start on every pull-request synchronization. Existing resource-safety and portability workflows outside the historical cohort are not removed or weakened.

For the observed 60-workflow fan-out, this change removes 57 automatic PR triggers. The expected routine count for that same change set is three, subject to GitHub path matching and future changes in other workflows. This is a trigger-count estimate, not a measured billing saving.

## Native research

The existing original larger Feynman QuPRS CNF source scout is manual only.

- **Focused (default):** select exactly one case and give the numeric ID of a previous run containing the original source-screen artifact. The mechanism runner verifies source revisions, QASM and CNF hashes and original screen evidence. No source regeneration, broad pilot or unrelated case is run. The single job has a 40-minute ceiling. Missing or expired artifacts fail closed; refresh explicitly rather than silently launching a full campaign.
- **Full:** explicitly choose a full campaign when the source lock changes, a new cohort is needed, or at a research milestone. At most nine jobs: one bounded original Feynman job (30 min), one screen (25 min), three long pilots (35 min each), four mechanism jobs (40 min each). Maximum sum of configured job limits: 320 runner-minutes, excluding platform overhead. Do not use this profile for routine commits.
- **Profiling:** original MPFR CodSpeed flamegraphs are manually dispatched only when a concrete hotspot needs attribution.

Both native comparison modes retain the existing original weighted CNFs, unchanged GPMC, matched 100/333-bit MPFR precision, fresh processes, seven rotated trials, 30-second/3-GiB per-process mechanism limits, raw outputs and numeric witness. The 120-second pilot is reserved for genuine long-tail studies. Do not count fallback or censored time as a completed fast path. Keep numerical discrepancies visible. A skipped or manually unrun study is not a pass.

Batch source changes before dispatch. Review existing artifacts and active jobs first; do not launch a new expensive run merely because another commit was made. No new workflow, branch or environment should be added for a benchmark that can use these entry points. Prefer one focused campaign per research iteration; broaden only after actual numerical and performance evidence justifies the cost. Record runner minutes separately from wall-clock latency and from billing.

## Measured motivation

The previous original-CNF scout (run 45303692980) consumed approximately 136.4 summed job-minutes across nine jobs; a subsequent original MPFR flamegraph run consumed 17.3. These are observed elapsed runner durations, not invoice amounts. They justify separating research from routine correctness; no savings or native speedup are asserted before new evidence.
