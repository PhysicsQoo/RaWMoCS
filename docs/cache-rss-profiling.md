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


## Analytically exact, cache-reusing residual cohort

The third independent CSV, `cache-reuse-profile.csv`, contains **36
worker records** plus one header: 4, 8, and 16 independent *repeated
residual* gadgets (12, 24 and 48 variables), three numbered trials each,
and four policies (cache disabled, legacy entry-limit, tight logical
payload quota, roomy logical quota). Policy order is rotated across trials
before measurement; each worker is a fresh Linux process. The original
28-case profile and 21-case quota profile retain their established schemas.

Each gadget is the four-clause CNF `(x|y)&(!x|y)&(x|z)&(!x|z)`:
it forces `y,z` true and leaves `x` free. Its two DPLL branches
produce an identical residual, creating a genuine cache-hit opportunity.
The source-pinned independent complex-rational WMC oracle is

`(1/2 + i/3 + 3/5 - i/7)(2+i)(1-i) = 733/210 - 37i/70`.

For `g` gadgets, the exact expected Full WMC is the `g`th power of
this fixed Gaussian-rational factor; the Boolean formula has `2^g`
models. Every run verifies SAT, this independent WMC count, positive cache
reuse when enabled, zero hits with cache disabled, and tight-quota
rejections. Cache disabled, legacy, tight and roomy variants solve the
*identical* CNF for each gadget count. No projected WMC or arbitrary phase
weights are asserted.

Each of the 36 workers is subject to the same five-second deadline and
Linux `RLIMIT_AS=512 MiB` **virtual address-space** limit, and the
suite independently requalifies memory-denial and timeout negative controls.
CSV outputs record trial, Cache hits, payload rejections, solve WallTime,
and per-process `VmHWM` Peak RSS. These are **three source-scoped trials**,
not statistically independent runner generations: shared hardware and
scheduler can bias them. A policy-induced Cache Hit difference is a valid
algorithmic observation; whether it translates to lower Peak RSS or
faster WallTime remains an empirical question. No default Cache setting,
production OS supervisor, or solver core has been modified.

### First controlled cache-reuse trial: observational result

The first [source-scoped Linux qualification run](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43177804973)
on branch commit `e31119b34626b91fcf70b89225b85cf017b97256`
completed the **28 original + 21 quota + 36 repeated-residual cases** and
all three pairs of resource negative controls. The 36 new worker rows each
passed SAT and an independently derived exact Full WMC oracle.

The following **median in-child solve WallTime** values summarize three
independent child processes per policy, but only one shared CI runner:

| Gadgets (variables) | Cache off | Legacy | Tight quota | Roomy quota |
| --- | ---: | ---: | ---: | ---: |
| 4 (12) | 0.096 ms | 0.100 ms | 0.089 ms | 0.089 ms |
| 8 (24) | 0.221 ms | 0.245 ms | 0.215 ms | 0.206 ms |
| 16 (48) | 0.554 ms | 0.558 ms | 0.558 ms | 0.585 ms |

For the 16-gadget family, Cache Hits per worker were **0 off, 16 legacy,
1 tight, 16 roomy**. Retained cache entries were **0, 33, 1, 33**
respectively. Yet no consistent WallTime benefit appears. The 48-variable
median process VmHWM values were **2,288 / 2,340 / 2,292 / 2,308 KiB**
for off / legacy / tight / roomy, without a reproducible advantage.

This result establishes that the cache-hit instrument and admission policies
actually discriminate workloads, **not** that memoization is helpful or
harmful for realistic weighted CNFs. A trivial residual is cheap to
recompute; cache-key construction, lookup and source-exact rational arithmetic
can dominate. Repeated CPU scheduling and page-quantized VmHWM further limit
inference. Next research should use bounded, source-pinned nontrivial
shared-residual graphs and compare bigger cached computations; do **not**
change the default Cache policy from these measurements alone.


## Connected shared-residual cycle study (source-pinned; pending measurement)

A separate **research-only** fourth CSV, `connected-cycle-profile.csv`,
tests more costly identical residual subproblems than the original
three-variable gadget. For an undirected cycle of `n` positive `y` variables,
each edge `(y_i | y_(i+1))` is guarded by two selector clauses:

```text
(x | y_i | y_(i+1)) & (!x | y_i | y_(i+1))
```

The pair is logically equivalent to the original edge, with selector `x`
free. On either value of `x`, the first DPLL decision reaches the same
connected, non-unit cycle CNF with `n` vertices. The selector appears in
`2n` literals and each cycle variable appears four times, so the existing
frequency heuristic selects `x`. This construction deliberately supplies
one nontrivial reuse opportunity, not a claim of realistic QuPRS inputs.

Both selector and cycle literals use exact Gaussian-rational weights.
An independent two-state transfer recurrence computes the exact Full
Complex WMC by conditioning on the first cycle variable and forbidding
adjacent false literals, including the closing edge. Small cases also
compare the recurrence against an independent exhaustive Boolean
assignment enumeration. No projected WMC or independent cancellation
safety theorem is inferred from this family.

The new cohort compares the **identical input CNF** with cache on/off at
6, 10 and 14 cycle vertices. Each policy × size has three fresh Linux
child-process trials; policy order alternates by trial. The existing
five-second parent watchdog, Linux `RLIMIT_AS=512 MiB` **virtual-address-space**
restriction, and memory-denial/timeout negative controls are run again
for this standalone suite. Every worker verifies SAT and exact WMC
before writing its CSV row. The gate also requires cache reuse under
the enabled policy, no reuse/entries when disabled, and fewer deterministic
search nodes with caching enabled. Rows record process VmHWM and elapsed
solve time, which are **observations**, not proof of speedup or reduced
memory consumption.

The `--cycle-suite` command is separate from the historical 28-, 21-
and 36-row studies; their schemas and old benchmark identities stay
unchanged. Six new native CodSpeed Simulation identities supplement the
previous 31. An exact-source successful GitHub Actions run and a
comparable runner analysis are required before reporting numeric
WallTime/RSS or changing the default cache policy.


### First source-qualified connected-cycle observation

Source SHA `c70f166f1837f831f52753de52a0453fca97de9d` passed
[Linux profile run 43181375405](https://github.com/PhysicsQoo/RaWMoCS/actions/runs/43181375405):
all 18 source-locked connected-cycle workers, historical 28/21/36
workers and all suite-specific memory-denial/timeout negative controls
completed successfully. The CSV `connected-cycle-profile.csv` and
`cycle-guard-controls.log` are retained as short-lived Actions artifacts.

Median in-child solve WallTime over **three fresh processes per policy
on one GitHub-hosted Linux runner**, with paired exact input and
alternating policy order:

| Cycle vertices (total variables) | Cache OFF median ms | Cache ON median ms | OFF nodes | ON nodes | ON cache hits |
| --- | ---: | ---: | ---: | ---: | ---: |
| 6 (7) | 0.152425 | 0.082715 | 23 | 13 | 1 |
| 10 (11) | 0.398046 | 0.260749 | 75 | 37 | 9 |
| 14 (15) | 1.266434 | 0.483446 | 231 | 69 | 25 |

At 14 cycle vertices, the OFF/ON elapsed-time median ratio is about
**2.62x**, and ON visits about **70% fewer search nodes**. This is a
meaningful synthetic positive control for expensive repeated residuals,
unlike the cheap 3-variable control with mixed/non-improving WallTime.
For the 14-vertex family, median process `VmHWM` is **2352 KiB OFF
versus 2368 KiB ON**, so the experiment **does not** demonstrate lower
physical peak RSS. Results are from a shared runner with only three
trials per policy; they cannot establish a robust real-world speedup,
projected WMC benefit, QuPRS end-to-end improvement or a change to the
default cache policy. Additional controlled hardware and representative
CNF families remain necessary.
