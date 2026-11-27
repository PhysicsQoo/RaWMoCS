# Shared Rust MPFR API — v0.2.1

The source-distributed `rawmocs_mpfr` library target and `rawmocs` CLI
share the production parser, router and MPFR kernels. The independent
`rawmocs-core` exact rational reference remains unchanged. This is a
source dependency, not a crates.io publication, C ABI or Python binding.
Pin the released source revision or source archive; pre-1.0 API evolution
remains possible. Linux x86-64 source installation and downstream consumption
are the admitted production target; portable reference-core CI does not
certify native GMP/MPFR on every OS.

## Dependency and first solve

From an independent Cargo project, point to the extracted source tree:

```toml
[dependencies]
rawmocs_mpfr = { package = "rawmocs-cli", path = "../rawmocs-0.2.1/crates/rawmocs-cli", version = "=0.2.1" }
```

Use your own committed Cargo.lock. The release consumer gate seeds its lock
from the source package and adds only its root package; the initial
dependency download is not an offline installation guarantee.

```rust
use rawmocs_mpfr::{solve_text, Precision, SolveOptions, SolverMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cnf = "p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n";
    let options = SolveOptions {
        precision: Precision::Digits60,
        mode: SolverMode::General,
        ..SolveOptions::default()
    };
    let report = solve_text(cnf, options)?;
    assert!(report.sat);
    assert_eq!(report.real_mpfr().binary().significand, "0");
    Ok(())
}
```

For files, call `solve_file(path.as_os_str(), options)` on a
`std::path::Path`. Both functions return `Result<SolveReport, SolveFailure>`.
File and text share the same grammar and solve lifecycle.

## Options and outcomes

| Option | Contract |
| --- | --- |
| `precision` | Digits30 (default), Digits60, Digits100, Digits130: 100, 200, 333, 432 working bits respectively |
| `mode` | Auto (default), AutoFrontier, AutoSeparator, Prejoin, General, GeneralSeparator, Reference |
| `timeout_ms` | None preserves the default 45-second solve deadline; Some(0) expires immediately; maximum 86,400,000; Reference rejects Some |
| `large_general` | Explicit bounded streaming admission only with General; default false |

Ordinary file loading/parsing is outside the solve deadline; large General
includes parsing in the deadline. Neither deadline is a hard whole-process
wall-time guarantee, and blocking I/O cannot be forcibly interrupted.
The final decimal formatting follows the existing CLI precision; the
deadline is checked before formatting, not a hard bound on formatting.

A successful report contains Boolean satisfiability, real/imaginary decimal
strings, requested precision, actual completion route, solve_ns and
call-local diagnostics. SAT with zero is distinct from UNSAT. Auto falls back
only on recognized route rejection, on the original problem and remaining
deadline. Frontier and Separator stay opt-in. SolveRoute describes the
actual completed kernel; successful Frontier planning may complete as Prejoin.

`real_mpfr().binary()` and `imag_mpfr().binary()` export the already
rounded finite stored value, as signed decimal integer significand times
2^exponent2, working precision_bits and negative_zero. Zero has canonical
exponent zero. This is lossless for the stored MPFR value, not an exact
mathematical WMC or a certified numerical error interval. No f64 conversion
is required. `solve_ns` includes solve/decimal formatting, excludes ordinary
input loading/parsing and output transport; it is not an end-to-end metric.

A failed call has no numerical count. Inspect `failure.kind` rather than
matching the diagnostic reason string:

| FailureKind | Legacy exit mapping |
| --- | ---: |
| InvalidInput | 2 |
| UnsupportedInput | 3 |
| Timeout | 4 |
| ResourceLimit | 5 |
| RouteRejected | 6 |
| InternalError | 7 |
| Cancelled | 8 (native classification) |

CLI-only compatibility entry points and diagnostic reason text are not the
supported downstream interface. Native successful and failed calls do not
emit CLI protocol or kernel diagnostics to stdout/stderr.

## Diagnostics and cancellation

`SolveDiagnostics` contains optional fallback, streaming_source, general,
separator, frontier_stop, prejoin and binary statistics. Missing means
no corresponding completion event; a fallback or completed phase does not
certify the whole solve. Failure diagnostics are available through
`SolveFailure.diagnostics`. Counters and timings are observations, not
resource guarantees.

Pass a shared `CancellationToken` reference to `solve_file_cancellable`
or `solve_text_cancellable`. A controller may call `cancel()` while a
worker solves; each independent request needs a fresh token. Cancellation
observed at a checkpoint returns Cancelled, with no partial count and
no new fallback. Reference checks only at the boundary. Blocking I/O,
single GMP/MPFR operations, allocator failure and hard interrupts require
external worker supervision. No universal cancellation latency SLA or
in-process hard OOM/RSS guarantee is offered.

## Evidence and scope

See the [shared-core research record](shared-rust-core-v021.md) and
[PR #143](https://github.com/PhysicsQoo/RaWMoCS/pull/143) for candidate
evidence. Three seven-round four-arm native batches exercised the original
QPE-inexact22, Grover6 and GF2^7 CNFs, Auto and AutoFrontier, versus the
published v0.2.0 same-mode CLI. 504/504 rows completed; all six cohorts per
batch passed the 5% p50/p90 gate with independent interval and exact
decimal/plan parity. These are prejoin-completion, fresh-process measurements
including source load/output, not persistent-memory latency or long-case
General fallback certification. Exact-main full science and source package
receipts are required before v0.2.1 publication.

Full complex WMC only. Projected WMC, certified rounding, Python native
bindings, batch API and a QuPRS default solver change are out of scope.
