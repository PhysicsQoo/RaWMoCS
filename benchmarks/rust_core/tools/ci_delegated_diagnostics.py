#!/usr/bin/env python3
"""Emit only fixed, non-sensitive categories for delegated cgroup CI failures.

Never print paths, command lines, raw events, user-controlled text,
stderr, or the source JSON itself. The synthetic test is read-only.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path


# Priority is specific before general; values printed are fixed literals.
CATEGORIES = (
    ("worktree must be clean", "SOURCE_NOT_CLEAN"),
    ("source sha not available", "SOURCE_SHA_UNAVAILABLE"),
    ("preflight", "DELEGATION_PREFLIGHT_FAILED"),
    ("cannot create owned subgroup", "SUBGROUP_CREATE_FAILED"),
    ("memory.max write", "MEMORY_MAX_WRITE_FAILED"),
    ("memory.max readback", "MEMORY_MAX_READBACK_FAILED"),
    ("cgroup.procs admission", "WORKER_PID_MIGRATION_FAILED"),
    ("worker pid membership unverified", "WORKER_MEMBERSHIP_UNVERIFIED"),
    ("local memory events", "LOCAL_MEMORY_EVENTS_UNAVAILABLE"),
    ("worker stdout", "WORKER_PROOF_UNAVAILABLE"),
    ("full complex wmc mismatch", "WMC_ORACLE_MISMATCH"),
    ("insufficient evidence", "LOCAL_OOM_OR_ORACLE_UNVERIFIED"),
    ("cgroup remains populated", "CGROUP_STILL_POPULATED"),
    ("owned cgroup removal failed", "CGROUP_REMOVAL_FAILED"),
    ("worker experiment returned nonzero exit", "SCOPED_WORKER_FAILED"),
    ("external deadline expired", "EXTERNAL_DEADLINE_EXPIRED"),
    ("timed process-group cleanup failed", "PROCESS_GROUP_CLEANUP_UNVERIFIED"),
    ("unable to run locked rust experiment", "RUST_EXECUTION_UNAVAILABLE"),
)


def classify(evidence: dict | None, internal_log: str) -> tuple[str, str]:
    if evidence is not None and evidence.get("classification") == "SCOPED_DELEGATED_MEMORY_OBSERVED":
        return "COMPLETE", "QUALIFIED_SCOPED"
    # The record's stderr/stdout/failure are not trustworthy for publication:
    # reduce all observed strings to an explicitly enumerated category.
    parts = []
    if isinstance(evidence, dict):
        for field in ("failure", "stdout", "stderr"):
            value = evidence.get(field)
            if isinstance(value, str):
                parts.append(value)
    parts.append(internal_log)
    source = "\n".join(parts).lower()
    for marker, category in CATEGORIES:
        if marker in source:
            return ("RECORD_PRESENT" if evidence is not None else "NO_RECORD"), category
    if evidence is not None:
        return "RECORD_PRESENT", "RECORD_NOT_QUALIFIED_UNKNOWN_REASON"
    return "NO_RECORD", "NO_RECORD_UNKNOWN_REASON"


def report(record_path: Path, log_path: Path) -> None:
    evidence = None
    if record_path.is_file():
        try:
            obj = json.loads(record_path.read_text(encoding="utf-8"))
            if isinstance(obj, dict):
                evidence = obj
        except (OSError, UnicodeError, ValueError):
            pass
    try:
        log = log_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        log = ""
    stage, reason = classify(evidence, log)
    print(f"DELEGATED_CI_FAILURE_STAGE={stage}")
    print(f"DELEGATED_CI_FAILURE_CATEGORY={reason}")
    print("DELEGATED_CI_REAL_ENFORCEMENT=NOT_VALIDATED")


def self_test() -> None:
    cases = [
        (None, "DELEGATED_EVIDENCE_FAIL: worktree must be clean", "NO_RECORD", "SOURCE_NOT_CLEAN"),
        ({"classification": "NOT_VALIDATED", "failure": "real delegated evidence rejected",
          "stdout": "", "stderr": "DELEGATED_CGROUP_FAIL: RESOURCE_LIMIT_UNAVAILABLE: cgroup.procs admission: Permission denied"},
         "", "RECORD_PRESENT", "WORKER_PID_MIGRATION_FAILED"),
        ({"classification": "NOT_VALIDATED", "failure": "worker experiment returned nonzero exit",
          "stdout": "DELEGATED_CGROUP_FAIL: cgroup remains populated after direct worker completion",
          "stderr": ""}, "", "RECORD_PRESENT", "CGROUP_STILL_POPULATED"),
        (None, "unsafe private path: /home/runner/sensitive/log", "NO_RECORD", "NO_RECORD_UNKNOWN_REASON"),
        ({"classification": "NOT_VALIDATED", "failure": "external deadline expired"}, "",
         "RECORD_PRESENT", "EXTERNAL_DEADLINE_EXPIRED"),
    ]
    for obj, log, stage, code in cases:
        if classify(obj, log) != (stage, code):
            raise AssertionError("diagnostic category regression")
    print("DELEGATED_CI_DIAGNOSTIC_NEGATIVE_CONTROLS=PASS")
    print("DELEGATED_CI_DIAGNOSTICS_REAL_ENFORCEMENT=NOT_RUN")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--self-test", action="store_true")
    modes.add_argument("--report", type=Path)
    parser.add_argument("--internal-log", type=Path)
    args = parser.parse_args()
    if args.self_test:
        if args.internal_log is not None:
            parser.error("--internal-log cannot be passed to --self-test")
        self_test()
    else:
        if args.internal_log is None:
            parser.error("--report requires --internal-log")
        report(args.report, args.internal_log)


if __name__ == "__main__":
    main()
