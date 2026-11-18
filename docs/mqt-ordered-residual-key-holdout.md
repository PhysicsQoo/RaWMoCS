# Independent original MQT QASM exact ordered residual key holdout

Research-only stacked Draft: original source inputs, correctness and
**full process** native performance. It is not an in-family Feynman
retest and not an assertion that MQT Bench publishes these CNF files.

## Source provenance and non-contamination

Use the seven *unmodified original* QASM sources in
\`benchmarks/MQTBench/\` at QuPRS SHA
\`0b28736651b3740ed3b201edd2efdef3da541e0d\`:
W-state4, VQE4, QNN4, Grover4, QWalk3, QAOA7 and QPEexact8.
Use exactly the existing pinned unmodified QuPRS WmcManager
native QASM-to-WMC encoder and its same-source GPMC/Ganak CNF
and phase-sidecar agreement gate. GPMC C++ source is pinned to
\`89bbc89d0de918e7a5385740355df55765039230\`.
The weighted CNF and signed positive/negative complex literal
weights are unchanged for every solver arm; encoder time is
excluded identically from native solver WallTime. Verify original
source QASM SHA, byte-identical exported QASM, CNF SHA, dual
dialect semantic weight match and phase sidecar SHA.

The existing independent original-MQT baseline
[native workflow #44254536888](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/44254536888)
already exported all seven QASM and finished 14/14 source×precision
cohorts, but it compared packed/fast/scratch **other Rust
implementations**. Its successes do *not* certify the new ordered
source Trail shortcut. In that baseline the ~100-bit QNN4 case
was ~14.9ms GPMC vs ~62.9ms Rust Fast and Grover4
~4.9ms vs ~18.1ms, showing that the out-of-family slower
cases must not be ignored.

## Predeclared five-arm native acceptance

Original GPMC; unmodified research Rust Inline; opt-in
\`--trail-ordered-key-fastpath\`; original Rust Inline with
\`--trail-stage-profile\`; ordered-key shortcut with both flags.
Exact same \`--cache-limit=50000\`, \`--node-budget=1000000\` and
source weighted CNF across all Rust variants. GPMC native
precision \`-prec=30/100\` decimal digits is paired with Rust MPFR
100/333-bit, using the same pinned source precision mapping.

Run **seven round-robin order-rotated fresh native child processes
per arm per source×precision**, for fourteen cohort total.
Each individual process has a 30s native timeout, 2GiB address
space cap and independently observed GNU time maxRSS. If any
first-rotation case takes over 10s or fails, record all five
results as censored and skip the expensive repeated rotations
for that case; do not selectively omit or relabel it a pass.
All raw per-trial status, precise WMC values, stdout SHA, CNF SHA,
SAT status, nodes, cache hits, AND decompositions, sorted-key
probes/hits, stage times, walltime and RSS are retained.

A complete cohort requires 35 completed native samples; 14/14
complete cohorts is the **strict source acceptance** threshold.
For each rotation, require Boolean SAT parity, exact rational
relative Complex real/imag comparison against original GPMC at
the same precision, bit-for-bit equal *Rust* stdout, identical
Rust DPLL/cache/decomposition topology, correct CLI flags,
stage nonoverlap and bounded timer coverage. Unsigned
root-level \`ordered_key_probes=0\` is allowed and is **negative
algorithm-admission evidence**, not a failed correctness proof.
No signed-weight cancellation is dropped merely because a
SAT proof or net WMC sum is zero.

## Go/No-Go and interpretation

Compare **unprofiled** GPMC/Inline/Ordered p50 and p90 full
process WallTime and peak RSS in the same run and use matched
per-trial complete process differences as secondary evidence.
Profiling clocks and admission hit counts cannot substitute for
speed. Do not call a result a win if only tiny millisecond-scale
cases improve within noise, while HWB6, QNN4 or Grover4 remain
fundamentally slower than original GPMC. Do not infer an exact
large-CNF WMC proof merely from matched independent solvers.

If the ordered shortcut is not repeatably better than original
Inline with no material memory regression across both Feynman
and MQT sources, retain No-Go and study the remaining canonical
key materialization, allocations, weighted class normalization,
and the solver's distinct AND/OR search decomposition.
Production defaults, QuPRS exporter and original GPMC are
unchanged. All private tracker references are excluded from
this public document.
