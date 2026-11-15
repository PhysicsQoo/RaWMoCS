# Packed residual Cache: isolating hash reuse from full-cache allocation

## Scope and controls

This is an opt-in **research MPFR CLI** experiment only. It does **not**
change production `rawmocs-core`, exact-rational semantics, or the QuPRS
`SolverAdapter` default solver choice.

Earlier `--packed-residual-key-fast` combined two changes. This study
holds the same DPLL search, flat CSR unit propagation, packed exact key,
50,000 Cache-entry limit, and 100/333-bit MPFR precision constant while
crossing the two changes independently:

| Research CLI | Reuse borrowed hash on admission | Skip temporary Arc and scope copy on Cache-full miss |
| --- | --- | --- |
| `--packed-residual-key` | No | No |
| `--packed-residual-key-hash-only` | Yes | No |
| `--packed-residual-key-bypass-only` | No | Yes |
| `--packed-residual-key-fast` | Yes | Yes |

Hash-only preserves the original Cache-full transient allocation behavior;
bypass-only preserves original packed Cache insertion and rehashing. Cache hits
are still looked up even after the Cache reaches its admission capacity.
Every borrowed lookup and hash-reused insertion verifies **full residual and
scope equality** rather than trusting a hash value. Recursion can populate
the Cache before its ancestor saves the result; the admission path retains
its post-recursion exact-key check.

## Native experiment

The source-pinned workflow
[`quprs-feynman-long-source-scout.yml`](../.github/workflows/quprs-feynman-long-source-scout.yml)
runs [`quprs_mpfr_packed_factor_ab.py`](../benchmarks/rust_core/tools/quprs_mpfr_packed_factor_ab.py).
It captures the **original committed** QuPRS Feynman source with QuPRS
`0b28736651b3740ed3b201edd2efdef3da541e0d`, and uses unchanged native
GPMC C++ `89bbc89d0de918e7a5385740355df55765039230`.
Both source SHAs and the original emitted weighted CNF bytes are checked.

* Original HWB6 is the long, qualifying primary case; original Barenco tof5
  and ModMult55 are held-out controls. Grover5 remains censored under its
  separately established 1M-node bound, with **no inferred performance rank**.
* Compare GPMC and the four Rust arms under **identical MPFR mantissa bit widths**:
  30 digits/100 bits and 100 digits/333 bits; Rust arms also use the same
  50k packed Cache capacity and same source CNF.
* Spawn fresh native subprocesses for five complete order-rotated trials,
  with per-process 30-second/2-GiB resource bounds and unchanged node caps.
* Enforce SAT/UNSAT parity, complete complex Rust stdout **byte identity**,
  fixed-cap DPLL node / Cache-hit / component-count parity, and numeric
  agreement with native GPMC. Output contains raw samples, medians and
  same-trial paired solve-only and WallTime speedup ratios.
* Report **solve-only, full-process WallTime, engine drop, and peak RSS**
  separately. The GPMC WallTime is not compared with Rust's in-process
  solve-only time or with CodSpeed Simulation.
* These larger samples provide **source and solver agreement**, not an
  independent complete Full WMC certificate.

## Interpretation gate

Do not assign the previous combined Fast improvement to a specific factor
without these completed source-matched measurements. A result is a
performance claim only if all numeric, structural, timing and resource gates
pass on the same job; censored cases carry no ranking. Interpretation should
also distinguish cases where the Cache is never full (therefore bypass-only
cannot plausibly save Cache-full allocations). Production promotion requires
additional independent correctness and diverse workload evidence.
