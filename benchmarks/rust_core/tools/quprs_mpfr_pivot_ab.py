#!/usr/bin/env python3
"""Original QuPRS emitted CNF, matched-MPFR 3-pivot long-case ablation.

Changed pivot order can change numerical accumulation. Therefore exact
source-decimal rational reference + SAT correctness are necessary; unlike
data-structure-only experiments, byte-equal MPFR output and identical
search nodes are NOT imposed on alternative pivot order.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import statistics
from typing import Any

from quprs_representative_native import timed_execution

CASES = ("feynman_qft_4", "feynman_mod5_4", "qaoa3", "grover2", "qft3")
PRECISIONS = {30: 100, 100: 333}
ARMS = ("frequency", "short_clause", "moms")
FLAGS = {
    "frequency": (),
    "short_clause": ("--pivot-short",),
    "moms": ("--pivot-moms",),
}
SOURCE_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"


def admissible(value: Fraction, exact: Fraction, digits: int) -> bool:
    # A predefined absolute/relative agreement guard, not formal interval
    # certification of floating-point summation or quantum source phases.
    guard = Fraction(1, 10 ** (digits - 6))
    return abs(value - exact) <= guard * max(Fraction(1), abs(exact))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--matched-reference", type=Path, required=True)
    ap.add_argument("--exact-reference", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--mpfr", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=5)
    ap.add_argument("--report", type=Path, required=True)
    args = ap.parse_args()
    if not 3 <= args.trials <= 7:
        raise SystemExit("three to seven fresh-process trials are required")
    original = json.loads(args.exact_reference.read_text())
    matched = json.loads(args.matched_reference.read_text())
    if original["source_commit"] != SOURCE_SHA or matched["source_sha"] != SOURCE_SHA:
        raise SystemExit("not the immutable original QuPRS exporter")
    orig_rows = {r["case"]: r for r in original["rows"]}
    certified = {(r["case"], r["digits"]): r for r in matched["rows"]}
    report: dict[str, Any] = {
        "study": "source-locked real QuPRS Feynman short clause / MOMS MPFR variable ordering",
        "source_commit": SOURCE_SHA,
        "digits_to_bits": PRECISIONS,
        "fixed_fast_assign": True,
        "fixed_borrowed_cache": True,
        "algorithms": list(ARMS),
        "trials_per_arm": args.trials,
        "limits": "8s wall per isolated process; 2GiB RLIMIT_AS",
        "numeric_qualification": "reference signed printed-weight exact rational, absolute/relative 10^-(digits-6); SAT separately",
        "cross_pivot_output_hash": "NOT_EXPECTED_SAME_DUE_TO_DIFFERENT_SUM_ORDER",
        "rows": [],
    }
    has_error = False
    for case in CASES:
        source_row = orig_rows[case]
        exact_samples = [
            s for s in source_row["samples"]
            if s["tool"] == "rawmocs" and s["status"] == "COMPLETED"
        ]
        if not exact_samples:
            raise SystemExit(f"{case}: missing exact source WMC reference")
        if any(
            s["sat"] != exact_samples[0]["sat"]
            or s["real"] != exact_samples[0]["real"]
            or s["imag"] != exact_samples[0]["imag"]
            for s in exact_samples
        ):
            raise SystemExit(f"{case}: exact reference not deterministic")
        exact = (Fraction(exact_samples[0]["real"]), Fraction(exact_samples[0]["imag"]))
        original_structure = exact_samples[0].get("rawmocs_stats")
        if original_structure is None:
            raise SystemExit(f"{case}: missing original DPLL structure")
        cnf = args.corpus / f"{case}-gpmc.cnf"
        digest = sha256(cnf.read_bytes()).hexdigest()
        if digest != source_row["cnf_sha256"]:
            raise SystemExit(f"{case}: source CNF SHA mismatch")
        for digits, bits in PRECISIONS.items():
            source_verified = certified[(case, digits)]
            if source_verified["status"] != "MATCHED_PRECISION_QUALIFIED":
                raise SystemExit(f"{case}: unqualified original MPFR baseline")
            if source_verified["source_cnf_sha256"] != digest:
                raise SystemExit(f"{case}: MPFR/source CNF bytes differ")
            samples = []
            for trial in range(args.trials):
                order = ARMS[trial % len(ARMS):] + ARMS[:trial % len(ARMS)]
                for arm in order:
                    command = [
                        str(args.mpfr.resolve()),
                        *FLAGS[arm], "--fast-assign", "--hashbrown-borrowed",
                        str(digits), str(cnf.resolve()),
                    ]
                    sample = timed_execution(command, "rawmocs_mpfr")
                    sample["arm"], sample["trial"] = arm, trial
                    samples.append(sample)
            row: dict[str, Any] = {
                "case": case, "digits": digits, "bits": bits,
                "source_cnf_sha256": digest,
                "source_qasm_sha256": source_row["qasm_sha256"],
                "source_phase_sha256": source_row["phase_sha256"],
                "reference_scope": (
                    "independent_full_enumeration"
                    if source_row["exact_printed_oracle_available"]
                    else "source_locked_cross_solver_exact_wmc"
                ),
                "samples": samples,
            }
            statuses: dict[str, str] = {}
            for arm in ARMS:
                arm_samples = [s for s in samples if s["arm"] == arm]
                if any(s["status"] != "COMPLETED" for s in arm_samples):
                    statuses[arm] = "CENSORED_TIMEOUT_RESOURCE_OR_NATIVE_ERROR"
                    continue
                if any(
                    s.get("mpfr_precision_bits") != bits
                    or s.get("mpfr_phase_ns") is None
                    or s.get("mpfr_stats") is None
                    for s in arm_samples
                ):
                    statuses[arm] = "CENSORED_INCOMPLETE_MPFR_TELEMETRY"
                    has_error = True
                    continue
                if any(
                    s["sat"] != exact_samples[0]["sat"]
                    or not admissible(Fraction(s["real"]), exact[0], digits)
                    or not admissible(Fraction(s["imag"]), exact[1], digits)
                    for s in arm_samples
                ):
                    statuses[arm] = "FAIL_NUMERIC_OR_SAT"
                    has_error = True
                    continue
                # Same arm must be repeatable. Different pivot orders may
                # legitimately sum in another order within the tolerance.
                if len({
                    (s["native_stdout_sha256"], tuple(sorted(s["mpfr_stats"].items())))
                    for s in arm_samples
                }) != 1:
                    statuses[arm] = "CENSORED_NONDETERMINISTIC_ARM"
                    has_error = True
                    continue
                if arm == "frequency" and arm_samples[0]["mpfr_stats"] != original_structure:
                    statuses[arm] = "FAIL_BASELINE_DPLL_STRUCTURE_DRIFT"
                    has_error = True
                    continue
                statuses[arm] = "QUALIFIED"
            row["arm_status"] = statuses
            row["wall_ns_median"] = {}
            row["solve_ns_median"] = {}
            row["peak_rss_kib_median"] = {}
            row["search_stats"] = {}
            for arm in ARMS:
                if statuses[arm] != "QUALIFIED":
                    continue
                group = [s for s in samples if s["arm"] == arm]
                row["wall_ns_median"][arm] = statistics.median(s["wall_ns"] for s in group)
                row["solve_ns_median"][arm] = statistics.median(
                    s["mpfr_phase_ns"]["solve"] for s in group
                )
                row["peak_rss_kib_median"][arm] = statistics.median(s["peak_rss_kib"] for s in group)
                row["search_stats"][arm] = group[0]["mpfr_stats"]
            if statuses["frequency"] != "QUALIFIED":
                has_error = True
            row["baseline_over_candidate"] = {
                arm: row["wall_ns_median"]["frequency"] / row["wall_ns_median"][arm]
                for arm in ("short_clause", "moms")
                if statuses["frequency"] == "QUALIFIED" and statuses[arm] == "QUALIFIED"
            }
            report["rows"].append(row)
            print(
                f"MPFR_PIVOT_AB case={case} bits={bits} "
                + " ".join(
                    f"{arm}={statuses[arm]}:{row['wall_ns_median'].get(arm, 'CENSORED')}"
                    for arm in ARMS
                )
            )
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    if has_error:
        raise SystemExit("numeric/determinism/baseline qualification failed; no pivot result promoted")


if __name__ == "__main__":
    main()
