#!/usr/bin/env python3
"""Matched MPFR digits2bits GPMC C++ vs research Rust native-process WMC.

Long source-emitted QuPRS CNFs first, no exact-vs-MPFR speed ranking.
Numeric and structural parity must pass before reporting performance.
"""
from __future__ import annotations

import argparse
from fractions import Fraction
from hashlib import sha256
import json
from pathlib import Path
import re
import statistics
from typing import Any

from quprs_representative_native import timed_execution

LONG = ("feynman_qft_4", "feynman_mod5_4", "qaoa3", "grover2", "qft3")
DIGITS = (30, 50, 100)
BITS = {30: 100, 50: 167, 100: 333}
SOURCE_SHA = "0b28736651b3740ed3b201edd2efdef3da541e0d"
ACCEPTED = {"ORACLE_VERIFIED_PAIRED", "SOLVER_AGREEMENT_ONLY"}


def near(a: Fraction, b: Fraction, digits: int) -> bool:
    # This is an admission heuristic, not a certified interval error.
    tolerance = Fraction(1, 10 ** (digits - 6))
    return abs(a - b) <= tolerance * max(Fraction(1), abs(b))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-report", type=Path, required=True)
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--gpmc", type=Path, required=True)
    ap.add_argument("--rawmocs-mpfr", type=Path, required=True)
    ap.add_argument("--report", type=Path, required=True)
    ap.add_argument("--trials", type=int, default=3)
    args = ap.parse_args()
    if not 2 <= args.trials <= 4:
        raise SystemExit("two to four fresh native process trials required")
    original = json.loads(args.source_report.read_text())
    if original["source_commit"] != SOURCE_SHA:
        raise SystemExit("QuPRS original emitter source is not pinned")
    source_rows = {r["case"]: r for r in original["rows"]}
    result: dict[str, Any] = {
        "source_sha": SOURCE_SHA,
        "native_reference": "GPMC MPFR C++ CLI versus RaWMoCS MPFR Rust research-only CLI",
        "numeric_contract": "same MPFR mantissa bits, nearest-even, separate four rounded complex products; possibly distinct summation order",
        "source_contract": "same original QuPRS emitted weighted CNF, signed literal weights, SAT distinct from WMC zero",
        "time_contract": "fresh process wall including input/read/parse/preprocessing/search/arithmetic/serialization; bounded 8s and 2GiB virtual memory",
        "digits_to_bits": BITS,
        "trials_per_tool": args.trials,
        "rows": [],
    }
    any_unqualified = False
    for case in LONG:
        source_row = source_rows[case]
        if source_row.get("status") not in ACCEPTED:
            raise SystemExit(f"{case}: original QuPRS source not qualified")
        file = args.corpus / f"{case}-gpmc.cnf"
        input_sha = sha256(file.read_bytes()).hexdigest()
        if input_sha != source_row["cnf_sha256"]:
            raise SystemExit(f"{case}: original weighted CNF drift")
        exacts = [
            s for s in source_row["samples"]
            if s["tool"] == "rawmocs" and s["status"] == "COMPLETED"
        ]
        if not exacts:
            raise SystemExit(f"{case}: exact printed rational witness unavailable")
        exact = (Fraction(exacts[0]["real"]), Fraction(exacts[0]["imag"]))
        original_structure = exacts[0].get("rawmocs_stats")
        if original_structure is None:
            raise SystemExit(f"{case}: no exact DPLL structure witness")
        if any(
            s["sat"] != exacts[0]["sat"] or
            (Fraction(s["real"]), Fraction(s["imag"])) != exact or
            s.get("rawmocs_stats") != original_structure
            for s in exacts
        ):
            raise SystemExit(f"{case}: original exact witnesses are inconsistent")
        for digits in DIGITS:
            samples = []
            for trial in range(args.trials):
                order = ("gpmc", "rawmocs_mpfr") if trial % 2 == 0 else ("rawmocs_mpfr", "gpmc")
                for tool in order:
                    command = (
                        [str(args.gpmc.resolve()), "-mode=1", f"-prec={digits}", str(file.resolve())]
                        if tool == "gpmc" else
                        [str(args.rawmocs_mpfr.resolve()), str(digits), str(file.resolve())]
                    )
                    sample = timed_execution(command, tool)
                    sample["trial"] = trial
                    sample["tool"] = tool
                    samples.append(sample)
            row = {
                "case": case, "variables": source_row["variables"],
                "clauses": source_row["clauses"], "source_cnf_sha256": input_sha,
                "source_qasm_sha256": source_row["qasm_sha256"],
                "source_phase_sha256": source_row["phase_sha256"],
                "digits": digits, "bits": BITS[digits],
                "source_oracle_scope": "exhaustive" if source_row["exact_printed_oracle_available"]
                    else "cross_solver_reference_only",
                "samples": samples,
            }
            if any(s["status"] != "COMPLETED" for s in samples):
                row["status"] = "CENSORED_NATIVE_FAILURE"
            else:
                numerical = all(
                    s["sat"] == exacts[0]["sat"] and
                    near(Fraction(s["real"]), exact[0], digits) and
                    near(Fraction(s["imag"]), exact[1], digits)
                    for s in samples
                )
                mpfr_samples = [s for s in samples if s["tool"] == "rawmocs_mpfr"]
                structure = all(
                    s.get("mpfr_stats") == original_structure and
                    s.get("mpfr_precision_bits") == BITS[digits]
                    for s in mpfr_samples
                )
                row["numeric_reference_admitted"] = numerical
                row["structural_parity_admitted"] = structure
                if numerical and structure:
                    row["status"] = "MATCHED_PRECISION_QUALIFIED"
                    row["wall_ns_median"] = {
                        name: statistics.median(s["wall_ns"] for s in samples if s["tool"] == name)
                        for name in ("gpmc", "rawmocs_mpfr")
                    }
                    row["peak_rss_kib_median"] = {
                        name: statistics.median(s["peak_rss_kib"] for s in samples if s["tool"] == name)
                        for name in ("gpmc", "rawmocs_mpfr")
                    }
                    row["speed_ratio_gpmc_over_rust"] = (
                        row["wall_ns_median"]["gpmc"] /
                        row["wall_ns_median"]["rawmocs_mpfr"]
                    )
                else:
                    row["status"] = "CENSORED_NUMERIC_OR_STRUCTURE_MISMATCH"
            if row["status"] != "MATCHED_PRECISION_QUALIFIED":
                any_unqualified = True
            result["rows"].append(row)
            ratio = row.get("speed_ratio_gpmc_over_rust")
            print(
                f"MATCHED_MPFR case={case} digits={digits} bits={BITS[digits]} "
                f"status={row['status']} ratio={'CENSORED' if ratio is None else f'{ratio:.4f}'}"
            )
    args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf8")
    if any_unqualified:
        raise SystemExit("matched MPFR science gates failed; censored cases are NOT timings")


if __name__ == "__main__":
    main()
