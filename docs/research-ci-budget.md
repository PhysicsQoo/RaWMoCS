# Research CI budget and evidence policy

This policy keeps correctness checks separate from expensive native research campaigns. It does not relax solver numeric tolerances, resource limits, or source identity checks.

## Routine changes

Use the existing Rust science workflow (production tests, independent A0, portability and one consolidated MPFR research test/Clippy job), public metadata check, and the existing core CodSpeed workflow when its paths match. Historical paired experiments and duplicated MPFR science workflows retain their manual dispatch entry points and original test commands. They no longer start on every pull-request synchronization. Existing resource-safety and portability workflows outside the historical cohort are not removed or weakened.

For the observed 60-workflow fan-out, this change removes 57 automatic PR triggers. The expected routine count for that same change set is three, subject to GitHub path matching and future changes in other workflows. This is a trigger-count estimate, not a measured billing saving.

## Simulation-first, bounded research

Use CodSpeed Simulation as the primary algorithm-development metric and Flamegraph for hotspot attribution; use native wall time and independent numerical witnesses for final long-case solver acceptance. Do not treat simulated CPU time as real elapsed time.

| Mode | Measurement step wall-clock cap | Admission |
| --- | --- | --- |
| Routine Rust regression | 5 minutes | Existing microbenchmarks only |
| Focused paired executable Simulation | 10 minutes | One original short/medium case, both native arms complete within 5 seconds at equal working precision, independent reference within 45 seconds |
| Explicit milestone Simulation | 20 minutes | Manually selected long/legacy study, never routine PR synchronization |
| Long native comparison | Existing 120 seconds per process | GPMC-completing long-tail sources; seven rotated pairs, p50/p90, RSS, independent numeric witness |

The existing Rust CodSpeed workflow is the sole uploader: routine jobs retain their original identities, and the focused original-CNF job runs within that same workflow on the research PR label or explicit dispatch. The default manual dispatch is routine only; the research label selects Grover 5 at 100-bit. The pinned QASM blob and CNF SHA-256 `308d1dc6b243cc7ac188103255e21c699f1adb971e43d5dc8c40828e6a95a128` are checked against the first qualified artifact. Every later comparison requires that same hash. Long Grover 6 and W-state 16 are explicit manual milestone selections only. The complete job ceiling is 25 minutes for focused and 35 minutes for milestone (includes builds/preflight). Do not publish formal ratios from a run with CodSpeed cross-workflow errors or significant syscall warnings; report CPU-only cost as diagnostic and use native acceptance for end-to-end claims. CodSpeed max-time is a benchmark configuration, not a substitute for the hard Actions measurement-step deadline. If either arm is censored, the output/route gate fails, or the action times out, mark the comparison inconclusive; do not publish a speed ratio. Prefer bounded stage profiling for longer cases; these profiles are not end-to-end solver ratios. Existing solver resource limits and native evidence are unchanged.

## Native research

The existing original larger Feynman QuPRS CNF source scout is manual only.

- **Focused (default):** select exactly one case and give the numeric ID of a previous run containing the original source-screen artifact. The mechanism runner verifies source revisions, QASM and CNF hashes and original screen evidence. No source regeneration, broad pilot or unrelated case is run. The single job has a 40-minute ceiling. Missing or expired artifacts fail closed; refresh explicitly rather than silently launching a full campaign.
- **Full:** explicitly choose a full campaign when the source lock changes, a new cohort is needed, or at a research milestone. At most nine jobs: one bounded original Feynman job (30 min), one screen (25 min), three long pilots (35 min each), four mechanism jobs (40 min each). Maximum sum of configured job limits: 320 runner-minutes, excluding platform overhead. Do not use this profile for routine commits.
- **Profiling:** use the bounded existing original MPFR CodSpeed workflow described above; long milestones require explicit manual selection.

Both native comparison modes retain the existing original weighted CNFs, unchanged GPMC, matched 100/333-bit MPFR precision, fresh processes, seven rotated trials, 30-second/3-GiB per-process mechanism limits, raw outputs and numeric witness. The 120-second pilot is reserved for genuine long-tail studies. Do not count fallback or censored time as a completed fast path. Keep numerical discrepancies visible. A skipped or manually unrun study is not a pass.

Batch source changes before dispatch. Review existing artifacts and active jobs first; do not launch a new expensive run merely because another commit was made. No new workflow, branch or environment should be added for a benchmark that can use these entry points. Prefer one focused campaign per research iteration; broaden only after actual numerical and performance evidence justifies the cost. Record runner minutes separately from wall-clock latency and from billing.

## Measured motivation

The previous original-CNF scout (run 45303692980) consumed approximately 136.4 summed job-minutes across nine jobs; a subsequent original MPFR flamegraph run consumed 17.3. These are observed elapsed runner durations, not invoice amounts. They justify separating research from routine correctness; no savings or native speedup are asserted before new evidence.
