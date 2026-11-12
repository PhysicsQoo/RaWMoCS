# Scoped Unix descendant cleanup: source-locked research

The Phase 0 conformance probe in `worker_contract.rs` verified only
**direct-child** lifecycle management across operating systems. This
separate experiment evaluates the narrower, incremental guarantee
that a parent can terminate a **dedicated Unix process group** and
stop an actual descendant that remains in that group. It is not a
production solver supervisor, nor a sandbox.

## Mechanism and evidence

On Linux and macOS, `std::os::unix::process::CommandExt::process_group(0)`
places the directly supervised worker in its own PGID (equal to its PID).
This is supported by stable Rust and does not require executing a
shell or configuring a Cargo dependency. The source explicitly checks
that the PGID is positive and not equal to the controlling parent's
process group. When the test requests timeout or cancellation,
the parent uses the POSIX `kill(-pgid, SIGKILL)` operation and then
`wait()` on the **direct** child. Unexpected parent errors trigger
best-effort group termination and direct-child reap via an owner guard.

An actual subordinate child process inherits the worker's PGID,
verifies this via `getpgid(0)`, and writes a monotonic tick number
to a temporary heartbeat file every 25 ms. The parent refuses to
proceed until at least three heartbeat writes prove the descendant
was running. It then triggers two separate terminal reasons:

1. **TIMEOUT**: parent monotonic deadline expires after 130 ms.
2. **CANCELLED**: a separately triggered cancellation flag is set
   after 65 ms while the deadline remains 130 ms.

In both cases, the group is signalled, the direct child is reaped,
and the previously live descendant's heartbeat must remain
unchanged over a further 200 ms. The mechanism is deliberately scoped
to descendants **remaining in the isolated PGID**. A stable
heartbeat is evidence of stopped observable activity under these
conditions, not proof the OS has reaped a grandchild or that an
adversarial process has not escaped by creating a new session/group.

A failed signal, premature worker exit, missing live descendant,
heartbeat that advances after termination, or any other unexpected
condition is a **failed test**, not a successful timeout/UNSAT/WMC
result. The example emits explicit PASS evidence only after these
checks have succeeded.

## Platform capability table

| Platform | Dedicated group and descendant heartbeat | Others |
| --- | --- | --- |
| Linux x64/ARM64 | Tested with group signal and live child heartbeat | No verified cgroup v2, hard RSS or escaped-group containment |
| macOS ARM64/Intel | Tested with group signal and live child heartbeat | No hard RSS, escaped-group containment or grandchild wait-reap |
| Windows | **UNSUPPORTED** for this Unix group mechanism | Job Objects require a separate Windows-specific implementation |

A process may start a separate session, new PGID or a descendant
that lives beyond this group; **this phase does not provide general
process-tree closure**, OS OOM classification, or ownership of
arbitrary user processes. On Windows the test **must print
UNSUPPORTED** rather than claim a false PASS. The existing Phase 0
cross-platform direct-child controls remain in
`worker_contract.rs` and are unchanged.

## Qualification

The independent `.github/workflows/process-group-contract.yml`
tests on Linux x64/ARM64, Windows, and macOS ARM64/Intel.
It checks formatting and Clippy, then runs the isolated conformance
example on each platform, checks exact PASS/UNSUPPORTED markers,
and retains separate OS evidence artifacts for 30 days.

Reproduction on a supported Unix host:

```sh
cargo fmt --manifest-path benchmarks/rust_core/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example process_group_contract -- -D warnings
cargo run --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example process_group_contract -- --self-test
```

## Further engineering work (not certified here)

- Windows Job Objects, process-tree hard kill on cancellation, nested
  jobs/permissions and failure injection.
- Escaped-process-group and adversarial descendant control on Unix.
- Linux cgroup v2 delegation and `memory.events` attribution;
  cgroup availability does not imply cgroup write permission.
- macOS memory evidence and a documented unsupported path where
  physical-memory enforcement cannot be guaranteed.
- Final solver-owned typed outcome contract, result provenance and
  total lifetime budget encompassing initialization and teardown.

No changes were made to `rawmocs-core`, exact Gaussian Full WMC,
cache admission, branching defaults, historical CodSpeed benchmark
identities or QuPRS integration.
