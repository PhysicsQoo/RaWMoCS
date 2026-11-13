# QOO-220 — SATLIB weighted-input snapshot safety and independent UNSAT gate

**Research only**; source baseline `main@93408b1fda5b41db92f9cf663b9dd2ec9813f242`. The `canonical-mul-research` feature remains **disabled by default**. No new public stable prepared-input API or solver algorithm is enabled.

## Problem identity is CNF **plus weights**

An immutable `PreparedCanonicalProblem` produced from source `A` owns a checked clone of both the Boolean clauses **and the exact signed/complex rational weights**. It is not a generic compilation of Boolean structure that automatically applies to future parameter sweeps. Replacing positive or negative weights produces a **different** weighted model counting problem even if the DIMACS file and SAT status are identical.

`benchmarks/rust_core/tests/satlib_prepared_snapshot.rs` builds pinned original SATLIB `uf20-01` and `uf20-02` (20 variables/91 clauses), prepares source A, and constructs a separate problem B with:
- a new ordinary signed/complex weight pair; or
- four deliberately unreduced, signed and negative-denominator `BigRational::new_raw` components, which B canonicalizes independently at the preparation boundary.

The test retains both prepared instances simultaneously and alternates their repeated solves with cache OFF and ON. Each result is checked against **its own** independent SAT truth table and four-product weighted-sum oracle. It also checks the exact numerator and denominator fields, SAT independently of WMC value, and search statistics parity with the baseline solver. The test requires the two exact weighted counts to differ; otherwise the purported weight-mutation test would be vacuous.

A separate negative control attempts to prepare a new problem with a zero denominator. It **must** fail closed without changing the older valid prepared snapshot; the old SAT/WMC witness remains exact and canonical.

## Provenance and interpretation

The SATLIB **Boolean** CNF bytes and original Git blob SHAs are recorded in `docs/qoo-219-satlib-provenance.md`. All complex rational literal weights are **synthetic** research inputs, not values in SATLIB and not evidence for quantum-WMC workloads.

The `PreparedCanonicalProblem::estimated_retained_payload_bytes()` method is still a **non-exhaustive** logical footprint, not allocator profiling. A prepared object's private storage must not be mutated or reinterpreted to represent changed external weights; the caller must invoke `prepare` on each changed weighted problem and include that cost in end-to-end benchmarks.

## Independent UNSAT gate remains open

The original SATLIB `dubois20.cnf` (60 variables/160 clauses) is currently only **source-labeled UNSAT**, not independently certified by this project. A separate proof-producing SAT run, a pinned checker and a reproducible proof artifact (e.g., verified LRAT) are needed before using it as a certified UNSAT/full-WMC oracle. A naive unconstrained resolution-based elimination can explode exponentially; neither such an attempt nor a filename label substitutes for a checked proof.

Do not include `dubois20` in claimed independently verified WMC performance cohorts until proof, time and memory bounds have been accepted. No default `solve`, `Gaussian::mul`, cache policy, QuPRS or GPMC changes are authorized by this study.
