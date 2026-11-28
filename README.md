# RaWMoCS

Rust-based Algebraic Weighted Model Counting Solver.

The original RaWMoCS reference library implements **full complex WMC** over exact
Gaussian rational weights. The initial reference engine includes original-CNF
unit propagation, chronological weighted DPLL, component decomposition, and a
canonical residual cache.

The numerical count and Boolean satisfiability are separate; a satisfiable
formula can have zero weighted count. All variables have independently defined
positive and negative literal weights.

## QuPRS weighted CNF executable

RaWMoCS provides an opt-in signed-complex full WMC executable
for weighted CNF emitted by QuPRS. It selects a factor-prejoin route from
the CNF structure, with general MPFR DPLL, component decomposition and
residual caching for ordinary admission failures. It never selects by circuit name or source
benchmark ID. The reference crate and its API remain unchanged.

Build on Linux x86-64 with Rust, GMP/MPFR build prerequisites and a resolved
dependency lock:

```sh
cargo build --locked --release --manifest-path crates/rawmocs-cli/Cargo.toml
crates/rawmocs-cli/target/release/rawmocs --digits 60 input.cnf
```

The CLI dependency lock is tracked. For source installation on the
validated Linux x86-64 target, use:

```sh
cargo install --locked --path crates/rawmocs-cli
rawmocs --version
```

The source-only packaging gate validates installation from an extracted
Git archive, records SHA256 checksums, and does not redistribute a linked
binary. It requires a Rust toolchain, native build prerequisites and access
to the locked dependency sources. Binary packages and other MPFR CLI targets
are not admitted by the portable reference tests. The four supported digit selectors are 30, 60, 100, 130
(working precisions 100, 200, 333, 432 bits). These selectors are working
precision choices, not a certified number of correct decimal digits.
QuPRS should use guarded precision where required and handle execution
failure independently from SAT, UNSAT, and numeric zero.

The source format is strict, with a `p cnf N M` header, `c t wmc`,
optional `c p weight L REAL IMAG 0` records, then one terminated clause
per line. Omitted literal weights are independently one. The finite
decimal grammar and source size limits are enforced. Example:

```
p cnf 1 0
c t wmc
c p weight 1 -1 0 0
```

A complete solve emits exactly a Boolean status and a rounded complex count
(`c r mpfr REAL IMAG`); SAT with zero is valid. Nonzero exit means no
count and empty stdout. Stderr reports INVALID_INPUT (2), UNSUPPORTED_INPUT
(3), TIMEOUT (4), RESOURCE_LIMIT (5), explicit ROUTE_REJECTED (6), or
INTERNAL_ERROR (7). The supervisor must classify external timeout,
cancellation and OOM/kill from process evidence; partial stdout is invalid.
Auto routing uses one solver deadline (45 seconds by default). Set
`--timeout-ms N` (0 to 86400000; zero expires immediately) after `--digits`.
Planning uses at most two thirds of the remaining time and 30 seconds.
The reference diagnostic uses the external process supervisor instead. The general route has no 32-variable admission cutoff;
it retains the parser's input bounds and optional bounded cache. General
search uses heap-resident continuation frames rather than a fixed recursive
depth cap. Frame allocation failure is a resource failure, not a count; The exact-rational reference is an
explicit small-case diagnostic, not the auto fallback. An external process
resource supervisor is still required for hard RSS and whole-process time.
The heap frame guard is not a bound on all allocations.

### Experimental opt-in structural routes (since v0.2.0)

These options were introduced in the published v0.2.0 source release.
The default remains `auto`; the QuPRS adapter does not select the
experimental routes automatically.

| Solver mode | Admission and fallback |
| --- | --- |
| `auto` (default) | Original bounded Prejoin; recognized admission rejection falls back to General. |
| `auto-frontier` | Experimental transactional exact-local stop on the bit cap, followed by bounded Prejoin on the retained factors; recognized admission rejection falls back to General on the original problem. |
| `auto-separator` | Original Prejoin; recognized admission rejection falls back to the experimental separator General route. |
| `prejoin` | Explicit bounded Prejoin; admission rejection returns exit 6, without fallback. |
| `general` | Explicit General search. |
| `general-separator` | Explicit experimental separator General search. |
| `reference` | Small exact-rational diagnostic; requires external timeout supervision. |

Zero-variable auto inputs use the corresponding General route directly.
Recognized admission rejection is distinct from timeout or internal error;
those failures are not retried as a different solver. Auto fallbacks use
the original problem and the remaining shared solver deadline.

```sh
rawmocs --digits 130 --solver auto-frontier --timeout-ms 20000 input.cnf
rawmocs --digits 130 --solver general-separator --timeout-ms 20000 input.cnf
```

A successful Frontier stop is reported on stderr as
`PREJOIN_FRONTIER_STOP`, and recognized fallback as `RAWMOCS_FALLBACK`.
The successful route, working precision and solver time are reported in
`RAWMOCS_COMPLETE`. The completion protocol is unchanged. The explicit
large-input option remains restricted to `--solver general`.

The v0.2.0 release preserved the previously witnessed long-case performance
within the specified regression gate. A genuine Frontier benefit has been
observed on QAOA11 encodings; this is not a cross-family or long-case
acceleration claim. Dense residuals may still reject the bounded plan
(width 19, work 40,000,000, live cells 2,000,000). Neither experimental
route is a new default. See the candidate's review and CI evidence in
[PR #142](https://github.com/PhysicsQoo/RaWMoCS/pull/142).
See the [v0.2.0 release notes](releases/v0.2.0.md) for the admitted scope.

### Explicit large General mode

The CLI supports bounded streaming admission when explicitly selecting
`--solver general --large-general`. The default input limits and auto router
are unchanged.

```sh
rawmocs --digits 30 --solver general --large-general --timeout-ms 20000 input.cnf
```

Large mode has hard limits of 32,000,000 source bytes, 1,000,000 bytes per
line, 65,536 variables, 600,000 clauses and 4,000,000 literal occurrences.
It does not retain the entire source string, but the parsed clauses and
search state remain in memory. The deadline covers parsing and solving.
For a large residual with at least 4,096 clauses, after two scan assignments
and a third pending unit, ordered occurrence-indexed propagation preserves
the original forced-literal and arithmetic order. Small residuals keep the
scan path. This is not an out-of-core solver or a hard process memory bound;
use an external resource supervisor, including for blocking input. The
Python adapter and auto mode do not select this option automatically.
Admission and safe timeout are not proof of a completed count.

### Shared Rust MPFR API (v0.2.1 candidate)

The CLI and the `rawmocs_mpfr` library target in `crates/rawmocs-cli`
use the same production solver. The typed source-distributed API supports
file/text solves, explicit precision and solver modes, lossless stored-MPFR
binary export, call-local diagnostics and cooperative cancellation. No
failure is returned as a numerical count. Native calls do not emit CLI
stdout/stderr. Use an external process for hard memory/time isolation.

See the [Rust API contract](docs/rust-api.md) for dependency setup, examples,
precision and failure semantics, and scope. This is not a crates.io release
or a Python binding. v0.2.1 publication remains gated on exact-main CI.

### Opt-in QuPRS Python adapter

For a QuPRS Python version supporting scoped solver registration (release
CI pins revision `b398f07d6845efc23540549ccbfec717c27b1722`), add
`crates/rawmocs-cli/tools` from the source package to your Python import
path. The adapter does not change QuPRS's default GPMC solver.

```python
from rawmocs_quprs import registered_rawmocs
from QuPRS.interface.ps2wmc import run_wmc

with registered_rawmocs("/absolute/path/to/rawmocs", digits=60):
    count = run_wmc("input.cnf", "rawmocs", timeout=20, preserve_count=True)
```

This selects a 200-bit working precision and the QuPRS external deadline.
Linux `prlimit` applies a 3 GiB address-space cap by default; it is not a
physical RSS limit or kernel OOM classifier. An internal search deadline
can also be selected with `timeout_ms`. The returned `PrintedCount` is a
numerical observation, not an equivalence verdict or certified error
bound; use `RawAdapter.parse_protocol` for the explicit Boolean SAT status.
Registration is scoped and no failure is converted to a count or silently
retried with another solver. This is not a built-in QuPRS adapter, Rust FFI
integration, or default-solver migration.

See [stable release admission](releases/README.md) for the source release evidence and limits.
No claim of competition-wide performance, projected WMC, exact
transcendental phase weights, certified rounding or equivalence checking.

## Scientific baseline

The PR science job checks out the existing **source-pinned QuPRS A0 research
corpus** at commit `abda06877571a32f644451801dccbbb33e8802ac` rather
than copying fixture data into this repository.

- Original independent oracle: 48 complex fixtures (39 Full WMC; 9 projected).
- Native Rust differential tests: all 39 Full WMC fixtures.
- Original 620 mathematical properties include 120 projected checks. These run
  as reference-only tests, **not** native projected solver certification.
- Adversarial cache context: 20 original pairs / 40 states; 32 Full WMC states
  are independently checked against the Rust core.
- Original 65,536-state two-variable rewrite-safety audit and native Rust
  counting comparison for that same source-defined universe.
- Zero and negative weights, SAT-but-zero, rational precision, and bounded
  node/depth/cache behavior are explicitly tested.

## Development

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

The external A0 integration tests are ignored by default and **fail closed**
when invoked without their source-pinned fixture directory. CI checks out the
existing original files and then runs those tests with `QUPRS_A0_DIR`.

The exact reference engine remains a research prototype, **not** a production solver or a quantum-circuit
equivalence checker. The finite node budget and depth limit return an error
instead of a partial count; the cache uses an entry-count limit, not a memory
byte limit. OS-level hard RSS caps, wall-clock timeout/cancellation,
projected WMC, source-exact transcendental phase weights, certified rounding
and QuPRS adapter integration remain outside this scope.


## Native Rust performance benchmarks

The separate unpublished workspace in `benchmarks/rust_core/` defines six
stable Divan-compatible benchmark identities for exact rational arithmetic,
forced/free factors, connected search, disconnected components and an
identical cache-on/cache-off formula. The benchmark fixtures are generated
once, before measurement. Solver calls include internal allocations and search,
but **not** QuPRS parsing, external solver startup or end-to-end verification.

Validate the benchmark source without claiming a speedup:

```sh
cargo test --locked --manifest-path benchmarks/rust_core/Cargo.toml
cargo bench --locked --manifest-path benchmarks/rust_core/Cargo.toml --bench solver --no-run
```

Until the repository is enabled in CodSpeed, actual measurement uploads are
opt-in. After the CodSpeed GitHub connection is authorized, set the repository
Actions variable `RAWMOCS_CODSPEED_ENABLED` to `true` to allow uploads on
performance-bearing PRs or main pushes; `workflow_dispatch` is also available.
The no-upload benchmark smoke job still runs independently. CodSpeed output is
not a required PR check and is not a scientific correctness gate.

The initial CodSpeed Simulation numbers measure **only** native CPU work, not
wall-clock solver-inclusive or QuPRS end-to-end time. Compare only matched
benchmark IDs, compatible instrumentation, and controlled SHA/runner inputs.
Do not infer performance superiority over GPMC or Ganak from this suite.

## Scalability study

See [Exact Full WMC scalability research](docs/scalability-study.md) for
analytical medium-CNF contracts, controlled benchmark series, and resource
safety boundaries.

## License

MIT. See LICENSE.

## Version roadmap

Published v0.2.0 is a scoped Structural WMC and Frontier source release, not a universal GPMC speedup claim. Frontier and Separator remain explicit opt-in; Auto and the QuPRS GPMC default are unchanged. The new cross-family scalable WMC research gate (S4) is **deferred / not achieved** and moves to v0.3.0, with all negative evidence retained in [candidate review #142](https://github.com/PhysicsQoo/RaWMoCS/pull/142). v0.2.1 retains the existing release gates and additionally requires natural cancellation, matched native transport non-regression against v0.2.0 and extracted-source Rust consumer evidence before CI publication.

| Planned version | Scope |
| --- | --- |
| v0.2.0 | Scoped structural WMC, transactional Frontier, exact arithmetic, release admission |
| v0.2.1 | Shared Rust core and library API |
| v0.3.0 | Scalable WMC (including deferred S4), Python binding and packaging |
| v0.4.0 | Opt-in QuPRS native integration and batch WMC |

Planned versions are not publication commitments. The S4 research targets remain at least two distinct original CNF families with successful GPMC runs >=10 seconds, genuine Frontier stops, independent counts and seven matched native trials; target 2x improvement over v0.1.1, QAOA11 gap <=2x, and non-benefiting p50 regression <=5%. No cap or precision relaxation. Only a distinct evidence-backed algorithm hypothesis should reopen the negative scout results.
