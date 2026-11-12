# Linux cgroup v2 visibility and delegation: Phase 1c capability probe

This source-only experiment asks which **prerequisites** for Linux
cgroup v2 worker isolation are available on a particular runner. It
does **not** constrain any worker's resident memory, move a process,
change `memory.max`, or conclude that an exit was caused by OOM.

## What the probe actually does

`benchmarks/rust_core/examples/linux_cgroup_probe.rs` uses the
stable Rust standard library, not shell commands or elevated
permissions. On Linux it:

1. Checks `/proc/self/mountinfo` for a cgroup v2 mount at
   `/sys/fs/cgroup`, and observes a unified hierarchy membership
   entry (`0::/...`) in `/proc/self/cgroup`.
2. Reads `memory.events` at the visible mount root and
   independently parses a nonnegative unique `oom_kill` counter.
   Also checks the root's `cgroup.subtree_control` for `memory`.
3. Attempts to create one uniquely named **empty cgroup directory**
   under that mount root, removes it immediately on success, and
   reports only directory-create capability. Failures are
   `UNAVAILABLE`, not presumed proof of full delegation.
4. Runs independent positive/negative parser controls (counter
   increase, counter regression, missing/negative/duplicate values)
   without modifying the kernel's own cgroup files.

The mount root visible to a process is **not necessarily identical**
to the process's own cgroup or a delegated subtree. Therefore, even
when empty subgroup creation succeeds, it does not establish
permission to migrate workers into that subgroup or to enable a
memory controller in it. The probe never writes
`cgroup.procs`, `cgroup.subtree_control` or `memory.max`.

Printed status vocabulary:

| Key | Allowed values | Meaning |
| --- | --- | --- |
| `CGROUP_V2_INTERFACE` | VISIBLE / UNAVAILABLE | cgroup2 mount and unified membership can both be observed |
| `CGROUP_V2_MEMORY_EVENTS` | READABLE / UNAVAILABLE | visible mount root has syntactically valid counters |
| `CGROUP_V2_MEMORY_SUBTREE` | ENABLED / NOT_ENABLED / UNAVAILABLE | mount root's delegated children report memory enabled |
| `CGROUP_V2_SUBGROUP_CREATE` | AVAILABLE / UNAVAILABLE | reversible empty directory creation at the observed mount root |
| `CGROUP_V2_MEMORY_ENFORCEMENT` | NOT_VALIDATED | **never** an RSS or memory.max enforcement claim |
| `CGROUP_V2_OOM_KILL_ATTRIBUTION` | NOT_VALIDATED | **never** a causal link from a worker failure to an OOM |
| `CGROUP_V2_NO_PROCS_MOVED` | PASS | probe source contains no migration or limit write |

On Windows and macOS the feature is `UNSUPPORTED`, not a
false cgroup-specific PASS. No root access, host-level modification,
cgroup remount, or global control is attempted.

## Why memory.events alone is insufficient

The Linux kernel specifies that memory controls are hierarchical and
delegated. Enabling a domain controller also has top-down and
no-internal-process constraints. `memory.events` may count events
for a whole subtree: comparing shared counters before and after a
worker exits cannot establish which worker caused an event.
An observed `oom_kill` change may be informative, but *only
after* separate verified process migration to a dedicated cgroup,
predeclared memory.max, control of other child processes and
documented cleanup. A return code, failed `try_reserve`, `RLIMIT_AS`
or memory.events from an unrelated group cannot be relabeled as a
verified hard physical-RSS violation.

Kernel reference:
https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html

## Reproduction and CI

```sh
cargo fmt --manifest-path benchmarks/rust_core/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example linux_cgroup_probe -- -D warnings
cargo run --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example linux_cgroup_probe -- --self-test
```

`.github/workflows/linux-cgroup-probe.yml` executes on Linux x64/
ARM64 plus Windows and macOS ARM64/Intel. For Linux, it requires
source-controlled parser negative tests and explicit status strings
with **no** claim of hard RSS or OOM causality. For other platforms
it requires explicit UNSUPPORTED. Each OS produces a 30-day artifact
with its actual source-pinned capability record.

Next stages must independently test verified current-cgroup
delegation, exact worker PID migration, cgroup.kill/descendant
lifecycle, scoped memory.max, memory.events before/after with
predeclared attributable worker effects, and permissions/cleanup
failure injection. This capability probe is not such a test.
No `rawmocs-core` or QuPRS API changes are made.
