# Source-locked MPFR residual Cache Key storage A/B

The previous borrowed-cache optimization avoids deep-cloning the
canonical Residual CNF **on a hit**, but still clones every clause and
literal on a miss to create an owned Cache Key. In a high-sharing
search such as original QuPRS Feynman QFT4, this may dominate
per-node overhead.

This research-only experiment tests a second, orthogonal idea:
after a borrowed, content-equality checked Cache lookup misses,
**move the already normalized Residual Vec into an Arc**. The
stored Cache Key keeps a cloned Arc pointer, not a deep copy of its
clauses. On a hit, check the original Vec **before** allocating an
Arc. Hashing and equality still cover the complete variable scope and
entire canonical clause set, never only the hash.

The two arms use the same native Rust MPFR executable, identical
original QuPRS emitted GPMC weighted CNF bytes and the same
100/333-bit MPFR GPMC precision contract. Both use fast two-literal
clause assignment, the original Frequency pivot, 10,000-entry cache
limit and hashbrown borrowed lookup. Shared mode is an opt-in
`--shared-residual-key` flag and requires
`--hashbrown-borrowed`. Default engine and production exact
`rawmocs-core` are unchanged.

On original Feynman qft_4, mod5_4, QAOA3, Grover2 and QFT3, run
five alternating independent native processes per mode per
precision. An admitted performance record must match SAT, full
MPFR output bytes, exact DPLL nodes, cache hits and component
decompositions, original source SHA and precision bits. Report
separate process WallTime, solve-only time, peak RSS and every
timeout/OOM/censored case. The independent exhaustive Full WMC
oracle only applies to the small eligible control cases; larger
instances use source-locked cross-solver and exact-reference
comparisons rather than independent exhaustive proof.

No production change or merge is authorized by a positive
microbenchmark alone. The primary decision is long QFT4
end-to-end improvement without mod5_4 degradation or memory growth.
