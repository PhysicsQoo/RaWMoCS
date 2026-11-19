# Research-only bounded exact residual key arena reuse

## Source-exact motivation and measured bottleneck

[Original source-pinned key-subphase CI #44264136348](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44264136348)
and [independent original MQT CI #44264136540](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44264136540)
both passed Rust Science, original weighted CNF SAT, matched GPMC 30/100 decimal
digits against Rust MPFR 100/333 bits, signed Complex WMC real/imag and
exact original Trail search topology. **Instrumented** original Inline
HWB6/100-bit key p50 was 253.158ms: 156.163ms (~61.7%) source
literal and view materialization, 55.119ms (~21.8%) order and dedup,
23.089ms (~9.1%) packing, remainder 18.549ms (~7.3%). Independent
MQT QNN4/100-bit showed materialization 11.464ms of 18.844ms key.
These timings are nested within the parent profiler, not isolated CPU
shares in full native WallTime. Exact ordered-key sorting shortcut
previously failed the hard-case performance Go/No-Go despite 75%
strict-order hits on HWB6.

## New single opt-in algorithm arm

`--trail-reuse-key-arena` reuses **only two transient workspaces**
for the existing exact source-clause `canonical_compact`:

- A flat signed residual literal vector, with original exact ordering.
- A packed `ResidualKeyView` vector retaining original clause IDs and
  offsets into the transient flat vector.

The existing source literal scan, canonical comparison/sort/dedup,
zero-terminated key bytes, exact weighted local scope, cache ownership
and branch-order semantics are **not** changed. Fully constructed
`PackedKey` literals and Clause IDs are independently owned; there
is no alias from cache keys into recyclable storage. The scratch
vectors are `mem::take`n per canonical key, cleared before each
source pass, and restored after the complete key materialization,
including Boolean contradiction paths. Retained capacities cannot
exceed 131,072 signed literals (~512 KiB) and 16,384 Clause Views
(~384 KiB), so one unusually large residual cannot permanently
pin arbitrary memory. This is not a global state cache and cannot
reuse stale literal assignments across undo or recursive frames.

Flag is off by default, requires original weighted fused compact
Trail and rejects VE, scoped DSU, post-unit and other experimental
search arms. Separate optional `--trail-stage-profile` retains
exclusive nested clocks, which can measure allocation effects but
cannot prove uninstrumented performance benefits.

Per-process stderr emits
`RESEARCH_MPFR_KEY_ARENA_REUSE enabled=... calls=... flat_reuses=... view_reuses=...`.
A reuse counter is **not** a speed claim.

## Native experiment and negative evidence

New original Feynman seven-arm and independent original MQT seven-arm
CI use: original GPMC, original Rust Inline, prior Ordered-only,
Arena-only, and each of the three Rust arms with source-exact optional
stage profiling. Source-locked QuPRS
`0b28736651b3740ed3b201edd2efdef3da541e0d` and original
unmodified GPMC C++ `89bbc89d0de918e7a5385740355df55765039230`,
original QASM SHA and Weighted CNF SHA, unchanged GPMC/Ganak
dialects, phase sidecars and full positive/negative signed Complex
literal weights. Every complete cohort has seven order-rotated fresh
native subprocesses per arm; 30s/2GiB address-space per child,
observed maxRSS and process-complete p50/p90 WallTime. Grover5
node-limit failure/censors are preserved and counted; no cherry-pick
or substitute tiny fixture for difficult originals.

Each trial must pass SAT, real+imag signed Complex numeric oracle
at exact paired precision, byte-identical Rust output and
DPLL-node/cache-hit/decomposition topology, source parity, correct
arm flag and bounded arena counter inequalities, stage nonoverlap
and source-provenance gates. Rust unit regression covers 100/333-bit,
cache 0/1/128, signed/zero/cancelled weights, contradiction and
original reversible Trail rollback, with Ordered opt-in both
enabled/disabled and profiler both enabled/disabled.

## Acceptance and subsequent work

**Numeric PASS does not imply speed GO.** To consider routing,
Arena-only must deliver repeatable materially better **unprofiled
full-process p50 and p90 WallTime** than original Inline on both
hard Feynman HWB6 and independent MQT QNN4 at both matched
precisions, without worsening peak RSS or losing any original
censored cases; improvements limited to parser, profiler or short
sub-5ms MQT cases do not qualify. A cache hit or vector reuse count
alone is insufficient. Even if faster than Inline, explicitly
compare original unmodified GPMC to judge if the ~2–3× deficit
is materially reduced.

If arena reuse gives little benefit, materialization time is
dominated by re-reading source clause literals/scope checks
rather than allocation; investigate exact incremental residual
occurrence lists, bounded by reversible Trail state, and independently
prove original Boolean SAT, signed Complex WMC and identical
canonical cache keys. Any subsequent independent hypothesis must have its own opt-in research
arm, paired source-exact CI and predeclared No-Go rule. Keep such work in
the **one active research Draft PR** until a separately reviewed,
clean main-based production implementation is justified.
No production defaults or routing changes are authorized here.


## Next controlled experiment: exact generation-stamped scope admission

The original Arena experiment passed full signed-complex numeric equivalence
and deterministic search topology on both source families, but the later
same-head 10%-p50 AND 10%-p90 hard-case performance screen did not pass
all four selected instances. Therefore all existing production defaults
remain unchanged and Arena-only is **an opt-in baseline**, not a router Go.

Instead of a new mutable incidence structure or opening another PR,
the strictly opt-in research switch \`--trail-scope-stamp-admission\`
is accepted **only together with** \`--trail-reuse-key-arena\` and the
already required original fused weighted Trail. It changes exactly
one expression inside \`canonical_compact\`: the first still-unassigned
literal of each candidate source clause is admitted when
\`local_generation[variable] == local_epoch\` rather than the original
\`scope.binary_search(variable).is_ok()\`. The current epoch and all
in-scope variables' stamps are installed by the **existing**
\`prepare_weighted_scope\` immediately before the current residual
key is built. The guard only applies with exact
\`inline_weight_relabel\`; every other source clause path stays on
the original binary-search condition. In Rust debug builds, each
stamp verdict is independently checked against the original sorted
scope membership. No mutable occurrence index, new key
normalization, CNF transformation or MPFR operation is introduced.

### Independent native three-arm comparison

The two existing source-pinned native workflows execute the original
seven-arm Arena benchmark **unchanged**, then run a **separate**
three-arm study with \`quprs_scope_stamp_paired.py\`:

1. The exact pinned unchanged original GPMC executable, using original
   QuPRS-emitted signed Complex Weighted CNF at 30/100 decimal digits.
2. Exact Rust \`--trail-reuse-key-arena\` control with 100/333-bit MPFR.
3. Same exact Rust control plus
   \`--trail-scope-stamp-admission\`.

Original source QASM, CNF, GPMC/Ganak dual dialect and phase sidecars,
checksummed sources and each GitHub checkout SHA are retained. Every
ordinary Feynman or independent MQT source×precision cohort requires
seven fresh process-order-rotated trials, 30s/2GiB bound per child,
complete WallTime p50/p90 and observed max RSS. Original Grover5 remains
a required and explicitly verified DPLL node-budget negative control.

All complete trials require original GPMC SAT and signed Complex WMC
real/imag agreement at same precision; **exact Rust stdout SHA-256,
nodes, cache hits and decomposition equality** between Arena and Stamp.
The Arena arm must show zero Stamp probes, while the Stamp arm requires
positive probes, and both arms retain the same bounded Arena reuse.
Test failures remain separate from completed numerical matches.

The **new incremental research Go screen** is deliberately separate
from the previously fixed *Arena vs Inline* 10%+10% screen: Stamp
must beat **Arena-only** on *both* HWB6 and independent MQT QNN4
at 100/333 bits (four difficult cohorts) with at least **5% lower
unprofiled full WallTime p50 AND p90**, at least **5 of 7 paired
wins** and no more than **5% higher p50 peak RSS**. A fast
short example, a stamp probe counter or a single lucky batch is
insufficient. Report the remaining original GPMC gap and all misses.
No automatic promotion follows even if the incremental screen passes.

The original three-arm report is separate from the seven-arm baseline
JSON so it can be rejected or archived without invalidating the
independently accepted Arena correctness evidence. A failed numeric
or source-provenance gate makes CI fail; a measured performance No-Go
is preserved explicitly without falsely failing numerical integrity.

## Scope Stamp scientific disposition and reversible active-clause bitmap experiment

### Source-locked Scope Stamp outcome: correct but not a hard-case algorithm Go

At the same original-source head, [Feynman native run #44305340744](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44305340744)
and [independent MQT native run #44305340356](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44305340356)
passed six Feynman ordinary and fourteen original MQT source-by-precision
SAT / Signed Complex real+imag WMC, same Rust stdout bytes and exact
DPLL/cache/decomposition topology. The isolated three-arm opt-in
Scope Stamp study found:

| Original source / MPFR | Arena-only p50 | Scope Stamp p50 | p50 gain | p90 gain | Native paired wins |
|---|---:|---:|---:|---:|---:|
| HWB6 / 100 | 566.911ms | 540.748ms | 4.615% | 4.107% | 7/7 |
| HWB6 / 333 | 605.276ms | 580.114ms | 4.157% | 4.867% | 7/7 |
| MQT QNN4 / 100 | 37.801ms | 36.324ms | 3.909% | 4.660% | 7/7 |
| MQT QNN4 / 333 | 40.005ms | 38.164ms | 4.602% | 4.962% | 7/7 |

All four **fail** the predeclared incremental p50 **and** p90
improvement >= 5% screen. Do not infer full algorithm Go or default
promotion from seven paired wins alone. The retained Scope Stamp
three-arm script, source-code opt-in flag and raw artifacts remain
available, but future CI no longer repeats this expensive No-Go arm.
The original seven-arm Arena study remains the unchanged control.

The source-exact Feynman workflow did not finish successfully:
original GPMC Grover5/333-bit crossed the unchanged **30s native
timeout**, though GPMC/100-bit finished (~29.29s). The Rust
Grover5 arms at both precisions remained censored at the original
1M DPLL-node cap. Because Grover5 is a deliberately censored
negative-control cohort, forcing original GPMC to complete at
*both* precisions was an incorrect acceptance criterion. Now only
an original GPMC completed/timeout witness is accepted, preserving
the exact status/exit code/child stderr, while Rust arms must present
the original fail-closed node-limit witness. Any unrelated source
or solver failure remains rejected. Those two censored cohorts are
never reported as numeric matched or hard-case speed improvements.

### Next exact structural hypothesis: reversible active-clause bitmap

The original solver already keeps source-owned immutable
`occurrence[slot(lit)]`, and reversible
`satisfied[cid]` / `live[cid]` counts. The new flag
`--trail-active-clause-bitmap` requires the bounded Arena flag and
**forbids concurrent Scope Stamp** so its marginal effect can be
identified independently. No solver default changes.

A lazily initialized, source-sized `Vec<u64>` records the invariant:

```
active_bitmap[cid] == 1  iff  satisfied[cid] == 0
```

For a satisfying assignment on a literal occurrence, only the
`0 -> 1` change of the satisfied counter clears the bit. On undo,
only the `1 -> 0` change sets the bit. False-branch assignments
that reduce `live[cid]` without satisfaction leave the bit set.
Thus contradictory all-assigned clauses are **still visited** and
must fail-closed, rather than being silently removed. The bitset is
initialized only when this opt-in arm is enabled at the root; no
new literal linkage or independently reconstructed clause state is
maintained. Child recursion reuses the exact same reversible bitmap.

At `canonical_compact`, the full-source branch iterates only set
bits using `trailing_zeros` in original ascending source CID
order; a parent-scope branch filters only already satisfied IDs
without changing the order of the supplied parent list. The
literal-level source order, signed variable rank, sorting,
duplicate elimination, independently owned final PackedKey bytes,
signed MPFR weights and search order remain unchanged.
The bitset is moved out and restored during key visitation only
to satisfy Rust's exclusive borrowing; no `apply`/`undo`
occurs during this visit. The source record emits
`RESEARCH_MPFR_ACTIVE_CLAUSE_BITMAP enabled=... key_visits=...`;
this count is not proof of a performance benefit.

Rust regression compares source clause IDs plus exact canonical
literals before/after assignment, throughout aborts and rollback,
on >= 65 original source clauses (crossing 64-bit words);
and independently checks exact signed Complex WMC, SAT,
branch/cache/decomposition topology at 100/333-bit precision,
cache 0/1/128. This is a stronger invariant than an output-only
numeric smoke test.

### Incremental native benchmark and No-Go screen

Keep the original seven-arm Arena experiment *identical*, and
run a **separate three-arm** matched native comparison:

- Original unmodified GPMC C++ with original QuPRS-emitted
  weighted CNF, 30 or 100 decimal digits.
- Rust Arena-only, 100/333-bit MPFR.
- Rust Arena+Active-Clause-Bitmap, with identical MPFR, cache,
  branch and resource settings.

At every complete source-by-precision cohort, use seven independent,
rotated, native child processes, 30s / 2GiB each, complete WallTime
p50/p90, RSS p50 and per-trial raw data. The original QASM and
CNF/weights/phase hashes, GitHub checkout SHA, signed Complex
real+imag numeric oracle, Rust stdout SHA, SAT, cache and
decomposition topology must match before evaluating performance.
Feynman six ordinary source cohorts and independent original
MQT 14 source cohorts are mandatory, preserving the actual
Grover5 GPMC timeout and Rust node-cap negative outcomes.

The **new predeclared experimental Go** requires, on both
Feynman HWB6 and independent MQT QNN4 at 100/333 bits,
unprofiled full-process **p50 AND p90 >=5% faster** than
Arena-only, **>=5/7 paired faster trials**, **RSS p50 <=105%**
of Arena-only, and zero numeric/source/topology violations.
Short-case performance or high active-bitmap hit rate never
substitutes for this. Failure is an explicitly recorded **No-Go**
without breaking numeric-comparison success. The original GPMC
gap is shown separately; even passing this incremental
screen would not authorize production routing or merging the
entire long-lived stacked research branch.


## Independent native bitmap No-Go and fused pre-unit analysis

[Final-head Feynman native CI #44318897630](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44318897630) and
[independent MQT CI #44318897307](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44318897307)
both **PASS**: 111 Rust tests, all six ordinary Feynman cohorts and
14 original MQT QASM×precision cohorts preserve SAT, original GPMC
signed-complex real and imaginary WMC, same 100/333-bit Rust
outputs and cache/branch/decomposition topology. All six platform
Rust Science jobs and metadata hygiene also passed. Original Grover5
C++ GPMC and Rust node-budget negative controls were preserved;
no censored child was excluded from the original report.

The predeclared **independent marginal performance screen
Bitmap versus Arena-only** fails in all four hard cases,
despite perfect arithmetic/search parity:

| Original source | Precision | Arena p50 | Bitmap p50 | p50 incremental gain | p90 incremental gain | Paired wins |
|---|---:|---:|---:|---:|---:|---:|
| HWB6 | 100-bit | 589.830ms | 592.830ms | **−0.509%** | +0.479% | 2/7 |
| HWB6 | 333-bit | 628.932ms | 633.598ms | **−0.742%** | **−0.844%** | 0/7 |
| MQT QNN4 | 100-bit | 28.691ms | 28.727ms | **−0.125%** | **−0.855%** | 1/7 |
| MQT QNN4 | 333-bit | 30.076ms | 30.249ms | **−0.574%** | +0.996% | 2/7 |

HWB6 recorded median ~6,252,362 bitmap key visits and QNN4
~363,311, without reducing full process WallTime.
Retained [Feynman source evidence](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44318897630/artifacts/14436055929) and [independent MQT evidence](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44318897307/artifacts/14432504870)
are exact original-corpus results, not an invented synthetic
benchmark. **Decision: bitmap correctness PASS; incremental
performance NO-GO**. The research-only switch remains disabled
by default; original seven-arm Arena controls are unchanged.
Stop both bitmap and Scope Stamp micro-optimization branches.

### Next isolated hypothesis: fuse post-unit frequency and component DSU

The original non-post-unit Trail path currently counts live-variable
frequencies by scanning still-live source clauses *after Boolean
unit propagation*, then scans substantially the same clauses again
to build DSU-connected components. Unlike Scope Stamp or active
clause bitmaps, this proposal eliminates a redundant source-literal
traversal, while retaining each variable's old frequency score
and the same canonical representative grouping.

New opt-in \`--trail-fused-pre-analysis\` requires the existing
exact bounded Arena-only research arm, excludes both prior
No-Go Scope Stamp and Bitmap arms, and never changes the
production solver. It initializes exact source-sized DSU parent IDs
only for the experiment, then during each canonical original
post-unit clause traversal:
1. increments the original frequency counter for every still
   unassigned signed literal;
2. performs the exact source-ordered DSU union that the original
   \`components_full\` would later perform, with identical root
   compression and representative assignment.

The candidate then reuses that completed parent vector when
collecting the **same BTreeMap-sorted connected components**.
Original cache key bytes, SAT, MPFR signed weights, free-variable
sum-product contributions, unit propagation and Shannon pivot
ranking remain unchanged. If all variables are eliminated,
no component result is manufactured: the old empty-active result
applies. With the optimization disabled, the old separate
frequency and component scans remain the baseline.

New Rust regressions verify exact signed Complex WMC, cache hits,
DPLL nodes and decomposition topology at 100/333 bits, across
cache limits 0/1/128, forced contradictions and disconnected
weighted components. Native stdout emits
\`RESEARCH_MPFR_FUSED_PRE_ANALYSIS enabled=... clause_scans=... dsu_merges=...\`;
these counts demonstrate traversal, not speed.

The **new incremental three-arm experiment** replaces the now
concluded Bitmap three-arm CI step while preserving all unchanged
original seven-arm Arena controls. It compares original unmodified
C++ GPMC with original source CNF, Rust Arena-only and Rust Arena
+ fused post-unit analysis; seven order-rotated independent child
processes/cohort, process-complete WallTime p50/p90, RSS,
precisely paired GPMC decimal digits and Rust MPFR bits.
It uses the exact same pinned QuPRS/MQT original sources,
CNF/phase sidecars, original Grover5 censored controls,
signed-complex numeric oracle and deterministic Rust
output/cache/decomposition checks.

The marginal **research-only Go screen** requires
>=5% unprofiled full-process improvement **in both p50 and p90**
on four original HWB6/QNN4×100/333-bit hard cohorts, >=5/7
paired wins and <=5% median peak RSS growth, before considering
further integration. Any failed cohort is No-Go.
This is **not** a relaxation of the original Arena-versus-Inline
>=10% p50 **and** p90 requirement, and not a claim of
GPMC parity or production acceptance. Source-exact
final-head CI for the fused variant remains outstanding.


## 2026-11-19: Reversible Literal Mask final exact-source No-Go

Final original-Feynman [CI #44439271063](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44439271063)
and independent original-MQT [CI #44439270806](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44439270806)
both passed; **115 Rust tests**, all **6/6** ordinary Feynman and **14/14**
independent MQT QASM×precision cohorts matched signed Complex real/imag
WMC, SAT, original GPMC precision, Rust stdout SHA and exact
nodes/cache/decomposition topology. Original Feynman Grover5 node-budget
censors were retained. Full raw seven rotated process pairs, wall p50/p90,
RSS and positive/negative source hashes are archived in the original
GitHub Actions artifacts for each run.

The second-stage reversible source-literal-position mask arm was
**correct but strictly not faster** than the same-generation Arena-only
control on the four predeclared hard original-CNF cohorts:

| Original source | MPFR bits | Arena native WallTime p50 | Arena+Literal Mask p50 | Paired faster |
|---|---:|---:|---:|---:|
| Feynman HWB6 | 100 | 464.489 ms | 469.631 ms | 1/7 |
| Feynman HWB6 | 333 | 493.958 ms | 493.589 ms | 4/7 |
| MQT QNN4 | 100 | 27.572 ms | 27.949 ms | 2/7 |
| MQT QNN4 | 333 | 29.202 ms | 29.495 ms | 0/7 |

**Incremental No-Go 4/4** under the already registered >=5% improvement
on both unprofiled full-process p50 and p90, >=5/7 independent paired
wins and <=5% p50 RSS increase. A lower number of scanned inactive
literals does not make the added occurrence-position and bit-update
maintenance free. Do not route, enable by default, or merge this arm.

The associated native three-arm **expensive** study was removed from
the two recurring source-exact workflows after the final evidence
was archived. The small self-test, exact reversible-key Rust regressions,
historical executable flag and all archival negative data remain. This
prevents repeating scientifically concluded No-Go work, rather than
suppressing a negative observation.

### Algorithmic next step

Do not continue allocating another occurrence-index per source clause
without a wall-time cost model. Next investigate either (a) a **bounded
exact weighted component decomposition** that changes the actual DPLL
search-node count without falsifying SAT/Complex WMC, or (b) an
incremental per-residual cost model that *only activates* a structural
scan-skip when the measured number of avoided literal visits exceeds
the reversible-index update cost. Any new path requires separate
opt-in comparisons against original Arena and unchanged original GPMC;
source-exact Feynman/MQT original QASM, signed weights, matched 100/333
MPFR bits, 30s/2GiB, seven process-order rotations and complete
WallTime p50/p90/RSS remain mandatory. In particular, a successful
small example is not sufficient to promote a hard-case algorithm.

The research PR remains Draft and no production solver default changes.


## 2026-11-19: independent exact bounded Factor-VE 11 vs 14 MQT holdout

As the reversible Literal Mask original-CNF native experiment did not
close the GPMC gap, the **next independent algorithmic hypothesis** is
an exact bounded root-level sum-product variable-elimination path that
can reduce DPLL nodes rather than only reduce source-literal scans.

This study is intentionally **first-stage MQT-only**, not a qualified
cross-family production speed claim. It reuses the existing source-locked
MQT original QASM→QuPRS Weighted CNF exporter and the original,
unmodified GPMC C++ executable, with unchanged signed Complex literal
weights, phase sidecars, 30/100 GPMC decimal digits and 100/333 Rust
MPFR bits. Four independently rotated native subprocess arms are
compared in seven trials per MQT QASM×precision cohort:

1. Original GPMC C++, unchanged source.
2. Original Rust Inline DPLL, exactly 1M node cap.
3. Opt-in exact root Factor-VE (existing 11-bag bounded admission).
4. Opt-in exact Factor-VE min-fill/320-scope probe with the **14-bag**
   research cap, holding the original maximum factor products and
   table-cell budget unchanged.

Every completed result must preserve Boolean SAT and real **and**
imaginary signed Complex WMC agreement with original GPMC under the
same MPFR precision. A VE attempt that fails its exact bound must
fall back to original Rust DPLL with unchanged nodes/cache/decomposition
and **byte-identical Rust output**; zero-DPLL-node VE completion is
tracked separately and must not be interpreted as a numeric shortcut
or comparable DPLL branch count. Product/cell and width caps are
fail-closed; a cancelled, OOM, timed-out, or unsupported case remains
explicit in the raw study, not silently filtered.

The CI first runs the original MQT Arena seven-arm baseline unchanged
and only then the separately reported four-arm Factor-VE experiment,
using its own JSON, shell log, and the same archived original CNFs.
A different algorithm may change the solver's search topology, so
topology equality is required for *fallback*, but never imposed on
completed exact VE. The full native p50/p90 WallTime, seven paired
trials, peak RSS, original-GPMC gap, bounded width, cells, products,
completion vs fallbacks and all censored cases are required before
considering the result.

**Promotion remains forbidden** until this MQT independent experiment
shows robust gains on a meaningful difficult QASM cohort, followed by
source-locked Feynman replication on the same matched precision and
a focused main-based production diff. Do not introduce another PR for
this exploratory stage. The sole open research PR remains Draft.
