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
