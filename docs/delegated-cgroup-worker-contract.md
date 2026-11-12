# Delegated cgroup v2 worker-memory qualification (Phase 2a)

This is a **research-only, opt-in prototype**, not a production solver
supervisor and not a certified Full Complex WMC result. It extends the
earlier visibility-only `linux_cgroup_probe.rs` without changing the
`rawmocs-core` API, QuPRS integration, default Frequency Pivot or Legacy
Cache. It has **two separate execution modes**.

## Default: read-only contract conformance

```sh
cargo run --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example delegated_cgroup_contract -- --self-test
```

On Linux, this verifies independent parsing of local `max`,
`oom` and `oom_kill` counters; strictly monotonic deltas;
counter regression, missing, duplicate and negative values; a
fail-closed classification which **cannot infer OOM from worker exit
alone**; and rejection of `/sys/fs/cgroup` itself as an unsafe
delegation root. It prints:

- `DELEGATED_CGROUP_EVENT_NEGATIVE_CONTROLS=PASS`
- `DELEGATED_CGROUP_LOCAL_OOM_CLASSIFICATION=PASS`
- `DELEGATED_CGROUP_MOUNT_ROOT_REFUSAL=PASS`
- `DELEGATED_CGROUP_NO_KERNEL_WRITES=PASS`
- `DELEGATED_CGROUP_REAL_ENFORCEMENT=NOT_RUN`

No PID is moved, no real cgroup directory is created, and no
`memory.max` file is modified in this default mode. The standard
GitHub-hosted CI qualifies **only this mode** on Linux x64/ARM64.
Windows/macOS report `UNSUPPORTED` for this Linux-only mechanism.

## Explicitly delegated Linux-host experiment (not run on GitHub-hosted CI)

A Linux administrator must first provide a **non-root, private,
delegated cgroup v2 subtree** in which the invoking user is entitled
to create child cgroups and whose `cgroup.subtree_control` already
contains `memory`. Do not point this at the host-wide
`/sys/fs/cgroup` root or any shared production cgroup.

Only on that deliberately prepared host may an operator choose:

```sh
cargo run --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example delegated_cgroup_contract -- \
  --delegated-run /sys/fs/cgroup/<explicitly-delegated-private-subtree>
```

The prototype uses a bounded fixed **64 MiB** `memory.max` per
uniquely named temporary child cgroup and does not change the parent
delegation configuration. For each independent case it:

1. Creates one unique, initially empty child cgroup under the
   explicitly supplied root; sets `memory.max` and checks readback.
2. Snapshots only that child's `memory.events.local`.
3. Starts its **own known cooperative test executable**, blocked
   on a one-byte stdin gate; moves that direct PID to the new child
   via `cgroup.procs`; verifies the child PID is listed *before*
   opening the work gate. It does **not** claim pre-admission
   startup allocations are bounded, so this is not a general
   noncooperative solver containment method.
4. Runs a 4 MiB page-touching positive case that must exit normally
   with no increase in `oom_kill`; separately runs a 192 MiB
   page-touching negative case under the 64 MiB cap. The negative
   case may be classified as **local cgroup OOM evidence** only
   if the worker exits unsuccessfully **and** the private child's
   `max`, `oom` and `oom_kill` local counters all increase.
   Exit 137/SIGKILL, nonzero status, or a parent-wide counter
   change are *never* sufficient.
5. Explicitly waits/reaps the direct worker, checks that the
   child cgroup reports `populated 0`, and removes that temporary
   cgroup. Failure to verify either step fails the run. An RAII
   owner guard attempts `cgroup.kill`, direct-child kill/wait,
   and directory removal on abnormal exits.

A parent-side monotonic deadline prevents an endlessly running
worker. A timeout is an error, **not** a verified OOM. The bounded
test deliberately does **not** run automatically even on a runner
with elevated permissions: the private subtree must be explicitly
specified at invocation.

### Scientific limits

The Linux kernel documents that `memory.max` is the cgroup
memory hard-limit control, while `memory.events.local` contains
nonhierarchical counters. However, `oom_kill` counts processes
in the cgroup killed by **any kind** of OOM killer. The combination
of a uniquely owned empty leaf, checked PID membership, a known
over-limit workload, and both `max/oom/oom_kill` deltas is
much stronger evidence than an exit code, but the prototype does
not eliminate all external kernel-level confounding.

- No CPU, time, process-count, or general RSS sandbox guarantee.
- No `clone3(CLONE_INTO_CGROUP)` atomic start: a cooperative
  stdin gate bounds the workload **after** admission only.
- No child processes or adversarial fork/breakaway cases here;
  separate Unix PGID and Windows Job Object research exists.
- No full solver-native API, exact Gaussian-rational WMC oracle
  run inside this limited worker yet.
- No production support for hard memory-limit attribution unless
  this opt-in test has been successfully executed and reviewed on
  a genuinely delegated host.
- No automatic fallback from a missing permission to an incorrectly
  certified `MemoryLimit`; failures are explicitly reported.
- The guard's Drop cleanup is best effort; an error cannot claim
  complete orphan-free cleanup. GitHub Actions cleanups are not
  a replacement for checking `populated 0`.

The Linux kernel cgroup v2 documentation covers memory
controllers, delegation, `memory.max` and `memory.events.local`:
https://docs.kernel.org/admin-guide/cgroup-v2.html

## Phase 2b (separate acceptance)

Before adopting a production solver-owned resource supervisor:
source-lock a **delegated-host** run with kernel/version, subtree
path, permission status, exact `memory.max` readback, PID membership,
before/after private local events, observed termination and
`populated 0`, and a positive exact Full Complex WMC independent
oracle fixture. Then investigate atomic worker startup,
process-tree cleanup and permission/cleanup fault injection. No
unobserved Linux/macOS/Windows capability may be marked supported.
