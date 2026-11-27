# M0: native memory lifecycle attribution

The solver and binding are unchanged from the source-pinned Python API candidate
`abe8316737ab5c90305160ea2d41626482cd65b7`. This diagnostic compares 12 same-process
file solves at 130 digits / 432 bits in a Rust library example and the direct
Python API. Both export the stored MPFR values, discard the solve report, and
must preserve the same independent 550-bit witness and binary signatures.
The original QPE Inexact22 CNF and witness are recovered from an immutable
hashed artifact. Linux glibc counters and /proc RSS/USS/HWM are sampled after
each request. HWM is process cumulative; mallinfo2 is a proxy, not exact
allocator-independent live bytes. Python GC is outside the native solve time.

Separate one-request Heaptrack profiles capture allocation stacks, allocation
volume and peak live bytes, with release debug symbols. Profiled timings are
not performance evidence. M0 records raw RSS growth without treating a
diagnostic pass as resource admission. Neither allocator tuning nor trimming
is used. No factor-table or MPFR pool change is justified until the actual
allocation stacks and repeat data are reviewed.

The PyO3 boundary currently owns the Rust SolveReport through dictionary
construction, exports independent strings/integers, and drops the report on
return; cancellation token ownership is an Arc scoped to the call. The
prejoin executor owns factor tables and a local bounded recycle pool, which
is dropped on return. These are source observations, not a leak proof.
Large factor table buffers, MPFR limb allocation and the input parse / planning
path remain hypotheses to inspect against the profiler. Preserve the existing
32 MiB raw growth and 5% median WallTime gates for eventual remediation;
no admission, release or QuPRS default change is authorized by this experiment.
