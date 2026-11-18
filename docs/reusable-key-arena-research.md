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
