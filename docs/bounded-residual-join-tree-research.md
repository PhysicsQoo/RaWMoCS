# Source-locked bounded Join-tree VE on residual Weighted CNF

This is **research only**, under the existing Draft PR. The original
QuPRS-emitted Weighted CNF, original C++ GPMC and released RaWMoCS
production solver are unchanged. The default Rust research path is also
unchanged; only `--trail-bounded-join-tree-ve` activates the experiment.

After exact reversible Boolean unit closure and component split, inspect
a **complete, disjoint, original-source residual component**. Only scopes
with 8..64 still-unassigned source variables, <=512 live parent clause
IDs, and surviving clauses of width <=12 are considered. No partially
satisfied or cross-component source clause can be discarded. Prefix
already owns forced and free-variable signed weights.

Use the already tested exact signed-Complex MPFR dense Factor-VE engine
as a *bucket / Join-tree message-passing* backend, with a source-primal
min-fill elimination order. This is conventional exact variable
elimination, **not** a new tree decomposition algorithm. Every message
preserves Boolean SAT separately from signed Complex WMC. A cycle or
non-treewidth-one residual can now be accepted if its min-fill bucket
width is <=14 and the exact cost preflight allows at most 150,000
factor cells and 600,000 Complex products. Any rejected or capped
attempt discards partial factor work and returns unchanged reversible
Trail. Hard cap: at most **four** structural attempts per solve, with all
negative attempts included; all planning/product work is included in
full native WallTime. No unbounded separator enumeration or width
threshold escalation.

Rust independent tests cover 100/333-bit original signed-complex
literal weights, an original 10-variable connected cycle with
three-literal clause and cancellation, independent exhaustive
2^10 assignment WMC oracle, unchanged Trail parity at cache limits
0/1/128, actual branch-node reduction, and certified width-15
clique NO-GO before factor allocation. The original A0 suite and
cross-platform science remain required.

The **existing** original Feynman and MQT workflows run
GPMC / Rust Inline / Rust Join-tree in seven order-rotated fresh
processes for each original source×precision cohort, with identical
CNF bytes, source SHA, GPMC 30/100 decimal precision against Rust
100/333 MPFR bits, 30s timeout, 2GiB per process, Boolean SAT,
both signed Complex real/imag channels, p50/p90 full WallTime,
peak RSS, nodes and exact Join-tree attempts/completions/products.
All failed and censored cases remain visible, no extra workflow.

Gate before any default/Router proposal: independent numeric correctness
and **nonzero source hard-case join-tree completions**, >=5/7 paired
wins, >=5% p50+p90 gain, RSS <=105% unchanged Inline, and a
narrower same-run GPMC hard-case gap. Negative Feynman/MQT Forest DP
evidence remains permanent: zero completions in all twelve original
source×precision cohorts. If Join-tree also does not hit genuine
hard cases, report NO-GO; do not blindly raise width, products, memory
or probe budgets. This is not a general high-treewidth separator solver.
