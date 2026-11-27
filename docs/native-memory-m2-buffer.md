# M2 factor buffer reuse scout

M1 idle reproduction (CI 46033251397, two orders, 96/96 witnessed solves)
showed stable idle regressions in both Grover6 and QNN12 despite fewer MPFR
allocations and lower peak heap. Verified artifacts 15215731215/15215332601:
Grover calls 4404184 -> 4255372, peak heap 60.18 -> 53.95 MB; QNN
2144796 -> 1518208, peak heap 84.36 -> 74.90 MB. Rust allocation peak
group remains 19.47 MB for Grover and 26.01 -> 22.86 MB for QNN.
The profiles are not a proof that the Rust Vec group causes idle retention.

The next default-off feature `m2-buffer-reuse` requires M1. It reuses the
consumed donor vector only when its scope is exactly the output scope
(pivot independent) and its length AND capacity equal output size.
Each slot's source index is the output index, both branch reads have
finished before replacing that slot, and later outputs cannot read it.
Absent versus numeric zero remains distinct. The vector is transferred
to the output factor; no excess capacity, shrink, unsafe allocator or
pool expansion. Old dead cells overwritten by the output may be dropped
instead of entering the pool: allocation topology may change, which
is the hypothesis, not a guaranteed improvement. Logical work and peak
caps are unchanged. No production default or public Python API changes.

One paired 12-call cohort per arm on QPE Inexact22, Grover6, QNN12:
baseline, M1, M2 from the same source, three independent runners with
rotated orders; exact stored binary plus independent witness; preserve
32 MiB growth, absolute max/median idle RSS, HWM and <=5% median wall
regression. Compare M2 against both baseline and M1. No repeated
Heaptrack on every PR; previously recorded profiles remain available.
A scout is not admission; additional independent cohorts, candidate
platform contracts and QuPRS reentry are required if promising.
