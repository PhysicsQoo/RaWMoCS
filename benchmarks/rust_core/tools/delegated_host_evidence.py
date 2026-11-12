#!/usr/bin/env python3
"""Fail-closed, source-locked evidence gate for an explicitly delegated Linux host.

The default --self-test uses synthetic strings and never touches kernel cgroups.
Real execution requires --delegated-run, --output and --confirm on a clean,
non-root Linux checkout. This script does not grant cgroup delegation.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys


LIMIT_BYTES = 64 * 1024 * 1024
PROOF = "EXACT_COMPLEX_WMC=24+3i"
CASES = ("under", "over")
EVENTS = ("max", "oom", "oom_kill")
REPO = Path(__file__).resolve().parents[3]


class EvidenceError(ValueError):
    """A missing, contradictory or non-causal memory-limit witness."""


def exactly_one(lines: list[str], prefix: str) -> str:
    candidates = [line[len(prefix) :] for line in lines if line.startswith(prefix)]
    if len(candidates) != 1:
        raise EvidenceError(f"expected exactly one {prefix!r}, found {len(candidates)}")
    return candidates[0]


def events_value(text: str) -> dict[str, int]:
    fields = text.split(",")
    if len(fields) != 3:
        raise EvidenceError("local event record must contain exactly three counters")
    values: dict[str, int] = {}
    for key, item in zip(EVENTS, fields, strict=True):
        match = re.fullmatch(rf"{key}:([0-9]+)", item)
        if match is None:
            raise EvidenceError(f"invalid local event counter {key}")
        values[key] = int(match.group(1))
    return values


def qualify(stdout: str, return_code: int) -> dict[str, object]:
    """Independent syntax/consistency validation; not a kernel provenance oracle."""
    if return_code != 0:
        raise EvidenceError(f"worker experiment returned nonzero exit: {return_code}")
    lines = [line.strip() for line in stdout.splitlines()]
    if exactly_one(lines, "DELEGATED_CGROUP_REAL_ENFORCEMENT=") != "OBSERVED":
        raise EvidenceError("real kernel enforcement marker absent")
    if any(
        line == "DELEGATED_CGROUP_REAL_ENFORCEMENT=NOT_RUN"
        or line == "DELEGATED_CGROUP_REAL_ENFORCEMENT=UNSUPPORTED"
        for line in lines
    ):
        raise EvidenceError("self-test or unsupported evidence cannot be admitted")

    results: dict[str, object] = {}
    for mode in CASES:
        key = f"DELEGATED_{mode}_"
        if exactly_one(lines, key + "MEMORY_MAX_READBACK=") != str(LIMIT_BYTES):
            raise EvidenceError(f"{mode}: missing exact 64 MiB memory.max readback")
        for marker in (
            "WORKER_PID_MEMBERSHIP",
            "POPULATED_ZERO",
            "CGROUP_REMOVED",
            "WORKER_MEMBERSHIP_AND_CLEANUP",
        ):
            if exactly_one(lines, key + marker + "=") != "PASS":
                raise EvidenceError(f"{mode}: missing successful {marker}")
        before = events_value(exactly_one(lines, key + "EVENTS_BEFORE="))
        after = events_value(exactly_one(lines, key + "EVENTS_AFTER="))
        delta = events_value(exactly_one(lines, key + "COUNTERS="))
        observed_delta: dict[str, int] = {}
        for event in EVENTS:
            if after[event] < before[event]:
                raise EvidenceError(f"{mode}: {event} event counter decreased")
            observed_delta[event] = after[event] - before[event]
        if delta != observed_delta:
            raise EvidenceError(f"{mode}: inconsistent local event deltas")

        exit_type = exactly_one(lines, key + "DIRECT_EXIT=")
        if mode == "under":
            if exit_type != "SUCCESS" or any(observed_delta.values()):
                raise EvidenceError("under-limit positive control is not clean")
        else:
            if exit_type != "FAILURE" or any(value == 0 for value in observed_delta.values()):
                raise EvidenceError("over-limit failure is not supported by all local counters")
        results[mode] = {
            "limit_bytes": LIMIT_BYTES,
            "local_events_before": before,
            "local_events_after": after,
            "local_events_delta": observed_delta,
            "direct_exit": exit_type,
            "membership": "PASS",
            "populated_zero": True,
            "removed": True,
        }
    if exactly_one(lines, "DELEGATED_UNDER_WORKER_WMC_ORACLE=") != "PASS":
        raise EvidenceError("under-limit Full Complex WMC proof missing")
    return results


def synthetic_log() -> str:
    lines = []
    for mode, before, after, exit_type in (
        ("under", (0, 0, 0), (0, 0, 0), "SUCCESS"),
        ("over", (10, 20, 30), (12, 21, 31), "FAILURE"),
    ):
        prefix = f"DELEGATED_{mode}_"
        lines.append(prefix + f"MEMORY_MAX_READBACK={LIMIT_BYTES}")
        lines.append(prefix + "WORKER_PID_MEMBERSHIP=PASS")
        for name, counts in (
            ("EVENTS_BEFORE", before),
            ("EVENTS_AFTER", after),
            ("COUNTERS", tuple(a - b for a, b in zip(after, before, strict=True))),
        ):
            lines.append(prefix + name + "=" + ",".join(f"{k}:{v}" for k, v in zip(EVENTS, counts, strict=True)))
        lines.append(prefix + f"DIRECT_EXIT={exit_type}")
        lines.append(prefix + "POPULATED_ZERO=PASS")
        lines.append(prefix + "CGROUP_REMOVED=PASS")
        lines.append(prefix + "WORKER_MEMBERSHIP_AND_CLEANUP=PASS")
    lines.extend((
        "DELEGATED_UNDER_WORKER_WMC_ORACLE=PASS",
        "DELEGATED_CGROUP_REAL_ENFORCEMENT=OBSERVED",
    ))
    return "\n".join(lines) + "\n"


def self_test() -> None:
    valid = synthetic_log()
    result = qualify(valid, 0)
    assert result["under"]["local_events_delta"] == dict.fromkeys(EVENTS, 0)
    assert result["over"]["local_events_delta"] == {"max": 2, "oom": 1, "oom_kill": 1}
    def mutate_exact(old: str, new: str) -> str:
        if valid.count(old) != 1:
            raise EvidenceError(f"negative fixture mutation anchor missing or ambiguous: {old}")
        return valid.replace(old, new)

    rejected = [
        (valid, 1),
        (mutate_exact("REAL_ENFORCEMENT=OBSERVED", "REAL_ENFORCEMENT=NOT_RUN"), 0),
        (mutate_exact("DELEGATED_over_DIRECT_EXIT=FAILURE", "DELEGATED_over_DIRECT_EXIT=SUCCESS"), 0),
        (mutate_exact("DELEGATED_over_COUNTERS=max:2,oom:1,oom_kill:1",
                       "DELEGATED_over_COUNTERS=max:2,oom:1,oom_kill:0"), 0),
        (mutate_exact("DELEGATED_under_COUNTERS=max:0,oom:0,oom_kill:0",
                       "DELEGATED_under_COUNTERS=max:1,oom:0,oom_kill:0"), 0),
        (mutate_exact("DELEGATED_under_WORKER_PID_MEMBERSHIP=PASS", ""), 0),
        (mutate_exact("DELEGATED_over_CGROUP_REMOVED=PASS", ""), 0),
        (mutate_exact("DELEGATED_UNDER_WORKER_WMC_ORACLE=PASS", ""), 0),
        (valid + "DELEGATED_CGROUP_REAL_ENFORCEMENT=OBSERVED\n", 0),
        (mutate_exact("DELEGATED_over_MEMORY_MAX_READBACK=67108864",
                       "DELEGATED_over_MEMORY_MAX_READBACK=33554432"), 0),
        (mutate_exact("DELEGATED_over_EVENTS_AFTER=max:12,oom:21,oom_kill:31",
                       "DELEGATED_over_EVENTS_AFTER=max:12,oom:21,oom_kill:29"), 0),
    ]
    for index, (record, code) in enumerate(rejected):
        try:
            qualify(record, code)
        except EvidenceError:
            continue
        raise EvidenceError(f"negative fixture {index} was incorrectly admitted")
    preflight_self_test()
    print("DELEGATED_EVIDENCE_SYNTHETIC_NEGATIVE_CONTROLS=PASS")
    print("DELEGATED_EVIDENCE_REAL_ENFORCEMENT=NOT_RUN")



def preflight_policy(
    *, private_subtree: bool, non_root: bool, cgroup_v2: bool,
    mount_writable: bool, memory_enabled: bool, source_inside: bool,
    directory_writable: bool, ancestor_procs_writable: bool,
) -> tuple[str, str]:
    """Advisory checks only. Never claims that kernel PID migration succeeded."""
    controls = (
        ("UNSAFE_MOUNT_ROOT_OR_PATH", private_subtree),
        ("ROOT_EXECUTION_FORBIDDEN", non_root),
        ("CGROUP_V2_NOT_OBSERVED", cgroup_v2),
        ("CGROUP_MOUNT_READ_ONLY", mount_writable),
        ("MEMORY_CONTROLLER_NOT_DELEGATED", memory_enabled),
        ("WORKER_SOURCE_OUTSIDE_PRIVATE_SUBTREE", source_inside),
        ("SUBTREE_DIRECTORY_NOT_WRITABLE", directory_writable),
        ("COMMON_ANCESTOR_PROCS_NOT_WRITABLE", ancestor_procs_writable),
    )
    for reason, allowed in controls:
        if not allowed:
            return "UNAVAILABLE", reason
    # File permissions are only prerequisites; no cgroup directory,
    # process migration, or memory.max write occurs in this preflight.
    return "CANDIDATE_NOT_PROVEN", "MUST_PERFORM_REAL_SCOPED_RUN"


def preflight(subtree: Path) -> tuple[str, str]:
    """Read-only on the host, including when the mount is writable."""
    if platform.system() != "Linux":
        return "UNSUPPORTED", "NON_LINUX"
    mount = Path("/sys/fs/cgroup")
    try:
        canonical_mount = mount.resolve(strict=True)
        canonical_subtree = subtree.resolve(strict=True)
    except OSError:
        return "UNAVAILABLE", "CGROUP_PATH_NOT_ACCESSIBLE"
    if canonical_subtree == canonical_mount or not canonical_subtree.is_relative_to(canonical_mount):
        return "UNAVAILABLE", "UNSAFE_MOUNT_ROOT_OR_PATH"
    if os.geteuid() == 0:
        return "UNAVAILABLE", "ROOT_EXECUTION_FORBIDDEN"

    try:
        stat = os.statvfs(canonical_subtree)
        if stat.f_flag & os.ST_RDONLY:
            return "UNAVAILABLE", "CGROUP_MOUNT_READ_ONLY"
        mountinfo = Path("/proc/self/mountinfo").read_text(encoding="utf-8")
        is_cgroup_v2 = any(
            before.split()[4] == str(mount)
            and after.split()[0] == "cgroup2"
            for line in mountinfo.splitlines()
            if (parts := line.split(" - ", 1)) and len(parts) == 2
            for before, after in [parts]
            if len(before.split()) >= 5 and after.split()
        )
        if not is_cgroup_v2:
            return "UNAVAILABLE", "CGROUP_V2_NOT_OBSERVED"
        enabled = "memory" in (
            canonical_subtree / "cgroup.subtree_control"
        ).read_text(encoding="utf-8").split()
        if not enabled:
            return "UNAVAILABLE", "MEMORY_CONTROLLER_NOT_DELEGATED"

        # Writing a PID to a new child cgroup requires access to the
        # common ancestor of the *source* and *destination* cgroups.
        # A worker spawned by this supervisor inherits its current cgroup.
        membership = Path("/proc/self/cgroup").read_text(encoding="utf-8").splitlines()
        unified = [line[3:] for line in membership if line.startswith("0::/")]
        if len(unified) != 1:
            return "UNAVAILABLE", "SOURCE_CGROUP_UNKNOWN"
        # Treat cgroup namespace paths as relative to the observed mount;
        # ambiguous namespace mappings fail closed before any kernel write.
        source = (canonical_mount / unified[0].lstrip("/")).resolve(strict=True)
        if not source.is_relative_to(canonical_subtree):
            return "UNAVAILABLE", "WORKER_SOURCE_OUTSIDE_PRIVATE_SUBTREE"
        if not os.access(canonical_subtree, os.W_OK | os.X_OK):
            return "UNAVAILABLE", "SUBTREE_DIRECTORY_NOT_WRITABLE"
        if not os.access(canonical_subtree / "cgroup.procs", os.W_OK):
            return "UNAVAILABLE", "COMMON_ANCESTOR_PROCS_NOT_WRITABLE"
    except (OSError, ValueError, IndexError):
        return "UNAVAILABLE", "CGROUP_PREFLIGHT_UNVERIFIABLE"
    return "CANDIDATE_NOT_PROVEN", "MUST_PERFORM_REAL_SCOPED_RUN"


def preflight_self_test() -> None:
    keys = (
        "private_subtree", "non_root", "cgroup_v2", "mount_writable",
        "memory_enabled", "source_inside", "directory_writable",
        "ancestor_procs_writable",
    )
    accepted = dict.fromkeys(keys, True)
    if preflight_policy(**accepted) != (
        "CANDIDATE_NOT_PROVEN", "MUST_PERFORM_REAL_SCOPED_RUN"
    ):
        raise EvidenceError("all prerequisites incorrectly labeled as enforced or unavailable")
    for key in keys:
        controls = accepted.copy()
        controls[key] = False
        status, reason = preflight_policy(**controls)
        if status != "UNAVAILABLE" or not reason:
            raise EvidenceError(f"preflight negative control accepted: {key}")
    if platform.system() == "Linux":
        status, reason = preflight(Path("/sys/fs/cgroup"))
        if (status, reason) != ("UNAVAILABLE", "UNSAFE_MOUNT_ROOT_OR_PATH"):
            raise EvidenceError("real mount root was not refused before any kernel mutation")
    else:
        if preflight(Path("/sys/fs/cgroup")) != ("UNSUPPORTED", "NON_LINUX"):
            raise EvidenceError("non-Linux preflight incorrectly supported cgroup v2")
    print("DELEGATED_PREFLIGHT_NEGATIVE_CONTROLS=PASS")
    print("DELEGATED_PREFLIGHT_KERNEL_WRITES=NONE")


def git(*args: str) -> str:
    return subprocess.check_output(["git", "-C", str(REPO), *args], text=True).strip()


def delegated_run(subtree: Path, output: Path, confirm: bool) -> None:
    if not confirm:
        raise EvidenceError("explicit --confirm required before real kernel cgroup writes")
    status, reason = preflight(subtree)
    if status != "CANDIDATE_NOT_PROVEN":
        raise EvidenceError(f"RESOURCE_LIMIT_UNAVAILABLE: preflight {status}: {reason}")
    subtree = subtree.resolve(strict=True)

    sha = git("rev-parse", "HEAD")
    if re.fullmatch(r"[0-9a-f]{40}", sha) is None:
        raise EvidenceError("source SHA not available")
    if git("status", "--porcelain"):
        raise EvidenceError("worktree must be clean for source-pinned qualification")
    output = output.resolve()
    if output.is_relative_to(REPO):
        raise EvidenceError("evidence must be written outside the source checkout")
    if output.exists():
        raise EvidenceError("refuse to overwrite pre-existing evidence")
    output.parent.mkdir(parents=True, exist_ok=True)

    command = [
        "cargo", "run", "--locked", "--manifest-path",
        str(REPO / "benchmarks/rust_core/Cargo.toml"),
        "--example", "delegated_cgroup_contract", "--", "--delegated-run", str(subtree),
    ]
    completed: subprocess.CompletedProcess[str] | None = None
    failure: str | None = None
    qualified: dict[str, object] | None = None
    try:
        # Own a distinct Unix session: if Cargo or its worker hangs, kill the
        # entire cooperative process group, not only Cargo's direct PID.
        proc = subprocess.Popen(
            command,
            cwd=REPO,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        )
        try:
            out, err = proc.communicate(timeout=180)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            out, err = proc.communicate(timeout=10)
            failure = (
                "external deadline expired; process-group kill attempted; "
                "cgroup directory cleanup remains UNVERIFIED"
            )
        completed = subprocess.CompletedProcess(command, proc.returncode, out, err)
        if failure is None:
            try:
                qualified = qualify(completed.stdout, completed.returncode)
            except EvidenceError as exc:
                failure = str(exc)
    except (OSError, subprocess.TimeoutExpired) as exc:
        failure = f"delegated experiment or timed process-group cleanup failed: {exc}"

    record = {
        "schema_version": 1,
        "source_commit_sha": sha,
        "kernel_release": platform.release(),
        "private_delegated_subtree": str(subtree),
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "classification": "SCOPED_DELEGATED_MEMORY_OBSERVED" if qualified else "NOT_VALIDATED",
        "cases": qualified,
        "failure": failure,
        "exit_code": completed.returncode if completed else None,
        "stdout": completed.stdout if completed else None,
        "stderr": completed.stderr if completed else None,
        "limitations": [
            "cooperative gate: startup allocations before PID migration not strictly capped",
            "local oom_kill includes any OOM killer and is not globally causal",
            "no adversarial child escape or production supervisor API",
        ],
    }
    # Exclusive creation avoids silently replacing or forging earlier evidence.
    with output.open("x", encoding="utf-8") as target:
        json.dump(record, target, indent=2, sort_keys=True)
        target.write("\n")
    if not qualified:
        raise EvidenceError(f"real delegated evidence rejected; inspect {output}: {failure}")
    print(f"DELEGATED_EVIDENCE_SOURCE_SHA={sha}")
    print("DELEGATED_EVIDENCE_REAL_ENFORCEMENT=QUALIFIED_SCOPED")
    print(f"DELEGATED_EVIDENCE_PATH={output}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    action = ap.add_mutually_exclusive_group(required=True)
    action.add_argument("--self-test", action="store_true")
    action.add_argument("--delegated-run", type=Path, metavar="PRIVATE_SUBTREE")
    action.add_argument("--preflight", type=Path, metavar="PRIVATE_SUBTREE")
    ap.add_argument("--output", type=Path, help="new JSON evidence path outside source checkout")
    ap.add_argument("--confirm", action="store_true", help="authorize this specific kernel-level experiment")
    args = ap.parse_args()
    try:
        if args.self_test:
            if args.output is not None or args.confirm:
                raise EvidenceError("--self-test cannot accept real-run options")
            self_test()
        elif args.preflight is not None:
            if args.output is not None or args.confirm:
                raise EvidenceError("--preflight is read-only and cannot accept run options")
            status, reason = preflight(args.preflight)
            print(f"DELEGATED_PREFLIGHT_STATUS={status}")
            print(f"DELEGATED_PREFLIGHT_REASON={reason}")
            print("DELEGATED_PREFLIGHT_KERNEL_WRITES=NONE")
        else:
            if args.output is None:
                raise EvidenceError("--output is mandatory for opt-in execution")
            delegated_run(args.delegated_run, args.output, args.confirm)
        return 0
    except (EvidenceError, OSError, subprocess.CalledProcessError) as exc:
        print(f"DELEGATED_EVIDENCE_FAIL: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
