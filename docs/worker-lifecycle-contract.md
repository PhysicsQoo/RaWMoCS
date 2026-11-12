# Cross-platform solver worker lifecycle: Phase 0 evidence

This stage provides a **small isolated conformance probe**, not a
production process supervisor and not an increase in publicly supported
solver resource limits. The probe is
`benchmarks/rust_core/examples/worker_contract.rs`, and is tested by
`.github/workflows/worker-contract.yml` on Linux x64 and ARM64,
Windows and macOS ARM64 and Intel.

## Verified scope: one directly owned child

Using the standard Rust process library only, the parent spawns its
own executable in controlled child modes, observes terminal status,
polls a monotonic deadline and cancellation flag, and explicitly
kills **and waits for** a single still-running child on a deadline
or cancellation. An owner guard attempts cleanup if parent polling
or other operations fail. Pre-cancelled requests do not spawn a child.

The measured results must distinguish:

| Verdict | Evidence | Interpretation |
| --- | --- | --- |
| `Completed` | Child exit status succeeded | Only the child process completed; this alone never certifies a WMC result |
| `SolverError` | Predeclared controlled child exit 17 | Deliberate solver-error channel in the fixture; not general solver API error mapping |
| `OtherWorkerExit` | Unknown exit 29, or unverified resource-like exit 42 | Unknown, fail closed: never infer OOM, UNSAT, or a certified count |
| `TimedOut` | Still-running child killed and reaped after monotonic deadline | No model-count verdict |
| `Cancelled` | Still-running child killed and reaped after cancellation, or request pre-cancelled | No model-count verdict |

The test suite deliberately gives a child an exit code *42* that
resembles an allocation-denial result. That **does not prove** a memory
limit was enforced, so it must be classified as `OtherWorkerExit`.
This differs from the existing Linux `cache_profile.rs` controlled
`prlimit --as` negative test, in which the child itself explicitly
reports that a `try_reserve_exact` failed. That test qualifies only
virtual address-space allocation refusal, **not** resident memory
enforcement.

## Deliberate unsupported boundaries

Phase 0 supports only ownership of the **direct child**, not reliable
cleanup of its descendant process tree. The probe does not currently
set up:

- Linux cgroup v2 delegated controls, memory.events accounting or
  cgroup OOM detection.
- Windows Job Objects, nested job handling, process-tree termination
  and committed/private-byte memory enforcement.
- macOS hard memory caps or a native process-group reaper.
- General solver cancellation injection, resumable solver state,
  hard real-RSS limits or final output provenance.

These gaps must remain explicitly **UNSUPPORTED** in every OS
conformance result. A process that spawns grandchildren or manipulates
its own process group cannot rely on Phase 0 termination semantics.
The standard process `kill` targets the direct process only.
A worker exit code, including 42, must never be promoted into a
verified `MemoryLimit` without independent OS and process-specific
evidence.

## Reproduction and acceptance

```sh
cargo fmt --manifest-path benchmarks/rust_core/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example worker_contract -- -D warnings
cargo run --locked --manifest-path benchmarks/rust_core/Cargo.toml \
  --example worker_contract -- --self-test
```

The program emits seven conformance PASS lines, five explicitly
unsupported capabilities and a final
`WORKER_LIFECYCLE_DIRECT_CHILD_CONFORMANCE=PASS`. The CI workflow
asserts **every** line; no incomplete run counts as passed, and each
OS result is independently archived with a 30-day artifact lifetime.
The test is built and run on each target OS; passing Linux CI alone
does not establish Windows or macOS compliance.

The next phase needs platform-specific child groups/job objects with
orphan controls, explicit cancellation/timeout precedence and OS
memory-limit provenance. Keep the work behind a separate design and
review rather than adding an incomplete error enum to
`rawmocs-core`. The production Frequency Pivot, Legacy Cache,
exact Gaussian-rational WMC and QuPRS adapter defaults remain
unchanged.
