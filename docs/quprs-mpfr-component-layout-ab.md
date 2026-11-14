# Research-only MPFR component-partition cost experiment

A controlled native A/B asks whether replacing an ordered BTreeMap of
decomposed CNF component roots with a directly indexed dense vector can
reduce source-emitted Feynman QFT4 WallTime. The existing production
`rawmocs-core` is **not modified**.

The alternate algorithm retains the same union-find root definition,
component-scope ordering, and ascending component-root iteration as the
existing BTreeMap algorithm. A unit test compares the exact clause
partitions and order across disconnected, connected and assigned inputs.

The CLI uses `--dense-components DIGITS FILE` for the alternate
research path, and the existing `DIGITS FILE` for baseline. A native
five-trial same-runner experiment rotates arm order on Feynman QFT4,
Feynman mod5_4, QAOA3 and Grover2 at both 100 and 333 MPFR bits. Both
arms use the **same binary, precision, original source CNF bytes**,
limits and native process wrapper. The comparator demands identical
SAT, numeric value, complete stdout, search nodes, cache hits,
component splits, and bit precision. All raw trials, including slower
ones, are preserved in `quprs-mpfr-components-ab.json`.

This experiment is a hypothesis, not an asserted speedup. A candidate
is not promoted to the production core without a meaningful **QFT4
end-to-end improvement**, held-out cases, resource checks and proof
of correctness independent of MPFR rounding error. Previous
cross-precision exact-vs-GPMC numbers remain outside this same-
precision speed ranking.
