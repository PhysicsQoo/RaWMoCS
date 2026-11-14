# Source-locked long-QuPRS residual normalization study

**Status: opt-in MPFR research.** The production exact-rational
`rawmocs-core`, default Research CLI mode, original GPMC C++ executable,
and QuPRS solver choice are unmodified.

## Motivating hypothesis

The existing Rust DPLL canonicalizes each residual CNF before its
cache-key lookup via stable lexicographic `Vec<Vec<i32>>::sort()` and
`dedup()`. Feynman QFT4 has more than 5,000 search nodes, many of them
small residuals created by repeated clause assignment and component
split. Compare three semantics-equivalent strategies:

- **Legacy:** the unchanged stable sort and dedup.
- **Unstable:** lexicographic `sort_unstable()` and dedup.
- **CheckSorted:** `is_sorted()` first; skip sorting when already
  canonical, otherwise unstable sort; always dedup.

All three must yield exactly the same normalized clauses, exact
cache-key identity, search structure, SAT and byte-identical MPFR
complex WMC results. Source-normalization at root remains unchanged.

## Comparable evidence

Using the unchanged original QuPRS PathSum/WmcManager source exporter
commit `0b28736651b3740ed3b201edd2efdef3da541e0d`,
run 5 rotating fresh native processes per arm on original Feynman QFT4,
mod5_4, QAOA3, Grover2 and QFT3. Each uses identical GPMC-dialect
weighted CNF bytes and the same 100/333-bit MPFR precision, with
previously qualified borrowed-cache lookup and fast-binary-clause
assignment *on every arm*. The benchmark records exact stdout SHA,
search nodes/cache hits/component splits, native WallTime including
process startup and serialization, solver-only time, peak RSS, and
the number of residuals for which sorting was skipped.

Unqualified SAT, numeric, source SHA, output, search structure,
incomplete telemetry, timeout and resource failures are censored,
rather than omitted. All raw samples are archived in Actions. The
source-scoped previous GPMC/Rust matched-precision study runs in the
same workflow as a control, but it must not be combined across
different CI runs into a speed leaderboard.

A candidate is only interesting if the **longest original Feynman
QFT4** improves materially without mod5_4 or resource regressions.
No default promotion is authorized by this experiment. Earlier
alternative pivot policies ShortClause/MOMS actually regressed
QFT4 by approximately 2.5× on a separate same-precision source-locked
study and remain research-only.
