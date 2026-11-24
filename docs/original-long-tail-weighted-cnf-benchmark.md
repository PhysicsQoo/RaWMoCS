# Original source-locked long-tail WMC comparison

## Why this exists

Subsecond original Feynman and MQT Weighted CNF cases are not a sufficient
competitive benchmark against C++ GPMC. Short cases remain exact SAT and
signed-Complex WMC regression fixtures, **not the primary performance
scorecard**. We will not infer Weighted CNF difficulty from qubit count,
QASM file size, gate count, or the runtime of a different tool.

An expensive case is eligible only if the **actual original 100-bit native
GPMC or RaWMoCS screen** consumes >=10 seconds full-process WallTime,
or reaches the same 30-second per-process screen timeout. A screened
timeout remains censored, not an invented number or automatically a win.

## Source corpus (16 pinned original QASM candidates)

Original QuPRS commit: 0b28736651b3740ed3b201edd2efdef3da541e0d.
Original GPMC C++ commit: 89bbc89d0de918e7a5385740355df55765039230.

Feynman: barenco_tof_10, gf2^6_mult, gf2^8_mult, gf2^10_mult,
adder_8, ham15-med, qcla_mod_7.

MQTBench: qnn_8, qnn_12, grover-noancilla_5/6, qwalk-noancilla_5,
qaoa_11, qpeexact_16, qft_16, wstate_16.

All are **existing** immutable original QASM files in the pinned QuPRS
repository. They become **QuPRS-encoded** Weighted CNF with exactly the
original pinned public PathSum + WmcManager API. They are *not* called
MQT-published CNFs and they are not generated synthetic structures.
A new research-only bridge copied into the checkout is not a change to
QuPRS itself; sources and algorithms are not transpiled or rewritten.

The experiment runs inside the **existing**
quprs-feynman-long-source-scout.yml workflow in its
original-long-tail-source-locked-pilot job. No new workflow, environment
or branch. Scoped to Draft research PR #128 or manual dispatch.

## Bounded research scout

- Each source export is bounded to 35s, 3GiB virtual address,
  16MiB file size; unexportable sources are recorded CENSORED_SOURCE_EXPORT.
- QuPRS original QASM bytes are checked against copied QASM; source
  and CNF SHA256 are retained. Original CNF parse, variable and clause
  counts, source status and exporter errors are retained.
- Source CNF >12MiB is censored. No inferred solve result.
- Every exported source is *first* screened with the same source CNF,
  original GPMC C++ and Rust Inline at 30 decimal digits / 100-bit MPFR,
  full process wall time, 30s / 3GiB AS for both. This stage discovers
  expensive cases; small cases are not paired again.
- Select at most TWO source-neutral expensive cases, sorted by confirmed
  completed screening cost ahead of censored. Both GPMC-hard and
  Rust-hard cases qualify; selection does NOT depend on who wins.
- Exploratory pilot: two order-rotated fresh processes for each arm,
  GPMC 30/100 decimal digits vs Rust MPFR 100/333 bits, **120s/3GiB
  per solver process**. Keep peak RSS, true full WallTime, Rust DPLL
  nodes/hits/decompositions, SAT and both signed Complex real/imag WMC.
  Compute exact decimal-rational relative numeric witnesses only when
  both solvers finish. Any disagreement fails after evidence is saved.
- Pilot is **not a production or general performance GO**. Two trials
  are for expensive source discovery only. Genuine hard-case speed
  claims require a later seven-trial replicated holdout with p50/p90,
  normal source balances and resource-budget sensitivity.
- One solver timing out cannot be presented as a verified WMC equality;
  censoring is preserved and tied to the bound, not a made-up runtime.
  When both finish, a numeric mismatch is a blocking failure.

## Interpretation

The 2026-11-23 7-trial source-pinned research evidence remains
important but is mostly subsecond. On HWB6 100-bit, full process
p50 Rust Inline 394.348ms versus GPMC 134.215ms, and bounded residual
Join-tree 394.365ms. This is an interesting negative hard-STRUCTURE
control, **not** a 10-second long-tail benchmark.
Original MQT QNN4 also remains subsecond, despite earlier competition
being described informally as "hard cases".

The primary next deliverable is a source-file and SHA-locked original
CNF corpus with actual >=10s native observations, no false runtime claims,
and a broader family-balanced long-tail selection. If no sources qualify,
state NO_ACTUALLY_MEASURED_LONG_CASES_IN_SOURCE_BATCH rather than use
short cases as a substitute. In that case, extend acquisition of
source-verified original CNF collections before making a broad
competitive solver-performance claim.

## 2026-11-23: first run and node-budget fairness correction

Source-locked original [run #45131338063](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45131338063)
completed successfully. 16 source cases, 14 CNF exports, 3 GPMC 30s
timeouts (`mqt_qnn_12`, `feynman_ham15_med`,
`feynman_gf2_8_mult`), 2 sources censored at the exporter.
All four selected 120s GPMC 100/333-bit cohorts reached 120s timeout.
Rust had `RESEARCH_MPFR_FAIL_CLOSED: node limit` exit 2 at the
research executable's **default 100,000 nodes**, well before the wall
limit. These are NOT completed Rust results, solver timeouts or OOMs.
No native long-tail Rust/GPMC ranking can be concluded from that run.

The corrected, opt-in Rust research CLI flag
`--node-budget=1000000000` permits up to one billion DPLL nodes;
**the research executable's 100k-node default remains unchanged**,
as do its existing 250k/1M explicit limits. The long-tail paired runner
adds this flag *only to the Rust arm*; it does **not** disable
`/usr/bin/timeout` (30s screen, 120s pilot),
`prlimit --as=3221225472` (3GiB virtual address), or output limits.
A node-limit hit remains a separately recorded
`CENSORED_NODE_BUDGET`, not a timeout or an invented solver duration.

Up to **three** actually measured long-screen sources are admitted to
pilot, with GPMC>=10s/GPMC timed-out cases prioritized before
Rust-only long cases. This preserves difficult original-source evidence
and avoids spending the entire expensive pilot on short GPMC cases.
Screening remains source-neutral and does not rank by which solver wins.
Any potential solver win on a censored GPMC run lacks a dual-solver
full numeric witness, so it is **not** a certified WMC comparison.
These are *research bounds only*, not production API changes.

## 2026-11-23: screen-first archive and parallel independent hard-case pilots

Completed corrected research [CI #45136524085](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45136524085)
generated 16 pinned source candidates, 14 original Weighted CNF exports
and 3 cases where *both* solvers timed out after 120s:
MQT QNN12, Feynman GF(2)8 Mult and Feynman Ham15-med.
Each case was attempted twice at 100-bit and twice at 333-bit
precision for each solver, with no dual completion or independent
full Complex WMC witness. Calling these ties or speedups is invalid.

The same original 100-bit CNF screen also shows several *clear
timeout-bound regressions*: GPMC GF(2)6 Mult 4.350s versus
Rust >30s, QNN8 1.034s versus Rust >30s, Adder8 0.202s
versus Rust >30s, and W-state16 5.215s versus Rust >30s.
Those should be prioritized for algorithmic diagnostics; do not
promote small-case victories to general long-case superiority.

The single prior job serialized up to 48 minutes of native solver
timeout windows. It is now split **inside the same existing workflow**
into (a) a bounded 16-case source and 30-second GPMC/Rust screening
job, with independent immutable source/CNF SHA and censored-result
artifact, and (b) three independently bounded, parallel matrix jobs
for the three *previously confirmed* long cases.

Each matrix pilot first downloads the identical screening artifact,
verifies pinned QuPRS/GPMC revisions, all 16 source QASM identifiers,
original QASM and QuPRS-generated Weighted CNF SHA-256 values,
precision and high research-only Rust node budget, and that its
precommitted case is actually selected in the screened data.
It **does not regenerate** Weighted CNF or redo expensive screening.
Each case uploads its own per-case JSON/logs even when other
cases time out or fail. There is no new GitHub workflow, Pages
environment, branch, or production API change.

Matrix cases are intentionally pinned to this independent screening
cohort. On any future changes to source selection, do not silently
benchmark a different case: update the cohort and verify source
lock explicitly. These remain exploratory two-trial pilots, not
the eventual seven-trial p50/p90 long-time acceptance evidence.

## 2026-11-24: seven-trial GPMC-completed/Rust-timeout mechanism cohort

The source-screen and all three 120s two-sided-timeout pilots passed in
[CI #45169735316](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45169735316).
Each hard case produced its own original-CNF source-locked artifact.
For original QNN12, Feynman GF(2)8 Mult, and Ham15-med, both solvers
reached 120s at 100 and 333 bits, two trials per precision. Their
signed Complex WMC values remain unavailable; do not label their
timeout times as finished solver WallTime or a solver ranking.

The highest-priority *discriminating* source-locked cohort comes from
the same 100-bit original QuPRS source screen: Feynman GF(2)6 Mult
(GPMC 5.612s, Rust >30s), MQT QNN8 (1.349s versus >30s), and
MQT W-state16 (6.870s versus >30s).

An **additional matrix job in the same existing workflow** now uses
the completed source-screen artifact (no re-encoding) and verifies
all 16 original QASM paths/hashes, copied source bytes, original
Weighted CNF SHA, pinned upstream commits, both numeric precisions,
and that each predeclared case was actually GPMC-completed/Rust-
timeout in the 100-bit screen.

For each of these three cases it measures **seven fresh, alternating
GPMC-versus-Rust native processes** at *both* 100 and 333 bits,
under equal 30s/3GiB and explicit, finite research-only Rust
1B-node bounds. For every GPMC completion it checks stable SAT and
signed Complex real/imag outputs at the stated precision. For any
dual completion it additionally verifies GPMC versus Rust numeric
agreement; a mismatch fails closed and preserves complete evidence.
If Rust remains censored, it reports only a **strict lower bound**
of Rust time / GPMC p50, with a null Rust p50 and null p90
rather than estimating an unobserved completion time.

The same original C++ GPMC build additionally receives one
single-trial *exploratory* feature-toggle probe per precision:
combined -no-pp_ee/-no-pp_ve/-no-pp_cs, -no-td, and -no-bj.
Each is still original-CNF, source- and precision-matched, and
timeout-limited. A toggle can change downstream solver states:
it is **hypothesis triage, not isolated causal proof**.
No new workflow, environment or production solver changes.

## 2026-11-24: replicated GPMC tree-decomposition mechanism and SCC hypothesis

The matched original-CNF GPMC/Rust 7-trial native run
[#45283520291](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/45283520291)
passed all 60 workflows; GPMC finished 7/7 trials on the three source
families at both numeric precisions, whereas Rust exceeded 30s in **all**
seven attempts per case/precision. Thus Rust has no measured completion
p50/p90; only a lower bound on the ratio to completed GPMC p50 is valid.

| Source/precision | GPMC p50 (s) | Rust status | Strict Rust/GPMC p50 lower bound |
| --- | ---: | --- | ---: |
| GF(2)6 Mult, 100-bit | 4.2624 | 7/7 30s timeouts | >7.04x |
| GF(2)6 Mult, 333-bit | 4.5510 | 7/7 30s timeouts | >6.59x |
| QNN8, 100-bit | 1.3535 | 7/7 30s timeouts | >22.16x |
| QNN8, 333-bit | 1.4988 | 7/7 30s timeouts | >20.02x |
| W-state16, 100-bit | 5.2566 | 7/7 30s timeouts | >5.71x |
| W-state16, 333-bit | 5.7961 | 7/7 30s timeouts | >5.18x |

Single exploratory original GPMC toggles (100-bit) with the same SAT
and signed Complex WMC as the corresponding default run:

| Original CNF | Default (s) | -no-td (s) | -no-pp_ee -no-pp_ve -no-pp_cs (s) | -no-bj (s) |
| --- | ---: | ---: | ---: | ---: |
| GF(2)6 Mult | 4.262 | 9.614 | 0.963 | 4.345 |
| QNN8 | 1.354 | 6.822 | 1.563 | 1.353 |
| W-state16 | 5.257 | 14.897 | 5.751 | 5.235 |

The 333-bit toggles show the same qualitative pattern. **These toggles
are single trials, not an isolated causal proof**: each changes downstream
search states and its effects may interact with other mechanisms. Disable
preprocessing only as an exploratory control, not as a blanket default.

### Exact signed-literal SCC reduction, conditional on full weight transfer

For a binary clause \`(a OR b)\`, add implication arcs \`¬a → b\` and
\`¬b → a\`; for unit \`a\`, add \`¬a → a\`. A strongly connected
component of literals proves equality of truth values for members.
If \`v\` and \`¬v\` share an SCC, the binary/unit implication subsystem
is inconsistent. Otherwise construct a polarity-preserving mapping
\`m(-v) = -m(v)\`. Transfer **both** original variable weights
to their representative before substituting clauses:

\[
w'_r(1)=\prod_{v:\lvert m(v)\rvert=r}
      w_v(\mathbb{1}_{m(v)>0}),\qquad
w'_r(0)=\prod_{v:\lvert m(v)\rvert=r}
      w_v(\mathbb{1}_{m(v)<0}).
\]

Here each \`w_v(1)\` and \`w_v(0)\` is the original *signed Complex*
literal weight; no intermediate floating-point approximation or
\`WMC != 0\` SAT shortcut is permitted. The full formula WMC remains
equal because assignments satisfying the implied equalities are in
bijection with assignments to representatives and the product
includes each original literal factor exactly once. Preserve a separate
Boolean SAT channel, especially where signed Complex factors cancel
to zero or include zero weights.

The **research-only, standard-library** script
\`benchmarks/mpfr_research/tools/quprs_long_tail_scc_poc.py\` checks this
transformation against a small *exhaustive* Boolean SAT + signed Complex
WMC oracle, including zero and cancelling weights. It then verifies
the SHA-locked actual QuPRS-produced CNF and reports a hypothetical
SCC-based structural substitution; the production solver is unchanged.

| Original source | Candidate variables eliminated | Clauses before → after | Greedy min-degree induced width before → after |
| --- | ---: | ---: | ---: |
| GF(2)6 Mult | 6 | 512 → 476 | 18 → 18 |
| QNN8 | 104 | 749 → 541 | 15 → 15 |
| W-state16 | 209 | 2341 → 1923 | 20 → 20 |
| Qwalk5 | 368 | 2582 → 1846 | 13 → 13 |
| Adder8 | 24 | 1880 → 1816 | 14 → 14 |

Greedy *min-degree induced width* is **not optimal treewidth**, and
lower variable/clause counts are **not proven native solver speedups**.
Source-only network graph inspection additionally found that many
articulation points in QNN8 and W-state16 disappear after SCC merging:
those were mostly shallow equivalence chains, not independent
low-width decomposition opportunities. The surviving weighted core
still needs genuinely separator-aware exact counting.

### Next algorithmic proof obligation

Favor source-CNF native **Project-Join Tree / separator-aware exact
weighted counting**, after validating the sign-preserving SCC factor
ownership. For a separator \`S\` and pairwise disjoint interiors \`X_i\`:

\[
\operatorname{WMC}(F)=
\sum_{s\in\{0,1\}^{S}}
\left(\prod_{v\in S}w_v(s_v)\right)
\prod_i
\left[
\sum_{x_i\in\{0,1\}^{X_i}}
\mathbf1[F_i(s,x_i)]
\prod_{v\in X_i}w_v(x_{i,v})
\right].
\]

Each weight is owned by exactly one separator or interior node, and
SAT status must follow a **separate Boolean OR/AND recurrence**;
complex cancellation must not turn a satisfiable instance into UNSAT.
Require an exhaustive tiny-oracle proof, real original SHA-locked
CNF correctness at 100/333 bits, and a *measured* 7-trial long-case
WallTime/RSS benefit before promoting any Rust fast path. Avoid
unbounded dense factor tables, and retain capped, explicit fallback
to the unchanged exact solver.

No changes to QuPRS/GPMC source or production RaWMoCS defaults.
