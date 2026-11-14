#!/usr/bin/env python3
"""Source-pinned long-running QuPRS CNF GPMC MPFR precision sweep.

This is a *single-solver* GPMC precision characterization, NOT a fair
GPMC-vs-exact-RaWMoCS speedup. A Rust MPFR implementation must be qualified
against the same 100/167/333-bit arithmetic before cross-solver ranking.
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

# Most expensive and structurally relevant source-emitted CNFs first.
LONG_CASES = ("feynman_qft_4", "feynman_mod5_4", "qaoa3", "grover2")
SMALL_CONTROL = "qft3"
PRECISIONS = (30, 50, 100)
SOURCE_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
ACCEPTED = {"ORACLE_VERIFIED_PAIRED", "SOLVER_AGREEMENT_ONLY"}
LOG2_10 = 3.3219280948873624


def equal_to_reference(actual: Fraction, reference: Fraction, digits: int) -> bool:
    # A deliberately conservative absolute guard for cancellation and a
    # relative guard for nonzero magnitudes; these are an accuracy check,
    # NOT a proof of mathematical error bounds for MPFR WMC.
    guard = Fraction(1, 10 ** (digits - 6))
    return abs(actual - reference) <= guard * max(Fraction(1), abs(reference))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-report", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--gpmc", type=Path, required=True)
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if not 2 <= args.trials <= 5:
        raise SystemExit("research trials must be in [2, 5]")
    original = json.loads(args.source_report.read_text())
    if original["source_commit"] != SOURCE_SHA:
        raise SystemExit("original QuPRS encoder commit is not pinned")
    source_rows = {row["case"]: row for row in original["rows"]}
    result: dict[str, Any] = {
        "source_quprs_commit": SOURCE_SHA,
        "measurement": "GPMC C++ native fresh-process WallTime including launch/parse/preprocess/solve/output",
        "precision_status": "ONE_SOLVER_ONLY__NOT_GPMC_VS_RAWMOCS_FAIR_SPEEDUP",
        "baseline_reference": "source-printed exact rational RaWMoCS (oracle <=17 variables; cross-solver otherwise)",
        "gpmc_flags": ["-mode=1", "-prec=<digits>"],
        "gpmc_minimum_digits": 30,
        "precision_bits": {str(d): __import__("math").ceil(d * LOG2_10) for d in PRECISIONS},
        "long_case_priority": list(LONG_CASES),
        "resource_scope": "8s process wall timeout, 2 GiB RLIMIT_AS, same runner, peak RSS KiB",
        "rows": [],
    }
    invalid = False
    for case in (*LONG_CASES, SMALL_CONTROL):
        expected = source_rows[case]
        if expected.get("status") not in ACCEPTED:
            raise SystemExit(f"{case}: original source is not qualified")
        cnf = args.corpus / f"{case}-gpmc.cnf"
        sha = sha256(cnf.read_bytes()).hexdigest()
        if sha != expected["cnf_sha256"]:
            raise SystemExit(f"{case}: source CNF hash mismatch")
        exact_samples = [
            s for s in expected["samples"]
            if s["tool"] == "rawmocs" and s["status"] == "COMPLETED"
        ]
        if not exact_samples:
            raise SystemExit(f"{case}: exact printed-rational reference unavailable")
        exact = (
            Fraction(exact_samples[0]["real"]),
            Fraction(exact_samples[0]["imag"]),
        )
        if any(
            s["sat"] != exact_samples[0]["sat"]
            or (Fraction(s["real"]), Fraction(s["imag"])) != exact
            for s in exact_samples
        ):
            raise SystemExit(f"{case}: exact reference not stable")
        for digits in PRECISIONS:
            records = []
            for trial in range(args.trials):
                command = [
                    str(args.gpmc.resolve()), "-mode=1",
                    f"-prec={digits}", str(cnf.resolve()),
                ]
                sample = timed_execution(command, "gpmc")
                sample["trial"] = trial
                records.append(sample)
            row = {
                "case": case, "cnf_sha256": sha,
                "original_qasm_sha256": expected["qasm_sha256"],
                "variables": expected["variables"], "clauses": expected["clauses"],
                "precision_digits": digits,
                "precision_bits": result["precision_bits"][str(digits)],
                "expected_sat": exact_samples[0]["sat"],
                "reference_source": "independent_exhaustive" if expected["exact_printed_oracle_available"]
                    else "source_pinned_exact_rational_cross_solver",
                "samples": records,
            }
            if any(s["status"] != "COMPLETED" for s in records):
                row["status"] = "CENSORED_GPMC_NATIVE_FAILURE"
            else:
                valid = all(
                    s["sat"] == exact_samples[0]["sat"]
                    and equal_to_reference(Fraction(s["real"]), exact[0], digits)
                    and equal_to_reference(Fraction(s["imag"]), exact[1], digits)
                    for s in records
                )
                if valid:
                    row["status"] = "GPMC_PRECISION_REFERENCE_AGREEMENT"
                    row["median_wall_ns"] = statistics.median(s["wall_ns"] for s in records)
                    row["median_peak_rss_kib"] = statistics.median(
                        s["peak_rss_kib"] for s in records
                    )
                else:
                    row["status"] = "CENSORED_NUMERICAL_DISAGREEMENT"
            if row["status"] != "GPMC_PRECISION_REFERENCE_AGREEMENT":
                invalid = True
            result["rows"].append(row)
            print(
                f"GPMC_PRECISION_SWEEP case={case} digits={digits} "
                f"bits={row['precision_bits']} status={row['status']} "
                f"wall_median_ns={row.get('median_wall_ns', 'CENSORED')}"
            )
    args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf8")
    if invalid:
        raise SystemExit("precision sweep has censored cases: inspect full artifacts")


if __name__ == "__main__":
    main()
