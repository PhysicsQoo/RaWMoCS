#!/usr/bin/env python3
"""Offline replay of a RaWMoCS delegated-cgroup research evidence bundle.

No kernel writes, process spawning, privileged operations, network access,
or claims of cryptographic origin. This verifies internal consistency
against a separately supplied, trusted source commit SHA.
"""

from __future__ import annotations

import argparse
import copy
import datetime as dt
import json
from pathlib import Path
import re
import sys

from delegated_host_evidence import EvidenceError, qualify, synthetic_log


TOP_LEVEL = frozenset(
    {
        "schema_version",
        "source_commit_sha",
        "kernel_release",
        "private_delegated_subtree",
        "generated_at_utc",
        "classification",
        "cases",
        "failure",
        "exit_code",
        "stdout",
        "stderr",
        "limitations",
    }
)
MAX_ARCHIVE_BYTES = 8 * 1024 * 1024


def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    """Reject duplicate JSON keys at *all* nesting depths."""
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceError(f"duplicate archived JSON key: {key}")
        result[key] = value
    return result


def parse_archive(source: str) -> dict[str, object]:
    try:
        data = json.loads(source, object_pairs_hook=unique_object)
    except (ValueError, TypeError) as exc:
        raise EvidenceError(f"invalid JSON archive: {exc}") from exc
    if not isinstance(data, dict):
        raise EvidenceError("archive must be one JSON object")
    return data


def verify_record(record: dict[str, object], trusted_sha: str) -> dict[str, object]:
    if type(trusted_sha) is not str or re.fullmatch(r"[0-9a-f]{40}", trusted_sha) is None:
        raise EvidenceError("explicit trusted SHA must contain exactly 40 lowercase hex digits")
    if set(record) != TOP_LEVEL:
        missing = sorted(TOP_LEVEL - set(record))
        unknown = sorted(set(record) - TOP_LEVEL)
        raise EvidenceError(f"archive schema mismatch: missing={missing}, unknown={unknown}")
    if type(record["schema_version"]) is not int or record["schema_version"] != 1:
        raise EvidenceError("unsupported archive schema version")
    if record["source_commit_sha"] != trusted_sha:
        raise EvidenceError("archive source SHA does not match externally pinned commit")
    if record["classification"] != "SCOPED_DELEGATED_MEMORY_OBSERVED":
        raise EvidenceError("only successful scoped evidence is admissible")
    if record["failure"] is not None:
        raise EvidenceError("archive contains a recorded failure")
    if type(record["exit_code"]) is not int or record["exit_code"] != 0:
        raise EvidenceError("real delegated test did not finish with exit code zero")
    if not isinstance(record["kernel_release"], str) or not record["kernel_release"].strip():
        raise EvidenceError("kernel release missing")
    path = record["private_delegated_subtree"]
    if (
        not isinstance(path, str)
        or not path.startswith("/sys/fs/cgroup/")
        or any(part in ("", ".", "..") for part in path[15:].split("/"))
    ):
        raise EvidenceError("archive does not name an unambiguous private cgroup path")
    if not isinstance(record["generated_at_utc"], str):
        raise EvidenceError("archive generation time missing")
    try:
        generated = dt.datetime.fromisoformat(record["generated_at_utc"])
    except ValueError as exc:
        raise EvidenceError("archive UTC timestamp malformed") from exc
    if generated.tzinfo is None or generated.utcoffset() != dt.timedelta(0):
        raise EvidenceError("archive timestamp is not UTC")
    limitations = record["limitations"]
    if (
        not isinstance(limitations, list)
        or not limitations
        or any(not isinstance(s, str) or not s.strip() for s in limitations)
    ):
        raise EvidenceError("scientific limitations missing")
    if not isinstance(record["stderr"], str):
        raise EvidenceError("raw stderr missing")
    stdout = record["stdout"]
    if not isinstance(stdout, str):
        raise EvidenceError("raw stdout missing")
    independently_derived = qualify(stdout, record["exit_code"])
    archive_cases = record["cases"]
    if not isinstance(archive_cases, dict) or set(archive_cases) != {"under", "over"}:
        raise EvidenceError("archived cases missing or contain unexpected entries")
    # Compare canonical JSON, not Python's loose equality (True == 1).
    canonical = lambda value: json.dumps(value, sort_keys=True, separators=(",", ":"))
    if canonical(archive_cases) != canonical(independently_derived):
        raise EvidenceError("stored cases differ from independently replayed raw evidence")
    return {
        "source_commit_sha": trusted_sha,
        "archive_consistency": "PASS",
        "kernel_observation_reproduced": False,
        "scope": "INTERNALLY_CONSISTENT_UNATTESTED",
    }


def verify_bytes(source: str, trusted_sha: str) -> dict[str, object]:
    if len(source.encode("utf-8")) > MAX_ARCHIVE_BYTES:
        raise EvidenceError("archive exceeds 8 MiB read-only replay limit")
    return verify_record(parse_archive(source), trusted_sha)


def synthetic_archive(sha: str) -> dict[str, object]:
    stdout = synthetic_log()
    return {
        "schema_version": 1,
        "source_commit_sha": sha,
        "kernel_release": "test-kernel-not-observed",
        "private_delegated_subtree": "/sys/fs/cgroup/test-only-not-real",
        "generated_at_utc": "2026-11-12T12:00:00+00:00",
        "classification": "SCOPED_DELEGATED_MEMORY_OBSERVED",
        "cases": qualify(stdout, 0),
        "failure": None,
        "exit_code": 0,
        "stdout": stdout,
        "stderr": "",
        "limitations": ["synthetic fixture; does not demonstrate actual kernel enforcement"],
    }


def self_test() -> None:
    sha = "0123456789abcdef" * 2 + "01234567"
    baseline = synthetic_archive(sha)
    assert verify_bytes(json.dumps(baseline), sha)["archive_consistency"] == "PASS"

    def mutated(field: str, value: object) -> dict[str, object]:
        data = copy.deepcopy(baseline)
        if field not in data:
            raise EvidenceError(f"unknown mutation target {field}")
        data[field] = value
        return data

    broken_case = copy.deepcopy(baseline["cases"])
    broken_case["over"]["local_events_delta"]["oom_kill"] = 0
    broken_cleanup = copy.deepcopy(baseline["cases"])
    broken_cleanup["under"]["removed"] = False
    broken_stdout = baseline["stdout"].replace(
        "DELEGATED_under_WORKER_PID_MEMBERSHIP=PASS",
        "DELEGATED_under_WORKER_PID_MEMBERSHIP=FAIL",
    )
    if broken_stdout == baseline["stdout"]:
        raise EvidenceError("stdout negative control was a no-op")
    rejects = [
        (baseline, "f" * 40),
        (mutated("source_commit_sha", "f" * 40), sha),
        (mutated("schema_version", True), sha),
        (mutated("classification", "NOT_VALIDATED"), sha),
        (mutated("failure", "timeout"), sha),
        (mutated("exit_code", 1), sha),
        (mutated("stdout", broken_stdout), sha),
        (mutated("stderr", 12), sha),
        (mutated("kernel_release", ""), sha),
        (mutated("private_delegated_subtree", "/sys/fs/cgroup"), sha),
        (mutated("private_delegated_subtree", "/sys/fs/cgroup/x/../y"), sha),
        (mutated("generated_at_utc", "2026-11-12T12:00:00"), sha),
        (mutated("limitations", []), sha),
        (mutated("cases", broken_case), sha),
        (mutated("cases", broken_cleanup), sha),
        ({k: v for k, v in baseline.items() if k != "cases"}, sha),
        ({**baseline, "unrecognized": True}, sha),
    ]
    for i, (record, pin) in enumerate(rejects):
        try:
            verify_bytes(json.dumps(record), pin)
        except EvidenceError:
            continue
        raise EvidenceError(f"archived negative control {i} incorrectly accepted")
    raw = json.dumps(baseline)
    duplicate_root = '{"schema_version":1,' + raw[1:]
    for name, source in (
        ("duplicate top-level JSON key", duplicate_root),
        ("non-object archive", "[]"),
        ("malformed JSON", '{"'),
        ("oversized archive", raw + " " * (MAX_ARCHIVE_BYTES + 1)),
    ):
        try:
            verify_bytes(source, sha)
        except EvidenceError:
            continue
        raise EvidenceError(f"{name} incorrectly accepted")
    print("DELEGATED_ARCHIVE_REPLAY_NEGATIVE_CONTROLS=PASS")
    print("DELEGATED_ARCHIVE_KERNEL_ENFORCEMENT=NOT_RUN")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--self-test", action="store_true")
    group.add_argument("--verify", type=Path, metavar="ARCHIVE_JSON")
    parser.add_argument("--expected-sha", help="independently trusted 40-character commit SHA")
    args = parser.parse_args()
    try:
        if args.self_test:
            if args.expected_sha is not None:
                raise EvidenceError("--expected-sha belongs only with --verify")
            self_test()
        else:
            if args.expected_sha is None:
                raise EvidenceError("--verify requires a separately trusted --expected-sha")
            if args.verify.stat().st_size > MAX_ARCHIVE_BYTES:
                raise EvidenceError("archive too large for offline replay")
            result = verify_bytes(args.verify.read_text(encoding="utf-8"), args.expected_sha)
            print(f"DELEGATED_ARCHIVE_SOURCE_SHA={result['source_commit_sha']}")
            print("DELEGATED_ARCHIVE_CONSISTENCY=PASS")
            print("DELEGATED_ARCHIVE_KERNEL_OBSERVATION_REPRODUCED=NO")
        return 0
    except (OSError, UnicodeError, EvidenceError) as exc:
        print(f"DELEGATED_ARCHIVE_REPLAY_FAIL: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
