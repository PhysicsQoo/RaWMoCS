# QOO-222 — Sound bounded XOR-from-CNF refutation research

**Scope:** benchmark-only, opt-in via `branching-research`. Main parent at creation `d32dc65e50c94c2d3c4bda65fd7ee5438a96c754`. This is not an enabled production preprocessing pass and does not change `solve()`, `Gaussian::mul`, cache admission, QuPRS or GPMC.

## Soundness and limitations

`benchmarks/rust_core/src/xor_research.rs` takes an arbitrary, unordered CNF clause collection (bounded to 256 variables and 10,000 clauses for the research path). Every accepted XOR equation is backed by **exactly four distinct ternary clauses**, all with the same three distinct variables. These four clauses must forbid exactly the four assignments of *one parity*; this implies the equation requiring the *other* parity. Incomplete groups, duplicate masks, extra clauses on the same triple, repeated-variable clauses, tautologies and mixed-parity groups are conservatively **not admitted**. Other unrelated CNF clauses are ignored by the XOR extractor but remain fully enforced by fallback DPLL.

Every equation stores the **original source clause indices**. The bounded GF(2) elimination maintains an exact symmetric-difference combination of admitted equations. On reaching a zero-LHS/one-RHS row, it returns only that subset as a replayable `XorRefutation`; `verify(variables, original_clauses, proof)` independently rechecks the four original clauses for every row and re-XORs all variable occurrences and RHS values. It must observe each variable an even number of times and a final RHS of one.

**No proof means UNKNOWN / continue original DPLL, not SAT**. A consistent XOR subsystem is not evidence of satisfiability, and an arbitrary CNF UNSAT case without a recognizable XOR contradiction is not refuted by this limited candidate. A proof of Boolean UNSAT entails exact complex WMC zero for any valid weights, but WMC zero alone never proves UNSAT.

## External provenance and science gates

Original byte-pinned SATLIB Boolean CNFs:

- `uf20-01`: 20 variables / 91 clauses / complete independent Boolean truth table 8 satisfying assignments.
- `uf20-02`: 20 variables / 91 clauses / complete independent Boolean truth table 29 satisfying assignments.
- `dubois20`: 60 variables / 160 clauses, checked 40-equation XOR parity contradiction from QOO-220; QOO-221 proved DPLL was UNKNOWN at node budgets 1,32,256.

The CNF bytes and Git hashes are in `docs/qoo-219-satlib-provenance.md`. **Literal complex rational weights are synthetic RaWMoCS research weights**, not supplied by SATLIB. For the SAT controls, exact complex WMC and SAT status are checked with an independent truth-table/schoolbook four-product oracle. Under 256-node Dubois DPLL budget, UNKNOWN is an expected **incomplete** answer; the XOR path's independent source certificate is a distinct kind of mathematical completeness.

The validator also tests clause reorder, mutation/duplicate/removal invalidation and deterministic small satisfiable CNF controls. No false-UNSAT is allowed; all source indices and masks are rechecked before returning a proof.

## Native evidence design

`xor-refutation-research.yml` runs three source files × two cache caps (0/10000) × three alternating orders × two modes (original bounded DPLL, source-checked XOR guard with fallback). The expected **36 PAIRED and 6 SUMMARY** rows are checked before artifact upload.

Each mode runs in a **fresh Linux process** with external `timeout -k 1s 6s`, `prlimit --as=512MiB` and logical node/depth limits. `RLIMIT_AS` caps virtual address space, **not** RSS or kernel cgroup memory.max; VmHWM is observational. Worker guard failure is **not** interpreted as a solver UNSAT result. The case's proof extraction/checking is included in candidate WallTime, and fallback is the unmodified default `solve_with_limits`. Three alternating rounds reduce order bias but do not guarantee low noise.

**Comparability restriction:** baseline Dubois UNKNOWN at a bounded node count versus candidate PROVEN_UNSAT cannot be interpreted as a numeric acceleration of the *same completed task*. The native SUMMARY explicitly emits `baseline_over_guard=NA_DIFFERENT_VERDICT` for this case, rather than a misleading large speedup multiplier; it still preserves the raw per-process WallTime and VmHWM evidence. Measure separate classification and compare only proof completeness under a fixed budget; SAT controls with the same exact verdict allow an overhead comparison. Do not use this study to claim a general solver speedup or default-admission decision.

## Follow-ups

Only after all source and negative controls pass should a future proposal consider general sparse XOR extraction, interactions with component decomposition and cache keys, and independent diverse paper/industrial CNF sources. A production-grade feature would need explicit API review, hardened cross-platform timeout and RSS enforcement under QOO-208, memory footprint analysis, and non-XOR overhead bounds. A source-only XOR proof is not a DRAT/LRAT artifact and has no third-party certificate-checker claim.
