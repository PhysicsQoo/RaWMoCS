#!/usr/bin/env python3
"""Same-runner source-identical exact Rust solver A/B on original QuPRS CNFs.

The native GPMC MPFR30 reference and original QuPRS encoder are qualified by
the companion representative report. This compares unchanged prior Rust exact
solver against a candidate and never infers improvements from separate CI runs.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
import os
from pathlib import Path
import statistics
import subprocess
from typing import Any

from quprs_representative_native import timed_execution

BASELINE_COMMIT = "45d71e9f5e302a53732b6f678898404c9c8a20e9"
CASES = (
    "ghz2", "ghz3", "ghz4", "graph3", "bv3", "qft2",
    "qft3", "grover2", "qaoa3", "feynman_mod5_4", "feynman_qft_4",
)
ACCEPTED = {"ORACLE_VERIFIED_PAIRED", "SOLVER_AGREEMENT_ONLY"}


def sha_file(path: Path) -> str:
    return sha256(path.read_bytes()).hexdigest()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--baseline", type=Path, required=True)
    ap.add_argument("--candidate", type=Path, required=True)
    ap.add_argument("--canonical-candidate", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--representative-report", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=3)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if not 2 <= args.trials <= 5:
        raise SystemExit("A/B requires two to four alternating independent trials")
    for binary in (args.baseline, args.candidate, args.canonical_candidate):
        if not binary.is_file():
            raise SystemExit(f"native executable missing: {binary}")
    source = json.loads(args.representative_report.read_text(encoding="utf8"))
    if source["source_commit"] != "0b28736651b3740ed3b201edd2efdef3da541e0d":
        raise SystemExit("unverified QuPRS exporter source")
    original_rows = {r["case"]: r for r in source["rows"]}
    base_root = args.baseline.resolve().parents[5]
    baseline_sha = subprocess.check_output(
        ["git", "-C", str(base_root), "rev-parse", "HEAD"], text=True
    ).strip()
    if baseline_sha != BASELINE_COMMIT:
        raise SystemExit(f"unexpected baseline commit: {baseline_sha}")
    report: dict[str, Any] = {
        "source_quprs_sha": source["source_commit"],
        "baseline_rawmocs_sha": baseline_sha,
        "candidate_checkout_sha": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True
        ).strip(),
        "candidate_source_head_sha": os.environ.get("RAWMOCS_HEAD_SHA") or None,
        "precision_contract": "exact finite printed decimal rational on both arms",
        "scope": "same input bytes, alternating native processes, 8s/2GiB bounds",
        "gpmc_contract": "GPMC MPFR30 is compared separately in original representative report",
        "rows": [],
    }
    source_head_sha = report["candidate_source_head_sha"]
    if source_head_sha:
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", source_head_sha,
             report["candidate_checkout_sha"]],
            check=True, capture_output=True,
        )
    invalid = False
    for case in CASES:
        input_file = args.corpus / f"{case}-gpmc.cnf"
        source_row = original_rows[case]
        if source_row.get("status") not in ACCEPTED:
            raise SystemExit(f"unqualified original paired input: {case}")
        cnf_sha = sha_file(input_file)
        if cnf_sha != source_row["cnf_sha256"]:
            raise SystemExit(f"source-pinned CNF bytes changed: {case}")
        row: dict[str, Any] = {
            "case": case, "cnf_sha256": cnf_sha,
            "variables": source_row["variables"], "clauses": source_row["clauses"],
            "samples": [],
        }
        arms = ("baseline", "candidate", "canonical")
        for trial in range(args.trials):
            order = arms[trial % 3:] + arms[:trial % 3]
            for arm in order:
                binary = args.baseline if arm == "baseline" else args.candidate
                if arm == "canonical":
                    binary = args.canonical_candidate
                command = [str(binary.resolve())]
                if arm == "canonical":
                    command.append("--canonical")
                command.append(str(input_file.resolve()))
                outcome = timed_execution(command, "rawmocs")
                outcome["arm"] = arm
                outcome["trial"] = trial
                row["samples"].append(outcome)
        all_completed = all(s["status"] == "COMPLETED" for s in row["samples"])
        if not all_completed:
            row["status"] = "CENSORED_NATIVE_FAILURE"
        else:
            exact_answers = {
                (s["sat"], Fraction(s["real"]), Fraction(s["imag"]))
                for s in row["samples"]
            }
            stdout_hashes = {s["native_stdout_sha256"] for s in row["samples"]}
            candidate_profiles = [
                s.get("rawmocs_phase_ns")
                for s in row["samples"] if s["arm"] == "candidate"
            ]
            canonical_profiles = [
                s.get("rawmocs_phase_ns")
                for s in row["samples"] if s["arm"] == "canonical"
            ]
            if len(exact_answers) != 1 or len(stdout_hashes) != 1:
                row["status"] = "EXACT_RESULT_OR_PROTOCOL_MISMATCH"
            elif any(profile is None for profile in candidate_profiles + canonical_profiles):
                row["status"] = "MISSING_CANDIDATE_PHASE_PROFILE"
            else:
                row["status"] = "EXACT_MATCH"
                row["wall_ns_median"] = {
                    arm: statistics.median(s["wall_ns"] for s in row["samples"] if s["arm"] == arm)
                    for arm in arms
                }
                row["peak_rss_kib_median"] = {
                    arm: statistics.median(s["peak_rss_kib"] for s in row["samples"] if s["arm"] == arm)
                    for arm in arms
                }
                row["candidate_phase_ns_median"] = {
                    phase: statistics.median(p[phase] for p in candidate_profiles)
                    for phase in ("read", "parse", "solve", "output")
                }
                row["canonical_phase_ns_median"] = {
                    phase: statistics.median(p[phase] for p in canonical_profiles)
                    for phase in ("read", "parse", "solve", "output")
                }
                base = row["wall_ns_median"]["baseline"]
                candidate = row["wall_ns_median"]["candidate"]
                canonical = row["wall_ns_median"]["canonical"]
                row["speed_ratio_baseline_over_candidate"] = base / candidate
                row["speed_ratio_baseline_over_canonical"] = base / canonical
                print(
                    f"RAWMOCS_AB case={case} status=EXACT_MATCH "
                    f"baseline_ms={base / 1e6:.3f} candidate_ms={candidate / 1e6:.3f} "
                    f"canonical_ms={canonical / 1e6:.3f} "
                    f"canonical_speedup={base / canonical:.3f} "
                    f"rss_baseline_kib={row['peak_rss_kib_median']['baseline']} "
                    f"rss_candidate_kib={row['peak_rss_kib_median']['candidate']} "
                    f"rss_canonical_kib={row['peak_rss_kib_median']['canonical']}"
                )
        if row["status"] != "EXACT_MATCH":
            invalid = True
            print(f"RAWMOCS_AB_CENSORED case={case} status={row['status']}")
        report["rows"].append(row)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    if invalid:
        raise SystemExit("exact A/B admission failed: inspect censored cases before claiming speedup")


if __name__ == "__main__":
    main()
