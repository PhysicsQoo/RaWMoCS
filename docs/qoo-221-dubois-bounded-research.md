# QOO-221 — Bounded source-certified Dubois20 UNSAT versus UNKNOWN

Status: opt-in research only. Base main `4100b08f45c5e67cb979d75419cec79f1ac241d1` (PR #46); no default solver API or arithmetic change.

## Independent Boolean proof
Original SATLIB Dubois20, 60 Boolean variables/160 clauses, source Git blob `fbad81bb8793f08cd9b12b0028d4d741263edb5b`. The checked `dubois_xor_certificate::verify_dubois20_unsat` derives 40 XOR parity equations directly from all clauses; each variable occurs twice, but the combined RHS is 1, producing 0=1. This is an algebraic source proof, NOT a DRAT/LRAT artifact or a solver-runtime qualification.
The complex exact rational weights supplied by `satlib_provenance::fixture()` are synthetic research inputs, not SATLIB WMC data.

## Three-arm bounded resource and logical-verdict contract
The `canonical-mul-research` feature remains OFF by default. Each original/gated/prepared solve uses an identical original CNF, synthetic weights, depth 128, cache cap 0 or 10000, and predeclared node cap 1, 32, or 256.
Any successful `Ok(report)` MUST be Boolean UNSAT and have exact canonical Gaussian zero. A node-budget or depth-budget error is explicitly UNKNOWN, NEVER a proof of UNSAT. All three modes must agree on classification for the same budget. Invalid rational denominators fail closed.

## Source-locked Linux native evidence
The dedicated `dubois-bounded-research.yml` runs the exact contract and an isolated native study: 3 node budgets × 2 cache caps × 3 alternating rounds × 3 modes = 54 PAIRED rows and 6 SUMMARY rows.
Each mode runs in a fresh Linux process. The independent XOR checker executes before timing. The prepared arm includes its preparation in total WallTime; setup is additionally reported separately. Record VmRSS before/while prepared remains alive, VmHWM and an approximate logical retained-data estimate.
The workflow timeout is 15 minutes; the release-mode native experiment is wrapped by `timeout 180s`. This is not a hard memory limit or a general per-child resource-protection contract. QOO-208 owns cross-platform enforcement.
A run that exhausts its node/depth limit is recorded as UNKNOWN; timing UNKNOWN-vs-UNKNOWN must not be interpreted as speed of proving UNSAT. All three arms can legitimately return UNKNOWN.

## Decision
A research-only PR may merge with independent proof, bounded test/CI, native verdict evidence and 62 unchanged historical CodSpeed identities. Do not enable the research feature by default or modify QuPRS/GPMC. Larger limits or external datasets require a separately budgeted study.