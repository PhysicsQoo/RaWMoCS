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
missing `prlimit` fail the workflow. Normal worker nonzero exits are labeled
`WORKER_FAILURE` with no assumed cause (which could be an oracle mismatch),
not automatically `RESOURCE_FAILURE` or UNSAT/zero WMC.

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


## Optional logical cache quota: independent cohort

After the default 28-row cache-entry-cap cohort, the same research workflow
now generates an **independent second CSV** named
`cache-quota-profile.csv` with exactly **21 rows** plus its header.
This preserves the old source-scoped CSV and its accepted schema.

For equality chains and independent forced pairs at 16, 32 and 64 variables,
and for the 3-variable cache-hit positive control, each fresh child process
uses the same exact CNF and weights with cache-entry cap 10,000 under three
fixed logical-payload admission policies:

| Policy | scope items | clause items | literal items | exact value bits |
| --- | ---: | ---: | ---: | ---: |
| `legacy` | no payload quota | no quota | no quota | no quota |
| `tight` | 2 | 2 | 2 | 32 |
| `roomy` | 1,024 | 1,024 | 2,048 | 16,384 |

The `tight` case must reject at least one would-be cache insertion, the
`roomy` case must reject none, and all cases must match independently derived
Gaussian-rational Full WMC oracles and remain SAT. The 3-variable
`legacy` and `roomy` cases must exercise actual cache hits.
Every row records `cache_payload_rejections`, cumulative logical payload
and counters alongside elapsed solve time and Linux process VmHWM.
The parent launches each case with the existing Linux `prlimit` 512 MiB
virtual-address-space limit and 5-second deadline. Missing `prlimit`,
worker failures, or timeouts are not accepted as valid data.

The two CSVs measure different policies: their numbers are directly
comparable **only within their own controlled cohort**. VmHWM is process
resident-memory high-water, not retained cache size, and the Linux
`RLIMIT_AS` is virtual-memory address-space enforcement, not an RSS cap.
Small or mixed RSS differences on these synthetic cases do not certify
lower memory use in general or establish a production supervisor.


### First qualifying source-pinned measurement

The first completed 49-case Linux profile at GitHub Actions
[run 43176065711](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43176065711),
source commit `6b9b75e659293947ea4e748d36829d21a9a38677`, produced
the following **single-trial** values for 64-variable analytical fixtures:

| CNF | policy | retained entries | payload rejections | cache hits | VmHWM KiB | solve WallTime ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| chain64 | legacy | 3 | 0 | 0 | 2,476 | 1.780 |
| chain64 | tight | 0 | 3 | 0 | 2,468 | 1.109 |
| chain64 | roomy | 3 | 0 | 0 | 2,472 | 1.136 |
| pairs64 | legacy | 97 | 0 | 0 | 2,480 | 1.233 |
| pairs64 | tight | 2 | 95 | 0 | 2,448 | 1.231 |
| pairs64 | roomy | 97 | 0 | 0 | 2,520 | 1.222 |

The 3-variable cache-hit control retained one actual hit under **all three
policies**, while tight declined one insertion. Exact SAT and Gaussian-rational
WMC oracle checks passed in every case. Existing 28 legacy rows and the two
negative controls also passed.

These synthetic 64-variable cases have **zero cache hits**, so they do not
establish whether logical quotas help memory in realistic CNFs with repeated
subproblems. Peak RSS differences here are small, sometimes reversed, and
subject to page rounding, allocator state and scheduler variability. No
statistical runtime gain or reliable physical memory saving is inferred.
The research workflow keeps both old and new CSVs as short-retention Actions
artifacts; this table permanently records only selected, explicitly scoped
observations. Run future controlled repetitions before choosing a production
cache policy. Standalone `--quota-suite` runs must also pass both fail-closed
negative controls independently.
