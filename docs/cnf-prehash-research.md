# Exact prehashed weighted-CNF residual memo keys

**Status:** isolated opt-in measurement, not a production improvement claim.

## Profiling evidence and precise target

[Final original-source Compact Trail CodSpeed run](https://app.codspeed.io/PhysicsQoo/RaWMoCS/runs/6afb1ab7e0499ce31c1e28d6)
profiles the *actual* Rust MPFR research solver, not the independent
BigRational public core. For original QuPRS HWB6 at 100-bit MPFR, the
4.3s **Simulation-modelled CPU** Compact Trail profile shows
`TrailEngine::count_relevant` self 25.18%,
`canonical_compact` literal filtering self 19.12%,
`PackedKey::hash` self 5.20% and `AHasher::write` self 4.32%.
These are separate flamegraph self-time functions; the total
simulation wall-equivalent is not native WallTime. Similar cache-key
cost is visible on ModMult55 and Barenco5. MPFR multiplication is
not the main self-time hot function in this profile.

[Six-arm original weighted-CNF native source-locked CI](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43888223513)
established Compact Trail against Parent-ID on the *same runner*,
seven independent native processes per original CNF/precision:
HWB6 100-bit 517.535→457.996ms, 333-bit 551.704→502.742ms,
while original unmodified GPMC is 154.476/169.089ms. Original
Grover5 is node-censored in all Rust arms.

The new hypothesis is narrowly about duplicated work in *memo
lookup and insertion*. The existing Hashbrown Cache on a miss
hashes the whole variable scope + canonical residual key once to
lookup and again on insertion. A cached full-key hash could
remove the second long hashing pass without changing its meaning.

## Exact, count-safe opt-in implementation

`--weighted-equivalence --trail-count --trail-parent-ids
--trail-compact-residual --trail-prehashed-cache
--hashbrown-borrowed 30 source.cnf` enables a separate research arm.

- Compute one `hashbrown` HashMap-build-hasher fingerprint of the
  **entire exact PackedKey** (variable scope + all sorted reduced
  source-CNF clauses) once on entering a DPLL node.
- Use Hashbrown's `raw_entry().from_key_hashed_nocheck` for lookup and
  `raw_entry_mut().from_key_hashed_nocheck` with
  `insert_hashed_nocheck` for insertion. The supplied hash is from
  **that same HashMap's BuildHasher**; never provide an unrelated
  hash value.
- Hashbrown continues to verify **full PackedKey equality** when
  multiple keys have the same hash. A 64-bit fingerprint is *never*
  treated as a WMC certificate or a proof that source CNFs match.
- Preserve original cache size admission, SAT versus complex-zero,
  both literal polarities and their Complex MPFR weights,
  all Shannon cofactor branches, source CNF and traversal topology.
- Do not use this option outside `--trail-compact-residual`.
  No public core, QuPRS adapter, independent GPMC or default
  search/cache policy is changed.

This is deliberately a small A/B on a confirmed hotspot. Because
large key hashing may be a minority of whole-process time, a gain
may be too small to justify the extra complexity. If so, retain
negative evidence and do not admit to production.

## Independent verification and admission

- Rust independent 100- and 333-bit exact dyadic Full Complex WMC,
  Boolean contradiction, SAT-but-numerical-zero, unit closure and
  disconnected component fixtures plus forty deterministic small
  source CNFs, at cache limits 0, 1, 128 or 256.
- Seven **same-original-CNF** solver arms: GPMC C++,
  Rust Frequency, signed weighted-equivalence, global Trail,
  Parent-ID Trail, Compact Trail and Compact+Prehash.
  Original QuPRS source SHA
  `0b28736651b3740ed3b201edd2efdef3da541e0d`,
  unchanged GPMC SHA `89bbc89d0de918e7a5385740355df55765039230`.
  Seven rotating subprocesses/source/precision (MPFR 100/333 bits)
  for completed HWB6, Barenco5 and ModMult55 cohorts; Grover5 is
  a bounded censored holdout.
- Parent-ID+Compact versus Parent-ID+Compact+Prehash **must be
  byte-for-byte identical in stdout SHA**, SAT, both Full Complex
  WMC components, DPLL nodes, cache hits and decompositions for
  every completed source trial. Mismatch is a failed study,
  not a performance win. Record exact input sha256, p50/p90
  native complete WallTime, observed peak RSS and all failures.
- Add three CodSpeed `rust_equivalence_trail_prehash_mpfr100`
  identities for the same original CNFs. The existing 24 MPFR
  and 68 public formal-core identities remain unchanged; compare
  Simulation to matching same-source Compact control and analyze
  flamegraphs. Simulation estimates modeled CPU, **not** native
  WallTime. Do not compare CPU Simulation values with native times.
- Treat negative, censored and unknown trials honestly.
  Do not promote a small isolated optimization unless repeated
  native WallTime and memory results are worth the complexity.
  Larger GPMC gaps and node-censored Grover5 require distinct
  count-safe search/component algorithms.

Implementation: `benchmarks/mpfr_research/src/trail_engine.rs`,
`benchmarks/mpfr_research/src/main.rs`.
Seven-arm runner: `benchmarks/mpfr_research/tools/quprs_cnf_prehash_ab.py`.
CI: `.github/workflows/cnf-prehash-science.yml`,
`.github/workflows/cnf-prehash-paired.yml` and
`.github/workflows/mpfr-original-cnf-codspeed.yml`.
