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

On Linux, this also runs an **independently oracled, exact Full Complex
WMC fixture in the actual stdin-gated child executable without a memory
cgroup cap**: clauses `[1, 2]`, `[-1, 3]`, three pairs of integer
Gaussian weights, and an independent eight-assignment enumeration
with expected result `24+3i`. The core solver must report SAT and
exactly the same Gaussian answer. The child emits a fixed proof token
which the parent captures and validates together with the child exit.
This verifies the solver work in the child binary, **not** its
enforcement under `memory.max`.

The test also verifies that a group owner performs best-effort cleanup
after its direct child has already been reaped, rather than assuming
no surviving descendant can exist when `child == None`.

On Linux, this additionally verifies independent parsing of local `max`,
`oom` and `oom_kill` counters; strictly monotonic deltas;
counter regression, missing, duplicate and negative values; a
fail-closed classification which **cannot infer OOM from worker exit
alone**; and rejection of `/sys/fs/cgroup` itself as an unsafe
delegation root. It prints:

- `DELEGATED_CGROUP_EVENT_NEGATIVE_CONTROLS=PASS`
- `DELEGATED_CGROUP_LOCAL_OOM_CLASSIFICATION=PASS`
- `DELEGATED_CGROUP_MOUNT_ROOT_REFUSAL=PASS`
- `DELEGATED_CGROUP_NO_KERNEL_WRITES=PASS`
- `DELEGATED_CGROUP_SOURCE_LOCKED_WMC_ORACLE=PASS`
- `DELEGATED_CGROUP_GATED_WMC_WORKER_UNCAPPED=PASS`
- `DELEGATED_CGROUP_REAPED_DESCENDANT_CLEANUP_CONTROL=PASS`
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
   For the positive control, the actual Worker must also emit the
   exact source-pinned `24+3i` WMC proof, exit successfully,
   and produce **no** new local `max`, `oom` or `oom_kill` events.
   Exit 137/SIGKILL, nonzero status, or a parent-wide counter
   change are *never* sufficient.
5. Explicitly waits/reaps the direct worker, checks that the
   child cgroup reports `populated 0`, and removes that temporary
   cgroup. Failure to verify either step fails the run. An RAII
   owner guard attempts `cgroup.kill` **even if the direct child
   was already reaped**, then direct-child kill/wait (if still
   owned), and directory removal on abnormal exits. The guard
   remains best effort; failure to confirm cleanup must be reported.

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
- The full-complex oracle runs inside the **cooperating, gated
  test worker** and on normal Linux CI without a cgroup cap.
  It is not an independent verification of WMC correctness for
  arbitrary user-supplied CNFs, and its run under a genuine
  `memory.max` limit remains **NOT_RUN**.
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


## Phase 2c: independently checked delegated-host evidence bundle

The opt-in command now emits source-checkable per-case kernel evidence
**only** after corresponding operations succeed. For each `under` and
`over` experiment it reports:

- Exactly read-back `memory.max=67108864` (64 MiB), and verified
  directly owned Worker PID membership in `cgroup.procs`.
- Raw private `memory.events.local` `max/oom/oom_kill` counters
  **before** and **after**, plus the computed delta.
- The direct worker's success/failure status and the positive
  `24+3i` Full Complex WMC oracle proof.
- The owned cgroup's `populated 0` and **successful removal**
  acknowledgement. These are distinct observations, not assumptions
  derived from direct-worker exit alone.

A separate stdlib-only Python verifier
(`benchmarks/rust_core/tools/delegated_host_evidence.py`) checks the
exact schema, uniqueness and consistency of every required marker,
zero counter increases for the positive case, strict positive local
`max`/`oom`/`oom_kill` increases plus unsuccessful exit for
the negative case, both cleanup witnesses, and the final real
enforcement verdict. A missing, duplicate, contradictory, regressing,
or `NOT_RUN` marker **fails** qualification. An accepted record
is a limited kernel-observation witness on a controlled private leaf,
not a universal OOM-causality proof.

The ordinary five-OS CI runs `--self-test` **only**, rejecting
synthetic counterexamples. It must print
`DELEGATED_EVIDENCE_REAL_ENFORCEMENT=NOT_RUN`.
Its passing status cannot be called a delegated-host OOM experiment.

### Privileged-host admission (manual and opt-in)

Run **only** on an already delegated, non-root Linux host with
a clean source checkout, a private non-root subtree below
`/sys/fs/cgroup` and the `memory` controller enabled in that
subtree's `cgroup.subtree_control`. Do **not** configure an
untrusted shared subtree, use `sudo`, modify a host-wide controller,
or ask the GitHub-hosted runner to fabricate delegation.

```sh
python3 benchmarks/rust_core/tools/delegated_host_evidence.py \
  --delegated-run /sys/fs/cgroup/YOUR_PRIVATE_DELEGATED_SUBTREE \
  --output /tmp/rawmocs-delegated-memory-evidence.json \
  --confirm
```

The wrapper requires the explicit `--confirm` opt-in, checks that
the Git tree is clean, records the exact Git commit and kernel
release, and runs the existing fixed 4 MiB / 192 MiB workloads
in their uniquely owned **64 MiB** capped child cgroups.
It captures both stdout and stderr and writes a new JSON bundle
outside the source checkout; it refuses to overwrite existing
evidence. For an external deadline failure it attempts to kill the
**whole cooperative Cargo/Worker process group** rather than just
the Cargo PID, but cgroup directory teardown remains **UNVERIFIED**
unless the underlying bounded Rust test completed its own cleanup
checks. An administrator must separately inspect any residual
private cgroup on an interrupted host; no automatic global cleanup
or privilege escalation is attempted. It emits
`DELEGATED_EVIDENCE_REAL_ENFORCEMENT=QUALIFIED_SCOPED` **only**
after both cases pass the independent evidence gate. Otherwise it
exits nonzero and records `NOT_VALIDATED`; if the subprocess never
started it records the blocker where possible.

The evidence file can include a private cgroup path, host kernel
details and captured stderr. Review/redact host-identifying paths
before publishing. No automatic upload to the public repository,
Actions artifact store or Linear is performed.

This source-only research contract still does **not** prove
full-lifetime memory isolation (the cooperative Worker starts before
its PID is assigned), that all descendants are contained, or that
`oom_kill` could never result from unrelated global memory
pressure. True production qualification requires corresponding
isolated-host admission and failure injection; macOS and Windows
have separate resource semantics.


## Phase 2d: mutation-free private delegation preflight

Before invoking any real `--delegated-run` kernel mutation, operators
may inspect a proposed **existing private** Linux cgroup v2 subtree:

```sh
python3 benchmarks/rust_core/tools/delegated_host_evidence.py \
  --preflight /sys/fs/cgroup/YOUR_PRIVATE_DELEGATED_SUBTREE
```

This operation makes **no kernel cgroup writes** and never runs Cargo
or Worker. It reports `DELEGATED_PREFLIGHT_STATUS=UNAVAILABLE` and a
specific reason for unsafe paths, root execution, missing cgroup2,
read-only mount, disabled memory controller, the current source
process **outside** the private subtree, unwritable directory, or
unwritable `cgroup.procs`. On non-Linux it reports
`UNSUPPORTED`. The preflight rejects `/sys/fs/cgroup` itself
and paths outside this mount without trying to create a cgroup.

The source-membership check is important: cgroup v2 delegation
checks permissions at the common ancestor of a process's source
and destination. This research Worker inherits the supervisor's
source cgroup before its cooperative gate releases. Requiring the
supervisor to be **already inside** the proposed delegated subtree
is a conservative fail-closed restriction; an operator cannot
satisfy it merely by pointing at an unrelated writable directory.
This check will exclude some configurations that might otherwise
be migratable with broader grants, intentionally avoiding accidental
cross-delegation writes.

If all observable prerequisites pass, the only possible preflight
status is `CANDIDATE_NOT_PROVEN`, **not** `READY`,
`MEMORY_LIMIT_SUPPORTED` or `QUALIFIED_SCOPED`.
Even correct apparent file permissions do not prove that a
unique sub-cgroup can be created, the Worker can be migrated,
`memory.max` can be written or OOM attribution is valid.
The actual opt-in test must still produce accepted private-leaf
memory events and verified cleanup. The real `--delegated-run`
path uses this same non-mutating preflight as an admission check.

All five standard GitHub CI platforms require the preflight's
independent negative controls and an actual mount-root refusal,
while continuing to report
`DELEGATED_EVIDENCE_REAL_ENFORCEMENT=NOT_RUN`.

Reference: Linux kernel
[cgroup v2 delegation and migration permissions](https://docs.kernel.org/admin-guide/cgroup-v2.html#delegation).


## Phase 2e: independent offline replay of archived evidence

The previously generated **local** JSON archive can now be rechecked
without a delegated Linux host, network connection, privileged process,
solver execution or cgroup mutation. Specify the *independently trusted*
exact commit that was qualified; do not simply copy an untrusted SHA
from inside the evidence file:

```sh
python3 benchmarks/rust_core/tools/verify_delegated_evidence.py \
  --verify /tmp/rawmocs-delegated-memory-evidence.json \
  --expected-sha <trusted-40-character-commit-sha>
```

The verifier rejects duplicated JSON keys (including nested ones),
unknown or missing schema fields, incorrect SHA and source identity,
nonzero exit, an archived failure, unsafe/ambiguous cgroup path,
missing time zone, absent scientific caveats, wrong outcome state,
and mismatches between the JSON `cases` and the **independently
recomputed** result from the original archived raw stdout.

It also reuses the source-controlled parser to enforce exactly-once
64 MiB limit/PID membership/positive complex-WMC proof, local
`max/oom/oom_kill` before/after/delta consistency, under/over
controls, and `populated 0` plus cgroup removal records. The file
size is bounded to 8 MiB. A passing check outputs
`DELEGATED_ARCHIVE_CONSISTENCY=PASS` and
`DELEGATED_ARCHIVE_KERNEL_OBSERVATION_REPRODUCED=NO`.

**An offline PASS is not cryptographic attestation, a signature,
an independent re-execution of the Linux kernel, or proof a reporter
did not fabricate both the archive and its stdout.** The operator
must obtain trusted provenance of the raw evidence, checkout SHA,
host identity/permission and protected collection path separately.
Synthetic positive fixtures only test the verifier. Normal GitHub CI
runs the verifier's negative controls and explicitly records
`DELEGATED_ARCHIVE_KERNEL_ENFORCEMENT=NOT_RUN`; it cannot confer
real delegated-host certification.

No source evidence file is automatically uploaded to GitHub; its
private cgroup path, kernel information and raw stderr should be
reviewed before sharing outside the trusted research environment.
