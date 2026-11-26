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

See [release admission](docs/first-release-admission.md) for the source release evidence and limits.
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
