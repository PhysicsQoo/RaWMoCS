# QuPRS long-CNF MPFR propagation and assignment ablation

This research-only study compares four variants of the **same
compiled Rust MPFR executable** on the original source-locked
QuPRS-emitted weighted CNF: baseline, combined conflict/unit scan,
two-literal-specialized clause assignment, and both optimizations.
The default path is left unchanged, and there are no changes to
production `rawmocs-core` or QuPRS.

The fused scan retains conflict-first unsatisfiability and the
original earliest unit literal. The assignment variant preserves
all clauses in the original order, dropping satisfied clauses,
removing only the falsified literal in other clauses, and
specializing the common binary-clause case. The benchmark compares
SAT, complete MPFR stdout SHA-256, solver nodes, cache hits,
decompositions, read/parse/solve time, process WallTime and peak RSS.

Source-locked cases are prioritized by worst-cost importance:
Feynman QFT4, Feynman mod5_4, then QAOA3, Grover2 and QFT3.
Two matched precisions (100 and 333 bits) are each measured in
five rotating fresh-process trials per arm. Results are only
admitted if all arms match the source-qualified GPMC/Rust MPFR
reference and preserve the Rust baseline search structure.

The objective is a meaningful **end-to-end QFT4** performance
improvement with no material Feynman mod5_4 regression. A
small microbenchmark win, or a change limited to tiny CNFs,
does not justify any production change. All slower cases and
unqualified outcomes must remain in the workflow artifact.
