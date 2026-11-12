# Scoped Windows Job Object descendant lifecycle: research only

This is a **native Windows, source-only conformance experiment**. It does
not change `rawmocs-core`, the Full Complex WMC solver, default cache,
search pivot, stable API or any QuPRS integration.

## Narrow mechanism

The independent `windows_job_contract.rs` example creates a Windows
Job Object using `CreateJobObjectW`, sets
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` using
`SetInformationJobObject(JobObjectExtendedLimitInformation)`, and
assigns its **direct worker** to that job using
`AssignProcessToJobObject`. This code uses source-pinned Win32
structure layouts and direct `kernel32` FFI, without a shell, helper
executable, or additional Cargo dependencies.

The worker has **gated stdin**: before it receives the one-byte start
token it cannot spawn descendants. The parent creates the Job,
assigns the worker process to it, and only then sends the token.
A worker that cannot be assigned never receives the token; its owner
guard kills and reaps it. This eliminates the uncontrolled period in
which a newly spawned normal-mode worker could create a child before
job assignment, for this cooperative fixture. It does *not* establish
safe assignment of arbitrary, noncooperating programs. Native
`CREATE_SUSPENDED` setup and adversarial escape are future work.

After assignment, the worker spawns a real descendant which inherits
the Job under normal Win32 creation rules. The descendant writes a
monotonically increasing heartbeat to a temporary file every 25 ms.
The parent requires at least three successful heartbeat ticks before
each negative control:

- **TIMEOUT**: monotonic parent deadline, then explicit
  `TerminateJobObject`, followed by a bounded wait for the directly
  owned child.
- **CANCELLED**: a separate flag triggers the parent to close the last
  Job handle (kill-on-close), followed by a bounded wait for the direct
  child.

After each stop, the descendant's previously live heartbeat must
remain stable for 230 ms. Any failure to create, configure or assign
the Job, missing heartbeat, early child exit, failed termination,
failed reap, or continuing descendant activity is **FAIL**, not a
verified OOM, exact WMC/UNSAT verdict, or an assumed success.
The owner guard attempts Job-handle cleanup and direct-child
termination/reaping on errors.

## Explicit boundaries

- **No hard resident memory limit**, memory.event attribution, or
  verified `MemoryLimit` classification. The kill-on-close flag only
  controls lifecycle, not RSS. No other Job resource limits are set.
- No guarantee over processes deliberately escaping a Job through
  breakaway paths; the child here uses ordinary process creation.
- The cooperative stdin gate is intentionally not a production
  security boundary. Robust arbitrary solver startup must use
  OS-specific suspended process creation and strict handle policy.
- No claim that GitHub Windows runners permit arbitrary nested Job
  hierarchies. A restricted runner will fail on
  `AssignProcessToJobObject`, with its Win32 error code, rather than
  reporting a fabricated PASS.
- On Linux and macOS this Windows-only mechanism must report
  **UNSUPPORTED**. Their separate POSIX-process-group experiment
  provides a different and narrower guarantee.

The Windows Job inheritance/nesting and last-handle semantics follow
Microsoft's documentation:
- https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects
- https://learn.microsoft.com/en-us/windows/win32/procthread/nested-jobs
- https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information

## CI and reproduction

```sh
cargo fmt --manifest-path benchmarks/rust_core/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example windows_job_contract -- -D warnings
cargo run --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example windows_job_contract -- --self-test
```

`.github/workflows/windows-job-contract.yml` requires two real
live-descendant witness controls on Windows, explicit unsupported
markers for hard RSS and adversarial breakaway, and explicit
unsupported classifications on Linux x64/ARM64 and macOS ARM64/Intel.
The five independent CI artifacts contain the exact source-linked
results. Successful compilation or running the test on Unix is
**not** evidence of Windows Job Object support.

Next steps: OS error classification for nested jobs and permissions,
reliable noncooperative startup, descendant escape controls, Linux
cgroup v2 delegated capabilities, and physically meaningful memory
limit evidence. Keep these separate from the current proof.
