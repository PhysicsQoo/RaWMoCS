# v0.2.1 Shared MPFR core — first research slice

The portable `rawmocs-core` reference implementation stays independent. The production MPFR parser, arithmetic, router and solver kernels move into the provisional `rawmocs_mpfr` library target within the existing CLI package; CLI and memory/file callers share one lifecycle. No algorithm, precision selector, admission cap or default route is intentionally changed. The existing CLI version remains 0.2.0 during research.

This is not a stable crates.io API or a Python binding. The initial result transports the same rounded decimal strings as the CLI and separates failures from completed SAT/count results. Internal solver diagnostics may still go to stderr. The cooperative deadline retains the original default parse/solve boundary and reference limitation; hard OOM, cancellation and blocking-I/O isolation remain external-process responsibilities. Timing is captured before stdout transport rather than after it; it is not an end-to-end metric.

Admission: existing CLI/source-locked tests, file/memory/CLI parity, SAT-zero versus UNSAT, invalid input and timeout/reentry. Then review typed options/errors, structured diagnostics, explicit cancellation semantics, precise high-precision representation, independent oracle and same-source/precision Native p50/p90/RSS regression against v0.2.0 plus CodSpeed. No release or QuPRS native integration until those gates pass.

## R3 — typed API contract

R1/R2 functional admission on `18e9aadefcd006f7ab6bb2f079c227ca671f4f9a`: Science 8/8, CodSpeed 3/3, 35 existing tests passed (one ignored), both new integration tests passed, and external lifecycle/fault controls passed. Full matched Native performance admission remains R6.

Use `Precision`, `SolverMode`, `SolveRoute`, `FailureKind` and `SolveOptions` rather than raw strings at the provisional library boundary. Precision determines digits and bits; actual route is independent of requested mode; failure kind determines the legacy exit code. The CLI-only hidden compatibility entry point keeps raw selector validation precedence, with the same shared lifecycle and no solver duplication. The existing finite rounded decimal output is preserved. In-process diagnostics and cancellation remain subsequent work; no stable external API promise yet.

## R4 — stored MPFR representation and call-local router diagnostics

R3 admission on `1ade717e7d20552e8c191a85bcd7a2569cc6fe98`: Science 8/8, CodSpeed 3/3, 35 existing tests passed (one ignored) and three integration tests passed. This slice keeps the original finite MPFR components alive without cloning or reparsing them. On demand, `MpfrValue::binary` returns signed integer significand, base-two exponent, actual working precision and signed-zero flag. The representation is exact for the rounded stored value, not a certified mathematical WMC. CLI decimal formatting is unchanged. Call-local `SolveDiagnostics` preserves recognized fallback and streaming source admission on both success and failure without global mutable state. Kernel-specific stderr metrics remain unchanged; their structured migration and a quiet native diagnostic surface are still pending. No change to arithmetic, routing, caps or deadlines. New tests cover binary roundtrip, signed zero, exponent range, recognized fallback and non-leaking diagnostics.
