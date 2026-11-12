# Linux isolated Cache and peak RSS research profile

The research-only executable at
`benchmarks/rust_core/examples/cache_profile.rs` runs a fixed cohort of
exact Complex Full WMC problems. It intentionally does not modify production
solver behavior or claim a cross-platform memory limit.

Run on Linux:

```sh
cargo run --locked --release --manifest-path benchmarks/rust_core/Cargo.toml --example cache_profile
```

The command first executes **two mandatory fail-closed negative controls**
and then invokes **28 fresh child processes**: analytical 16/32/64-variable
equality chains and independent forced pairs, each with cache entry limits
0, 1, 8, and 10,000. Four additional 3-variable cache-hit positive-control
runs use an exact independently derived count: two variables are forced true
while a third remains free. Every child verifies SAT and the independent exact
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
28 records. GitHub-hosted machines can change CPU, memory allocator and
kernel settings; compare caps and formulas **within a single run first**.
Absolute RSS includes the binary and input, is not equal to the size of cache
entries, and can be coarsely page-rounded. Wall-time values are observations
rather than CodSpeed Simulation estimates, and one trial does not prove a
statistically significant speedup.

**Resource-guard research controls (Linux only):** Every normal worker now
runs via the system `prlimit --as=536870912` command. This enforces an OS
`RLIMIT_AS` **512 MiB virtual-address-space** limit, not an RSS limit.
The parent separately observes a five-second deadline, terminates the worker
if breached, and reports a `TIMEOUT` error, never a partial Full WMC answer.

Two explicit negative controls must pass before emitting the CSV: a child
under a 256 MiB `RLIMIT_AS` tries to reserve 1 GiB of virtual space
(`RESOURCE_FAILURE` via the child-specific exit 42), and a separate child
that sleeps beyond its deadline is killed (`TIMEOUT`). The public
`guard-controls.log` artifact records both. Unexpected exit codes or
missing `prlimit` fail the workflow rather than being interpreted as UNSAT
or zero WMC.

**Safety boundary:** Address-space limits cover a different resource than
physical RAM and are not hard RSS caps, a cgroup policy, or a guarantee of
perfect real-time enforcement. A child may still encounter other allocation
failures or be killed externally; nonzero exits and timeouts produce no
certified count. This Linux-only research profile is **not** a production API
or cross-platform QuPRS resource supervisor.

Do not add a hard cache byte cap until the measured payload and RSS
relationship is understood. Subsequent work needs process-level OS-specific
resource enforcement, explicit unknown/error results, adversarial stress
tests and platform qualification.
