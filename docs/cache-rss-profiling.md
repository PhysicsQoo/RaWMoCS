# Linux isolated Cache and peak RSS research profile

The research-only executable at
`benchmarks/rust_core/examples/cache_profile.rs` runs a fixed cohort of
exact Complex Full WMC problems. It intentionally does not modify production
solver behavior or claim a cross-platform memory limit.

Run on Linux:

```sh
cargo run --locked --release --manifest-path benchmarks/rust_core/Cargo.toml --example cache_profile
```

The command invokes **24 fresh child processes**: analytical 16/32/64-variable
equality chains and independent forced pairs, each with cache entry limits
0, 1, 8, and 10,000. Every child verifies SAT and the independent exact
Gaussian-rational closed-form WMC answer. A parent checks the child exit code,
and terminates it if it exceeds its research-only 5-second timeout; a failure
prevents a successful evidence report.

Each process emits one CSV row with these columns:

- `solve_wall_ns`: in-child elapsed time around `solve_with_limits`,
  excluding fixture construction and independent oracle computation
- `process_peak_rss_kib`: Linux `/proc/self/status` **VmHWM**, the
  high-water resident memory including Rust runtime, code and other allocations
- `nodes`, `cache_entries`, `cache_hits`, `decompositions`: solver work
- `key_scope_items`, `key_clauses`, `key_literals`, `value_bits`,
  `max_value_bits`: the separate logical cache payload diagnostic counters

The independently archived GitHub Actions CSV comprises one header and exactly
24 records. GitHub-hosted machines can change CPU, memory allocator and
kernel settings; compare caps and formulas **within a single run first**.
Absolute RSS includes the binary and input, is not equal to the size of cache
entries, and can be coarsely page-rounded. Wall-time values are observations
rather than CodSpeed Simulation estimates, and one trial does not prove a
statistically significant speedup.

**Safety boundary:** The 5-second parent timer detects/kills a cooperative
research child when observed, but is **not a guarantee of hard RSS or perfect
real-time enforcement**. A worker may exceed available memory or be killed by
the OS before it can report. This profiling harness is not a public API or
QuPRS production resource supervisor.

Do not add a hard cache byte cap until the measured payload and RSS
relationship is understood. Subsequent work needs process-level OS-specific
resource enforcement, explicit unknown/error results, adversarial stress
tests and platform qualification.
